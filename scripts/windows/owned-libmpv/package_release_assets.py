import argparse
import hashlib
import io
import json
import tarfile
import zipfile
from pathlib import Path, PurePosixPath


DLL_SHA256 = "6a6ac973ecb495b6799b32d221fdf357f212f9cbd65bc01599670ed6b9d13f56"
NATIVE_SOURCE_SHA256 = "7a5516a93b86550dc5ad5118781d572d22433dd7b2789b1e00f2c1728f1dc6bf"
GCC_SOURCE_SHA512 = "c51c30ca7422d0cbecf504b2e0f33c3aca31e0f90a76b65217f465163fa6fa17b3f5de39e145c47e5bab90ac0ce7fff3b03c8d553ae36e01faaea5a50f8648d1"
CROSSTOOL_SOURCE_SHA256 = "979434eeb1828579c15988a16241018e27e8e6f3a1b624faa0c6db159e102096"
SOURCE_MAP_SHA256 = "a27294ddabadba1e009fc08c315bb399285a3d7344be8db03e0f35cf59a6812f"


def digest(path, algorithm="sha256"):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, algorithm).hexdigest()


def verify_candidate(candidate):
    candidate = Path(candidate)
    manifest = candidate / "SHA256SUMS.txt"
    if not manifest.is_file():
        raise ValueError("Candidate checksum manifest is missing")
    records = {}
    for line in manifest.read_text(encoding="utf-8").splitlines():
        checksum, relative = line.split("  ", 1)
        path = PurePosixPath(relative)
        if path.is_absolute() or ".." in path.parts or "\\" in relative or relative in records:
            raise ValueError("Unsafe or duplicate candidate manifest path")
        target = candidate.joinpath(*path.parts)
        if not target.is_file() or digest(target) != checksum:
            raise ValueError(f"Candidate checksum mismatch: {relative}")
        records[relative] = checksum
    for required in ("libmpv-2.dll", "mpv.lib", "NOTICE.md", "source-inputs.tar.gz", "licenses/mpv/LICENSE.GPL"):
        if required not in records:
            raise ValueError(f"Required candidate file missing from manifest: {required}")
    if records["libmpv-2.dll"] != DLL_SHA256:
        raise ValueError("Candidate DLL does not match the approved hash")
    if records["source-inputs.tar.gz"] != NATIVE_SOURCE_SHA256:
        raise ValueError("Candidate source archive does not match the reviewed source hash")
    metadata = json.loads((candidate / "candidate.json").read_text(encoding="utf-8"))
    if metadata.get("source_archive_sha256") != NATIVE_SOURCE_SHA256:
        raise ValueError("Candidate metadata does not match the reviewed source hash")
    return candidate


def source_readme():
    return """# Corresponding source for Windows libmpv 6a6ac973

This source package corresponds to `libmpv-2.dll` SHA-256 `6a6ac973ecb495b6799b32d221fdf357f212f9cbd65bc01599670ed6b9d13f56`, built in Actions run 41150745596. It is a source and build-input delivery for the standalone GPL-enabled DLL, not a Ryotunes Omni application release or a legal certification.

`source-inputs.tar.gz` SHA-256 `7a5516a93b86550dc5ad5118781d572d22433dd7b2789b1e00f2c1728f1dc6bf` contains the pinned native library source archives, recipe, locks, notices and the full MinGW-w64 7213d719 source. `toolchain/gcc-16.2.0.tar.xz` is the GCC 16.2.0 source archive verified against the upstream SHA-512 file. `toolchain/crosstool-ng-b1a94f65.tar.gz` is the crosstool-NG source tree at commit `b1a94f65c5187d99037a4e2f57aefbdf995a9677`, including the GCC 16.2.0 patch set. `provenance/gcc-runtime-source-map.json` records the 101 linked GCC-family member-to-source mappings used for review.

The build recipe uses the toolchain image pinned in `source-inputs.tar.gz`. The mapping is evidence, not a byte-for-byte reproduction of the compiler runtime objects: libstdc++ objects have no DWARF, patch 0004 changes headers that may have been compilation inputs, and the objects were not recompiled and compared. The README and review inside the recipe describe the remaining limits. No confirmed runtime-exception incompatibility was found; this is not legal advice or a compliance certification.

To prepare the native build inputs, extract `source-inputs.tar.gz` and follow `recipe/NOTICE.md` and `recipe/REVIEW.md`. The GCC and crosstool-NG archives are supplied as corresponding toolchain source and patch references; reproducing the toolchain image itself has not been tested from these archives alone.
""".encode("utf-8")


