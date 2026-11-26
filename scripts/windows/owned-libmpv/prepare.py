import argparse
import copy
import hashlib
import json
import platform
import re
import shutil
import tarfile
import urllib.request
from pathlib import Path, PurePosixPath


RECIPE = Path(__file__).resolve().parent


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def safe_relative(value):
    path = PurePosixPath(value)
    if not value or not path.parts or path.is_absolute() or ".." in path.parts or "\\" in value or ":" in value:
        raise ValueError(f"Unsafe relative path: {value}")
    return path


def load_lock(path):
    lock = json.loads(Path(path).read_text(encoding="utf-8"))
    if lock.get("schema") != 1 or not lock.get("sources"):
        raise ValueError("Unsupported or empty source lock")
    if not re.fullmatch(r"ghcr\.io/btbn/ffmpeg-builds/win64-gpl@sha256:[a-f0-9]{64}", lock.get("toolchain_image", "")):
        raise ValueError("Toolchain image must be pinned by digest")
    names = set()
    destinations = set()
    for source in lock["sources"]:
        if not re.fullmatch(r"[a-z0-9-]+", source.get("name", "")) or source["name"] in names:
            raise ValueError("Invalid or duplicate source name")
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", source.get("repository", "")):
            raise ValueError("Invalid source repository")
        if not re.fullmatch(r"[a-f0-9]{40}", source.get("revision", "")):
            raise ValueError("Source revision must be a full commit")
        if not isinstance(source.get("sha256"), str) or not re.fullmatch(r"[a-f0-9]{64}", source["sha256"]):
            raise ValueError(f"Missing source checksum: {source['name']}")
        destination = safe_relative(source["destination"])
        if str(destination) in destinations:
            raise ValueError("Duplicate source destination")
        names.add(source["name"])
        destinations.add(str(destination))
    return lock


def load_notice_inputs(recipe):
    lock = json.loads((recipe / "notice-inputs.lock.json").read_text(encoding="utf-8"))
    if lock.get("schema") != 1 or not lock.get("files"):
        raise ValueError("Unsupported or empty notice lock")
    paths = set()
    for entry in lock["files"]:
        relative = safe_relative(entry["path"])
        if str(relative) in paths:
            raise ValueError("Duplicate notice input")
        paths.add(str(relative))
        if not re.fullmatch(r"https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/[a-f0-9]{40}/[^\s]+", entry.get("url", "")):
            raise ValueError("Notice URL must use a fixed upstream revision")
        if not re.fullmatch(r"[a-f0-9]{64}", entry.get("sha256", "")):
            raise ValueError("Missing notice checksum")
        if digest(recipe / "toolchain-licenses" / str(relative)) != entry["sha256"]:
            raise ValueError(f"Notice checksum mismatch: {relative}")
    return lock


def acquire(source, cache, offline):
    path = cache / f"{source['name']}.tar.gz"
    if path.exists():
        if digest(path) != source["sha256"]:
            raise ValueError(f"Cached source checksum mismatch: {source['name']}")
        return path
    if offline:
        raise ValueError(f"Source unavailable offline: {source['name']}")
    url = f"https://codeload.github.com/{source['repository']}/tar.gz/{source['revision']}"
    temporary = path.with_suffix(".download")
    with urllib.request.urlopen(url, timeout=180) as response, temporary.open("wb") as output:
        shutil.copyfileobj(response, output)
    if digest(temporary) != source["sha256"]:
        raise ValueError(f"Downloaded source checksum mismatch: {source['name']}")
    temporary.replace(path)
    return path


def extract_source(archive_path, destination, revision):
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        roots = {safe_relative(member.name).parts[0] for member in members}
        if len(roots) != 1 or not next(iter(roots)).endswith(f"-{revision}"):
            raise ValueError("Unexpected source archive root")
        root = next(iter(roots))
        stripped = []
        for member in members:
            path = safe_relative(member.name)
            if len(path.parts) == 1:
                continue
            item = copy.copy(member)
            item.name = str(PurePosixPath(*path.parts[1:]))
            if item.islnk():
                target = safe_relative(item.linkname)
                if target.parts[0] != root or len(target.parts) < 2:
                    raise ValueError("Unsafe source hard link")
                item.linkname = str(PurePosixPath(*target.parts[1:]))
            if not (item.isfile() or item.isdir() or item.issym() or item.islnk()):
                raise ValueError("Unsupported archive entry type")
            stripped.append(item)
        archive.extractall(destination, members=stripped, filter="data")


def prepare(output, cache, offline=False, verify_only=False):
    lock = load_lock(RECIPE / "sources.lock.json")
    load_notice_inputs(RECIPE)
    cache.mkdir(parents=True, exist_ok=True)
    inputs = [(source, acquire(source, cache, offline)) for source in lock["sources"]]
    if verify_only:
        print(f"Verified {len(inputs)} source archives")
        return
    if output.exists() and any(output.iterdir()):
        raise ValueError("Output must be new or empty; existing work will not be overwritten")
    output.mkdir(parents=True, exist_ok=True)
    archives = output / "archives"
    archives.mkdir()
    shutil.copytree(RECIPE, output / "recipe", ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "package_release_assets.py"))
    for source, path in inputs:
        shutil.copyfile(path, archives / path.name)
        extract_source(path, output / "sources" / str(safe_relative(source["destination"])), source["revision"])
    metadata = {"state": "sources-preserved-not-release-approved", "python": platform.python_version(), "lock": lock}
    (output / "preparation.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    with tarfile.open(output / "source-inputs.tar.gz", "w:gz") as bundle:
        bundle.add(archives, arcname="archives")
        bundle.add(output / "recipe", arcname="recipe")
        bundle.add(output / "preparation.json", arcname="preparation.json")
    print(f"Preserved {len(inputs)} verified inputs: {output / 'source-inputs.tar.gz'}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    prepare(args.output.resolve(), args.cache.resolve(), args.offline, args.verify_only)


if __name__ == "__main__":
    main()
