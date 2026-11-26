import hashlib
import json
import shutil
import tempfile
import unittest
from pathlib import Path

from package import REQUIRED_NOTICES, package
from smoke import verify_checksums


class CandidatePackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        (self.root / "candidate").mkdir()
        (self.root / "prepared/sources/freetype/docs").mkdir(parents=True)
        (self.root / "prepared/recipe").mkdir(parents=True)
        (self.root / "prepared/recipe/sources.lock.json").write_text("{}")
        (self.root / "prepared/recipe/REVIEW.md").write_text("Review fixture")
        (self.root / "prepared/recipe/NOTICE.md").write_text("Updated notice fixture")
        recipe = Path(__file__).resolve().parent
        shutil.copyfile(recipe / "notice-inputs.lock.json", self.root / "prepared/recipe/notice-inputs.lock.json")
        shutil.copytree(recipe / "toolchain-licenses", self.root / "prepared/recipe/toolchain-licenses")
        for relative in REQUIRED_NOTICES:
            source = self.root / "prepared/sources" / relative
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_bytes(b"Upstream grant\r\n")
        (self.root / "prepared/sources/freetype/docs/FTL.TXT").write_bytes(b"Example notice\r\n")
        for name in ("libmpv-2.dll", "mpv.lib", "source-inputs.tar.gz", "NOTICE.md"):
            (self.root / "candidate" / name).write_bytes(b"test fixture")
        (self.root / "candidate/pe-imports.txt").write_text("\tDLL Name: KERNEL32.dll\n")

    def test_packaging_preserves_notice_bytes_and_matches_checksums(self):
        package(self.root)
        verify_checksums(self.root / "candidate")
        self.assertEqual((self.root / "candidate/licenses/freetype/docs/FTL.TXT").read_bytes(), b"Example notice\r\n")
        metadata = json.loads((self.root / "candidate/candidate.json").read_text())
        self.assertEqual(metadata["state"], "candidate-not-approved-for-distribution")
        self.assertEqual(metadata["source_archive_sha256"], hashlib.sha256((self.root / "candidate/source-inputs.tar.gz").read_bytes()).hexdigest())
        self.assertEqual((self.root / "candidate/NOTICE.md").read_text(), "Updated notice fixture")

    def test_candidate_tampering_is_rejected(self):
        package(self.root)
        (self.root / "candidate/mpv.lib").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            verify_checksums(self.root / "candidate")

    def test_required_embedded_grants_are_preserved_exactly(self):
        package(self.root)
        for relative in REQUIRED_NOTICES:
            with self.subTest(path=relative):
                self.assertEqual((self.root / "candidate/licenses" / relative).read_bytes(), (self.root / "prepared/sources" / relative).read_bytes())

    def test_missing_required_notice_blocks_packaging(self):
        (self.root / "prepared/sources/libxml2/dict.c").rename(self.root / "dict.backup")
        with self.assertRaisesRegex(ValueError, "Required upstream notice missing: libxml2/dict.c"):
            package(self.root)

    def test_toolchain_notices_are_preserved_exactly(self):
        package(self.root)
        lock = json.loads((self.root / "candidate/notice-inputs.lock.json").read_text())
        for entry in lock["files"]:
            with self.subTest(path=entry["path"]):
                self.assertEqual((self.root / "candidate/licenses/toolchain" / entry["path"]).read_bytes(), (self.root / "prepared/recipe/toolchain-licenses" / entry["path"]).read_bytes())
        verify_checksums(self.root / "candidate")

    def test_toolchain_notice_tampering_blocks_packaging(self):
        (self.root / "prepared/recipe/toolchain-licenses/gcc/COPYING.RUNTIME").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "Notice checksum mismatch"):
            package(self.root)

    def test_shared_runtime_import_blocks_packaging(self):
        (self.root / "candidate/pe-imports.txt").write_text("\tDLL Name: libgcc_s_seh-1.dll\n")
        with self.assertRaisesRegex(ValueError, "libgcc_s_seh-1.dll"):
            package(self.root)

    def test_license_directory_contents_are_preserved(self):
        source = self.root / "prepared/sources/harfbuzz/LICENSES/MIT.txt"
        source.parent.mkdir()
        source.write_bytes(b"Additional grant\r\n")
        package(self.root)
        self.assertEqual((self.root / "candidate/licenses/harfbuzz/LICENSES/MIT.txt").read_bytes(), source.read_bytes())
        verify_checksums(self.root / "candidate")

    def test_missing_manifest_entry_is_rejected(self):
        package(self.root)
        sums = self.root / "candidate/SHA256SUMS.txt"
        sums.write_text("\n".join(line for line in sums.read_text().splitlines() if not line.endswith("  mpv.lib")) + "\n")
        with self.assertRaisesRegex(ValueError, "incomplete"):
            verify_checksums(self.root / "candidate")


if __name__ == "__main__":
    unittest.main()