def package_release_assets(candidate, gcc_source, crosstool_source, source_map, output):
    candidate = verify_candidate(candidate)
    gcc_source = Path(gcc_source)
    crosstool_source = Path(crosstool_source)
    source_map = Path(source_map)
    output = Path(output)
    if not gcc_source.is_file() or digest(gcc_source, "sha512") != GCC_SOURCE_SHA512:
        raise ValueError("GCC source does not match the upstream SHA-512")
    if not crosstool_source.is_file() or digest(crosstool_source) != CROSSTOOL_SOURCE_SHA256:
        raise ValueError("crosstool-NG source does not match the pinned commit archive")
    if not source_map.is_file() or digest(source_map) != SOURCE_MAP_SHA256:
        raise ValueError("GCC runtime map does not match the reviewed evidence")
    mapping = json.loads(source_map.read_text(encoding="utf-8"))
    if len(mapping.get("members", [])) != 101 or any(item.get("state") != "exact" for item in mapping["members"]):
        raise ValueError("GCC source map is incomplete")
    if output.exists() and any(output.iterdir()):
        raise ValueError("Release asset output must be new or empty")
    output.mkdir(parents=True, exist_ok=True)
    binary_name = "libmpv-windows-x64-6a6ac973.zip"
    source_name = "libmpv-corresponding-source-6a6ac973.tar.gz"
    binary_path = output / binary_name
    with zipfile.ZipFile(binary_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        members = {
            "libmpv-2.dll": candidate / "libmpv-2.dll",
            "libmpv.dll.a": candidate / "mpv.lib",
            "LICENSE.GPL": candidate / "licenses/mpv/LICENSE.GPL",
            "NOTICE.md": candidate / "NOTICE.md",
        }
        for name, path in members.items():
            archive.write(path, name)
        sums = "".join(f"{digest(path)}  {name}\n" for name, path in sorted(members.items()))
        archive.writestr("SHA256SUMS.txt", sums)
    source_path = output / source_name
    source_files = {
        "README.md": source_readme(),
        "source-inputs.tar.gz": (candidate / "source-inputs.tar.gz").read_bytes(),
        "toolchain/gcc-16.2.0.tar.xz": gcc_source.read_bytes(),
        "toolchain/crosstool-ng-b1a94f65.tar.gz": crosstool_source.read_bytes(),
        "provenance/gcc-runtime-source-map.json": source_map.read_bytes(),
    }
    inner_sums = "".join(
        f"{hashlib.sha256(data).hexdigest()}  {name}\n" for name, data in sorted(source_files.items())
    )
    source_files["SHA256SUMS.txt"] = inner_sums.encode("utf-8")
    with tarfile.open(source_path, "w:gz", format=tarfile.PAX_FORMAT) as archive:
        for name, data in sorted(source_files.items()):
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = 0o644
            archive.addfile(info, io.BytesIO(data))
    sums = f"{digest(binary_path)}  {binary_name}\n{digest(source_path)}  {source_name}\n"
    (output / "SHA256SUMS.txt").write_text(sums, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--gcc-source", type=Path, required=True)
    parser.add_argument("--crosstool-source", type=Path, required=True)
    parser.add_argument("--source-map", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    package_release_assets(args.candidate, args.gcc_source, args.crosstool_source, args.source_map, args.output)


if __name__ == "__main__":
    main()
