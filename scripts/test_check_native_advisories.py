import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("check-native-advisories.py")
SPEC = importlib.util.spec_from_file_location("check_native_advisories", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class NativeAdvisoryCheckTests(unittest.TestCase):
    def test_load_sources_rejects_non_commit_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sources.json"
            path.write_text(
                '{"sources":[{"name":"mpv","repository":"mpv-player/mpv","revision":"not-a-commit"}]}',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "invalid Git revision"):
                MODULE.load_sources(path)

    def test_load_sqlite_version_reads_locked_bundled_header(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cargo_lock = root / "Cargo.lock"
            cargo_lock.write_text(
                '[[package]]\nname = "libsqlite3-sys"\nversion = "0.39.1"\n',
                encoding="utf-8",
            )
            header = root / "registry/src/index/libsqlite3-sys-0.39.1/sqlite3/sqlite3.h"
            header.parent.mkdir(parents=True)
            header.write_text('#define SQLITE_VERSION        "3.54.1"\n', encoding="utf-8")
            self.assertEqual(
                MODULE.load_sqlite_version(cargo_lock, root),
                ("0.39.1", "3.54.1"),
            )

    def test_sqlite_cpe_filter_ignores_non_vulnerable_context_matches(self):
        cve = {
            "configurations": [
                {
                    "nodes": [
                        {
                            "operator": "AND",
                            "cpeMatch": [
                                {
                                    "vulnerable": False,
                                    "criteria": "cpe:2.3:a:sqlite:sqlite:*:*:*:*:*:*:*:*",
                                },
                                {
                                    "vulnerable": True,
                                    "criteria": "cpe:2.3:a:php:php:*:*:*:*:*:*:*:*",
                                },
                            ],
                        }
                    ]
                }
            ]
        }
        self.assertEqual(MODULE.sqlite_cpe_matches(cve), [])


if __name__ == "__main__":
    unittest.main()
