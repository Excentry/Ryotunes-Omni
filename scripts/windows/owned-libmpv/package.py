import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

from imports import verify_pe_imports
from prepare import load_notice_inputs


REQUIRED_NOTICES = (
    "mpv/Copyright",
    "mpv/LICENSE.GPL",
    "mpv/LICENSE.LGPL",
    "ffmpeg/LICENSE.md",
    "ffmpeg/COPYING.GPLv3",
    "ffmpeg/libavcodec/jfdctfst.c",
    "ffmpeg/libavcodec/jfdctint_template.c",
    "ffmpeg/libavcodec/jrevdct.c",
    "rubberband/COPYING",
    "rubberband/README.md",
    "rubberband/src/ext/pommier/sse_mathfun.h",
    "rubberband/src/ext/float_cast/float_cast.h",
    "libplacebo/LICENSE",
    "freetype/LICENSE.TXT",
    "freetype/docs/FTL.TXT",
    "freetype/docs/GPLv2.TXT",
    "freetype/src/bdf/README",
    "freetype/src/bdf/bdflib.c",
    "freetype/src/pcf/README",
    "freetype/src/pcf/pcfread.c",
    "freetype/src/gzip/zlib.h",
    "fribidi/COPYING",
    "fribidi/lib/fribidi.h",
    "harfbuzz/COPYING",
    "harfbuzz/src/ms-use/COPYING",
    "libass/COPYING",
    "libxml2/Copyright",
    "libxml2/dict.c",
    "libxml2/list.c",
    "libplacebo/3rdparty/fast_float/LICENSE-MIT",
    "libplacebo/3rdparty/jinja/LICENSE.txt",
    "libplacebo/3rdparty/markupsafe/LICENSE.txt",
    "libplacebo/3rdparty/glad/LICENSE",
    "libplacebo/3rdparty/Vulkan-Headers/LICENSE.md",
    "libplacebo/3rdparty/Vulkan-Headers/LICENSES/Apache-2.0.txt",
    "libplacebo/3rdparty/Vulkan-Headers/LICENSES/MIT.txt",
    "libplacebo/demos/3rdparty/nuklear/LICENSE",
)


def package(output):
    prepared = output / "prepared"
    candidate = output / "candidate"
    sources = prepared / "sources"
    notice_inputs = load_notice_inputs(prepared / "recipe")
    imports = verify_pe_imports(candidate / "pe-imports.txt")
    for relative in REQUIRED_NOTICES:
        if not (sources / relative).is_file():
            raise ValueError(f"Required upstream notice missing: {relative}")
    shutil.copyfile(prepared / "recipe/REVIEW.md", candidate / "REVIEW.md")
    shutil.copyfile(prepared / "recipe/NOTICE.md", candidate / "NOTICE.md")
    records = []
    for entry in notice_inputs["files"]:
        source = prepared / "recipe/toolchain-licenses" / entry["path"]
        destination = candidate / "licenses/toolchain" / entry["path"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        records.append({"path": destination.relative_to(candidate).as_posix(), "sha256": entry["sha256"]})
    shutil.copyfile(prepared / "recipe/toolchain-licenses/README.md", candidate / "licenses/toolchain/README.md")
    shutil.copyfile(prepared / "recipe/notice-inputs.lock.json", candidate / "notice-inputs.lock.json")
    for source in sorted(sources.rglob("*")):
        if not source.is_file():
            continue
        relative = source.relative_to(sources)
        required_notice = relative.as_posix() in REQUIRED_NOTICES
        license_directory = any(part.lower() in {"licenses", "licences"} for part in relative.parts[:-1])
        notice_filename = re.match(r"(?i)^(?:licen[cs]e|copying|copyright|notice|authors)(?:[._-]|$)", source.name) or source.name in {"FTL.TXT", "GPLv2.TXT"}
        if not (required_notice or license_directory or notice_filename):
            continue
        destination = candidate / "licenses" / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        records.append({"path": destination.relative_to(candidate).as_posix(), "sha256": hashlib.sha256(destination.read_bytes()).hexdigest()})
    for source in output.glob("*-dependencies.json"):
        shutil.copyfile(source, candidate / source.name)
    for source in output.glob("*-options.json"):
        shutil.copyfile(source, candidate / source.name)
    shutil.copyfile(prepared / "recipe/sources.lock.json", candidate / "sources.lock.json")
    with (candidate / "source-inputs.tar.gz").open("rb") as source_archive:
        source_archive_sha256 = hashlib.file_digest(source_archive, "sha256").hexdigest()
    metadata = {
        "state": "candidate-not-approved-for-distribution",
        "source_archive": "source-inputs.tar.gz",
        "source_archive_sha256": source_archive_sha256,
        "notice_files": records,
        "license_review": "REVIEW.md",
        "toolchain_notices": "notice-inputs.lock.json",
        "reported_windows_imports": imports,
        "pending": ["complete per-file notice review", "GCC runtime source/patch confidence review: no object reproduction and patched-header inputs unresolved", "Windows provider regression matrix", "manual Windows Sandbox installation and GUI validation", "durable public source hosting and retention review", "combined application GPL distribution review"],
    }
    (candidate / "candidate.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    lines = []
    for path in sorted(candidate.rglob("*")):
        if path.is_file() and path.name != "SHA256SUMS.txt":
            with path.open("rb") as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            lines.append(f"{digest}  {path.relative_to(candidate).as_posix()}")
    (candidate / "SHA256SUMS.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    package(Path(sys.argv[1]).resolve())
