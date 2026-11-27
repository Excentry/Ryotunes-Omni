# Native dependency advisory monitoring

The weekly `Native dependency advisories` workflow queries OSV for the exact 40-character Git revisions in `scripts/windows/owned-libmpv/sources.lock.json`. The current lock contains 16 source entries. It reads the bundled SQLite version from the `sqlite3.h` shipped by the locked `libsqlite3-sys` crate and queries the NVD CPE API for that version. A non-empty result fails the scheduled check and prints advisory identifiers and links; results require human applicability review and are not, by themselves, proof that a shipped binary is vulnerable.

To run the check locally:

1. Install the repository's locked Rust dependencies with `cargo fetch --locked`.
2. Run `python3 scripts/check-native-advisories.py`.
3. Run `python3 -m unittest scripts/test_check_native_advisories.py`.

Refresh policy:

- Review alerts weekly and whenever the native source lock, SQLite dependency, or bundled media recipe changes.
- For each finding, verify the affected commit/version range, whether affected code is included and enabled in the produced artifact, and whether a fixed upstream revision is available.
- Update native pins only with a new integrity hash and a fresh Windows candidate build and runtime validation. Keep the prior source/binary evidence immutable.
- For SQLite changes, update through the locked `rusqlite`/`libsqlite3-sys` dependency and record the exact embedded SQLite version in the check output; do not infer it from the Rust crate version.
- Record false positives and non-applicable findings with the evidence and date. Do not mark an advisory resolved solely because a query no longer returns it.

Initial check on 2026-11-27 returned advisories for pinned mpv, FFmpeg, HarfBuzz and libxml2 commits, and for SQLite 3.54.1. For example, OSV `CVE-2026-123095` describes an mpv `ytdl_hook` playlist-injection issue, while NVD `CVE-2026-119586` describes SQLite shell `.recover` command injection. The complete current identifiers are printed by the checker and may change as advisories are added or corrected. These findings need artifact-level applicability review; this inventory does not establish whether the affected code is shipped or reachable in Ryotune Omni.
