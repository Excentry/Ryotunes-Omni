import copy
import io
import json
import shutil
import tempfile
import tarfile
import unittest
from pathlib import Path

import prepare


class SourcePreparationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)

    def archive(self, entries):
        path = self.root / "input.tar.gz"
        with tarfile.open(path, "w:gz") as archive:
            for name, data in entries:
                member = tarfile.TarInfo(name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        return path

    def test_checked_in_lock_has_full_revisions_and_hashes(self):
        lock = prepare.load_lock(prepare.RECIPE / "sources.lock.json")
        self.assertEqual(len(lock["sources"]), 16)
        mingw = next(source for source in lock["sources"] if source["name"] == "mingw-w64")
        self.assertEqual(mingw["revision"], "7213d719fe344553aa24fef7cc7dedee4940d594")
        self.assertNotIn("subrandr", {source["name"] for source in lock["sources"]})

    def test_checked_in_toolchain_notices_match_locked_bytes(self):
        lock = prepare.load_notice_inputs(prepare.RECIPE)
        self.assertEqual(len(lock["files"]), 5)
        mingw_notices = [entry for entry in lock["files"] if entry["path"].startswith("mingw-w64/")]
        self.assertEqual(len(mingw_notices), 3)
        self.assertTrue(all("/7213d719fe344553aa24fef7cc7dedee4940d594/" in entry["url"] for entry in mingw_notices))

    def test_notice_lock_rejects_unpinned_urls_and_unsafe_paths(self):
        original = json.loads((prepare.RECIPE / "notice-inputs.lock.json").read_text())
        for field, value in (("url", "https://raw.githubusercontent.com/gcc-mirror/gcc/master/COPYING.RUNTIME"), ("path", "../outside"), ("sha256", "unverified")):
            with self.subTest(field=field):
                lock = copy.deepcopy(original)
                lock["files"][0][field] = value
                (self.root / "notice-inputs.lock.json").write_text(json.dumps(lock))
                with self.assertRaises(ValueError):
                    prepare.load_notice_inputs(self.root)

    def test_notice_lock_rejects_duplicate_inputs(self):
        shutil.copytree(prepare.RECIPE / "toolchain-licenses", self.root / "toolchain-licenses")
        lock = json.loads((prepare.RECIPE / "notice-inputs.lock.json").read_text())
        lock["files"].append(copy.deepcopy(lock["files"][0]))
        (self.root / "notice-inputs.lock.json").write_text(json.dumps(lock))
        with self.assertRaisesRegex(ValueError, "Duplicate notice input"):
            prepare.load_notice_inputs(self.root)

    def test_missing_locked_notice_is_rejected(self):
        shutil.copyfile(prepare.RECIPE / "notice-inputs.lock.json", self.root / "notice-inputs.lock.json")
        with self.assertRaises(FileNotFoundError):
            prepare.load_notice_inputs(self.root)

    def test_cached_tampering_is_rejected_even_offline(self):
        path = self.root / "example.tar.gz"
        path.write_bytes(b"tampered")
        source = {"name": "example", "sha256": "0" * 64}
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            prepare.acquire(source, self.root, True)

    def test_missing_offline_input_fails(self):
        with self.assertRaisesRegex(ValueError, "unavailable offline"):
            prepare.acquire({"name": "missing", "sha256": "0" * 64}, self.root, True)

    def test_extracts_root_and_preserves_notice_bytes(self):
        revision = "a" * 40
        data = b"Copyright example\r\nPermission grant\n"
        archive = self.archive([(f"example-{revision}/LICENSE", data)])
        prepare.extract_source(archive, self.root / "output", revision)
        self.assertEqual((self.root / "output/LICENSE").read_bytes(), data)

    def test_archive_parent_traversal_is_rejected(self):
        revision = "a" * 40
        archive = self.archive([(f"example-{revision}/../../outside", b"unsafe")])
        with self.assertRaises(ValueError):
            prepare.extract_source(archive, self.root / "output", revision)
        self.assertFalse((self.root / "outside").exists())

    def test_absolute_archive_path_is_rejected(self):
        archive = self.archive([("/absolute/file", b"unsafe")])
        with self.assertRaises(ValueError):
            prepare.extract_source(archive, self.root / "output", "a" * 40)

    def test_escaping_symlink_is_rejected(self):
        revision = "a" * 40
        path = self.root / "symlink.tar.gz"
        with tarfile.open(path, "w:gz") as archive:
            member = tarfile.TarInfo(f"example-{revision}/link")
            member.type = tarfile.SYMTYPE
            member.linkname = "../../outside"
            archive.addfile(member)
        with self.assertRaises(tarfile.FilterError):
            prepare.extract_source(path, self.root / "output", revision)

    def test_unpinned_revision_and_hash_are_rejected(self):
        original = json.loads((prepare.RECIPE / "sources.lock.json").read_text())
        for field, value in (("revision", "main"), ("sha256", None), ("destination", "../outside")):
            with self.subTest(field=field):
                lock = copy.deepcopy(original)
                lock["sources"][0][field] = value
                path = self.root / "lock.json"
                path.write_text(json.dumps(lock))
                with self.assertRaises(ValueError):
                    prepare.load_lock(path)


if __name__ == "__main__":
    unittest.main()
