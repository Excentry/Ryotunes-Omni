import json
import os
import re
import sys
import tomllib
import urllib.parse
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OSV_QUERY_BATCH = "https://api.osv.dev/v1/querybatch"
NVD_CVE_API = "https://services.nvd.nist.gov/rest/json/cves/2.0"
TIMEOUT_SECONDS = 30


def request_json(url, body=None):
    data = None if body is None else json.dumps(body).encode()
    headers = {} if body is None else {"Content-Type": "application/json"}
    request = urllib.request.Request(url, data=data, headers=headers, method="GET" if body is None else "POST")
    with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
        return json.load(response)


def load_sources(path):
    lock = json.loads(path.read_text(encoding="utf-8"))
    sources = lock["sources"]
    for source in sources:
        if not re.fullmatch(r"[0-9a-f]{40}", source["revision"]):
            raise ValueError(f"invalid Git revision for {source['name']}: {source['revision']}")
    return sources


def load_sqlite_version(cargo_lock, cargo_home):
    lock = tomllib.loads(cargo_lock.read_text(encoding="utf-8"))
    packages = [package for package in lock["package"] if package["name"] == "libsqlite3-sys"]
    if len(packages) != 1:
        raise ValueError(f"expected one locked libsqlite3-sys package, found {len(packages)}")
    version = packages[0]["version"]
    headers = list((cargo_home / "registry" / "src").glob(f"*/libsqlite3-sys-{version}/sqlite3/sqlite3.h"))
    if not headers:
        raise FileNotFoundError(
            f"expected the fetched libsqlite3-sys {version} bundled SQLite header; run cargo fetch --locked"
        )
    sqlite_versions = set()
    for header in headers:
        match = re.search(
            r'^#define SQLITE_VERSION\s+"([0-9]+\.[0-9]+\.[0-9]+)"$',
            header.read_text(encoding="utf-8"),
            re.M,
        )
        if match is None:
            raise ValueError(f"could not read SQLite version from {header}")
        sqlite_versions.add(match.group(1))
    if len(sqlite_versions) != 1:
        raise ValueError(f"conflicting bundled SQLite versions found: {sorted(sqlite_versions)}")
    return version, sqlite_versions.pop()


def query_git_advisories(sources):
    queries = [
        {
            "package": {
                "name": f"https://github.com/{source['repository']}",
                "ecosystem": "GIT",
            },
            "commit": source["revision"],
        }
        for source in sources
    ]
    response = request_json(OSV_QUERY_BATCH, {"queries": queries})
    results = {}
    for source, result in zip(sources, response["results"], strict=True):
        for advisory in result.get("vulns", []):
            results.setdefault(source["name"], set()).add(advisory["id"])
    return results


def sqlite_cpe_matches(cve):
    matches = []

    def visit(node):
        for match in node.get("cpeMatch", []):
            if match.get("vulnerable") and match.get("criteria", "").startswith(
                "cpe:2.3:a:sqlite:sqlite:"
            ):
                matches.append(match)
        for child in node.get("children", []):
            visit(child)

    for configuration in cve.get("configurations", []):
        for node in configuration.get("nodes", []):
            visit(node)
    return matches


def query_sqlite_advisories(version):
    cpe = f"cpe:2.3:a:sqlite:sqlite:{version}:*:*:*:*:*:*:*"
    advisories = []
    start_index = 0
    while True:
        query = urllib.parse.urlencode(
            {
                "cpeName": cpe,
                "startIndex": start_index,
                "resultsPerPage": 2000,
            }
        )
        response = request_json(f"{NVD_CVE_API}?{query}")
        for entry in response.get("vulnerabilities", []):
            cve = entry["cve"]
            if sqlite_cpe_matches(cve):
                advisories.append(cve["id"])
        page_size = response.get("resultsPerPage", 0)
        if page_size == 0 and response.get("totalResults", 0) > start_index:
            raise ValueError("NVD returned an empty page before the end of its results")
        start_index += page_size
        if start_index >= response.get("totalResults", 0):
            break
    return sorted(set(advisories))


def main():
    sources_path = ROOT / "scripts/windows/owned-libmpv/sources.lock.json"
    cargo_lock = ROOT / "Cargo.lock"
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    sources = load_sources(sources_path)
    sqlite_crate_version, sqlite_version = load_sqlite_version(cargo_lock, cargo_home)

    print(f"Checking {len(sources)} pinned native source commits with OSV")
    git_advisories = query_git_advisories(sources)
    print(f"Checking bundled SQLite {sqlite_version} from libsqlite3-sys {sqlite_crate_version} with NVD")
    sqlite_advisories = query_sqlite_advisories(sqlite_version)

    for source, advisories in git_advisories.items():
        print(f"OSV advisories for {source} ({len(advisories)}):")
        for advisory in sorted(advisories):
            print(f"  {advisory} https://osv.dev/vulnerability/{advisory}")
    for advisory in sqlite_advisories:
        print(f"NVD advisory: bundled SQLite {sqlite_version}: {advisory} https://nvd.nist.gov/vuln/detail/{advisory}")

    if git_advisories or sqlite_advisories:
        print("Advisories require applicability review before changing or approving a native bundle.")
        return 1
    print("No advisories matched the queried source commits or SQLite CPE.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"native advisory check failed: {error}", file=sys.stderr)
        raise SystemExit(2)
