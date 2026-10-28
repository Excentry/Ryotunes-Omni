//! Explicit, package-managed updates shared by both desktop hosts. GitHub is the release
//! authority; the renderer can select a version, never a URL, command or destination.
//!
//! Arch installs the release package with pacman. Void installs the same package, repacked
//! locally into an .xbps.
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const REPOSITORY: &str = "https://github.com/Excentry/Ryotune-Omni";
const LATEST: &str = "https://api.github.com/repos/Excentry/Ryotune-Omni/releases/latest";
const MAX_PACKAGE: u64 = 1024 * 1024 * 1024;
static INSTALL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const VOID_DEPENDS: &str = include_str!("../../../packaging/void/depends");

#[derive(Clone, Copy, Debug, PartialEq)]
enum Manager {
    Pacman,
    Xbps,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub release_url: String,
    pub notes: String,
    pub can_install: bool,
    pub unsupported_reason: Option<String>,
    /// The latest release is already installed on disk (pacman upgraded the files under this
    /// running daemon); only a restart is left to do.
    pub restart_pending: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledUpdate {
    pub version: String,
    pub restart_required: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
}

// Canonical v1 versions only: no traversal, prereleases, leading zeroes or legacy v2 tags.
fn version(raw: &str) -> Option<(u64, u64, u64)> {
    let mut parts = raw.split('.');
    let (major, minor, patch) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || major != "1" {
        return None;
    }
    if (minor.len() > 1 && minor.starts_with('0'))
        || !minor.bytes().all(|byte| byte.is_ascii_digit())
        || patch.len() != 1
        || !patch.as_bytes()[0].is_ascii_digit()
    {
        return None;
    }
    Some((1, minor.parse::<u64>().ok()?, (patch.as_bytes()[0] - b'0') as u64))
}

fn package_name(version: &str) -> String {
    format!("ryotune-omni-{version}-1-x86_64.pkg.tar.zst")
}

fn release_info(release: Option<Release>, current: &str, reason: Option<String>) -> UpdateInfo {
    let mut info = UpdateInfo {
        current_version: current.into(),
        latest_version: None,
        available: false,
        release_url: format!("{REPOSITORY}/releases"),
        notes: String::new(),
        can_install: false,
        unsupported_reason: reason,
        restart_pending: false,
    };
    let Some(release) = release else { return info };
    let Some(latest) = release.tag_name.strip_prefix('v') else { return info };
    let Some(next) = version(latest) else { return info };
    if release.draft || release.prerelease {
        return info;
    }
    info.latest_version = Some(latest.into());
    // v1 is a deliberate new release series, not a numeric downgrade of the legacy v2 line.
    info.available = version(current).is_none_or(|installed| next > installed);
    info.release_url = format!("{REPOSITORY}/releases/tag/v{latest}");
    info.notes = release.body.unwrap_or_default();
    let name = package_name(latest);
    let checksum = format!("{name}.sha256");
    let has_package =
        release.assets.iter().any(|a| a.name == name && a.size > 0 && a.size <= MAX_PACKAGE);
    let has_checksum =
        release.assets.iter().any(|a| a.name == checksum && a.size > 0 && a.size <= 4096);
    if !has_package || !has_checksum {
        info.unsupported_reason = Some("This release has no complete x86_64 Arch package and checksum. View the changelog or check again after the release build finishes.".into());
    }
    info.can_install = info.unsupported_reason.is_none() && has_package && has_checksum;
    info
}

async fn response(url: &str, timeout: Duration) -> Result<reqwest::Response, String> {
    crate::http::client()
        .get(url)
        .header(reqwest::header::USER_AGENT, concat!("Ryotune-Omni/", env!("CARGO_PKG_VERSION")))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("Could not contact GitHub: {e}"))
}

async fn bounded_body(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, String> {
    response = response.error_for_status().map_err(|e| format!("GitHub request failed: {e}"))?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err("GitHub response exceeded the allowed size.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn installation() -> Result<Manager, String> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("Self update supports the x86_64 Arch and Void Linux packages. Download a release for your platform instead.".into());
    }
    let manager = if Path::new("/usr/bin/pacman").is_file() {
        Manager::Pacman
    } else if Path::new("/usr/bin/xbps-install").is_file() {
        Manager::Xbps
    } else {
        return Err("Self update needs pacman or xbps.".into());
    };
    if !Path::new("/usr/bin/pkexec").is_file() {
        return Err(
            "Self update needs polkit (pkexec), with a desktop authentication agent.".into()
        );
    }
    let exe = std::env::current_exe()
        .map(on_disk)
        .map_err(|e| format!("Cannot determine the running installation: {e}"))?;
    let mut owner = match manager {
        Manager::Pacman => Command::new("/usr/bin/pacman"),
        Manager::Xbps => Command::new("/usr/bin/xbps-query"),
    };
    owner.arg(if manager == Manager::Pacman { "-Qoq" } else { "-o" }).arg(exe).kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(10), owner.output()).await {
        Ok(Ok(output)) if output.status.success() && owns(manager, &output.stdout) => Ok(manager),
        _ => Err("This copy of Ryotunes Omni is not installed from its package, so it cannot update itself. Install the Omni package from its release page; development builds are never overwritten.".into()),
    }
}

/// Whether owner query names Ryotunes Omni, never the upstream Ryotunes package.
fn owns(manager: Manager, stdout: &[u8]) -> bool {
    match manager {
        Manager::Pacman => stdout == b"ryotune-omni\n",
        Manager::Xbps => std::str::from_utf8(stdout)
            .ok()
            .and_then(|text| text.split_once(": "))
            .is_some_and(|(pkgver, _)| xbps_version(pkgver).is_some()),
    }
}

/// The running binary's path on disk. Once pacman replaces the file under a running daemon, the
/// kernel reports `/usr/bin/ryotunesd (deleted)`; the package still owns the original path.
fn on_disk(exe: PathBuf) -> PathBuf {
    match exe.to_str().and_then(|path| path.strip_suffix(" (deleted)")) {
        Some(path) => PathBuf::from(path),
        None => exe,
    }
}

/// The installed package's version, which runs ahead of this daemon once the package manager (a
/// system upgrade, or an update installed from another window) has replaced the files under it.
async fn installed_package_version(manager: Manager) -> Result<Option<String>, String> {
    let mut query = match manager {
        Manager::Pacman => Command::new("/usr/bin/pacman"),
        Manager::Xbps => Command::new("/usr/bin/xbps-query"),
    };
    match manager {
        Manager::Pacman => query.args(["-Q", "ryotune-omni"]),
        Manager::Xbps => query.args(["-p", "pkgver", "ryotune-omni"]),
    };
    let output = tokio::time::timeout(Duration::from_secs(10), query.kill_on_drop(true).output())
        .await
        .map_err(|_| "Timed out reading the installed package.")?
        .map_err(|e| format!("Could not query the installed package: {e}"))?;
    if !output.status.success() {
        return Err("Could not query the installed Ryotunes Omni package.".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(match manager {
        Manager::Pacman => package_version(&text),
        Manager::Xbps => xbps_version(&text),
    })
}

/// `ryotune-omni 1.1.9-1` → `1.1.9`.
fn package_version(query: &str) -> Option<String> {
    query
        .trim()
        .strip_prefix("ryotune-omni ")
        .and_then(|value| value.rsplit_once('-').map(|(version, _)| version.to_owned()))
}

/// `ryotune-omni-1.2.1_1` → `1.2.1`.
fn xbps_version(pkgver: &str) -> Option<String> {
    let (version, revision) = pkgver.trim().strip_prefix("ryotune-omni-")?.rsplit_once('_')?;
    let revision_ok = !revision.is_empty() && revision.bytes().all(|byte| byte.is_ascii_digit());
    (revision_ok && !version.contains('-')).then(|| version.to_owned())
}

async fn check() -> Result<(UpdateInfo, Option<Manager>), String> {
    let response = response(LATEST, Duration::from_secs(30)).await?;
    let release = if response.status() == reqwest::StatusCode::NOT_FOUND {
        None
    } else {
        let body = bounded_body(response, 1024 * 1024).await?;
        Some(
            serde_json::from_slice::<Release>(&body)
                .map_err(|e| format!("Invalid GitHub release metadata: {e}"))?,
        )
    };
    let installation = installation().await;
    let mut info =
        release_info(release, env!("CARGO_PKG_VERSION"), installation.as_ref().err().cloned());
    let manager = installation.ok();
    if let (true, true, Some(manager)) = (info.available, info.can_install, manager) {
        info.restart_pending = installed_package_version(manager).await.ok().flatten().as_deref()
            == info.latest_version.as_deref();
    }
    Ok((info, manager))
}

pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    check().await.map(|(info, _)| info)
}

fn checksum(bytes: &[u8], name: &str) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Invalid package checksum encoding.")?;
    let mut fields = text.split_whitespace();
    let hash = fields.next().unwrap_or_default();
    if fields.next() != Some(name)
        || fields.next().is_some()
        || hash.len() != 64
        || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(
            "Invalid package checksum: expected one SHA-256 entry for this release package.".into(),
        );
    }
    Ok(hash.to_ascii_lowercase())
}

async fn download(url: &str, path: &Path, expected: &str) -> Result<(), String> {
    let mut response = response(url, Duration::from_secs(15 * 60))
        .await?
        .error_for_status()
        .map_err(|e| format!("Package download failed: {e}"))?;
    if response.content_length().is_some_and(|length| length > MAX_PACKAGE) {
        return Err("Release package is too large.".into());
    }
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .map_err(|e| format!("Could not stage update: {e}"))?;
    let mut size = 0u64;
    let mut hash = Sha256::new();
    while let Some(chunk) =
        response.chunk().await.map_err(|e| format!("Package download interrupted: {e}"))?
    {
        size += chunk.len() as u64;
        if size > MAX_PACKAGE {
            return Err("Release package is too large.".into());
        }
        hash.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| format!("Could not write update: {e}"))?;
    }
    if size == 0 || format!("{:x}", hash.finalize()) != expected {
        return Err("Package SHA-256 verification failed. Nothing was installed.".into());
    }
    file.sync_all().await.map_err(|e| format!("Could not save update: {e}"))?;
    Ok(())
}

pub async fn install_update(requested: String) -> Result<InstalledUpdate, String> {
    if version(&requested).is_none() {
        return Err("Select a valid v1 release.".into());
    }
    let _guard = INSTALL.try_lock().map_err(|_| "An update is already in progress.")?;
    let (info, manager) = check().await?;
    if info.latest_version.as_deref() != Some(&requested) {
        return Err(
            "The latest release changed. Check for new versions again before updating.".into()
        );
    }
    if !info.available {
        return Err("This version is already running, or a newer version is installed.".into());
    }
    let (true, Some(manager)) = (info.can_install, manager) else {
        return Err(info
            .unsupported_reason
            .unwrap_or_else(|| "Self update is not available for this installation.".into()));
    };
    // A system upgrade may already have replaced the files while this older daemon kept running.
    if let Some(installed_version) = installed_package_version(manager).await? {
        if version(&installed_version) > version(&requested) {
            return Err(
                "A newer package is already installed. Quit Ryotunes Omni and open it again."
                    .into(),
            );
        }
        if installed_version == requested {
            return Ok(InstalledUpdate { version: requested, restart_required: true });
        }
    }
    // Private random staging, removed on every success/error. No user-controlled path reaches root.
    let staging = tempfile::Builder::new()
        .prefix("ryotune-omni-update-")
        .tempdir()
        .map_err(|e| format!("Could not create private update staging: {e}"))?;
    let name = package_name(&requested);
    let url = format!("{REPOSITORY}/releases/download/v{requested}/{name}");
    let expected = checksum(
        &bounded_body(response(&format!("{url}.sha256"), Duration::from_secs(30)).await?, 4096)
            .await?,
        &name,
    )?;
    let path = staging.path().join(&name);
    download(&url, &path, &expected).await?;
    match manager {
        Manager::Pacman => install_pacman(&path, &requested).await?,
        Manager::Xbps => install_xbps(&path, staging.path(), &requested).await?,
    }
    Ok(InstalledUpdate { version: requested, restart_required: true })
}

async fn install_pacman(path: &Path, requested: &str) -> Result<(), String> {
    // Verify the package's internal identity, not just its download filename, before privilege.
    let metadata = Command::new("/usr/bin/pacman").arg("-Qp").arg(path).kill_on_drop(true).output();
    let output = tokio::time::timeout(Duration::from_secs(30), metadata)
        .await
        .map_err(|_| "Timed out reading the package identity.")?
        .map_err(|e| format!("Could not inspect the package: {e}"))?;
    let expected_identity = format!("ryotune-omni {requested}-1\n");
    if !output.status.success() || output.stdout != expected_identity.as_bytes() {
        return Err(
            "Downloaded package identity does not match the selected Ryotunes Omni release.".into(),
        );
    }
    // Explicit Update click + polkit approval authorizes just this package transaction. Retain
    // pacman's signature/dependency policy; never use --overwrite, --nodeps or bypass signatures.
    // Do not kill pacman on a timeout: interrupting a committed transaction can damage the install.
    let output = Command::new("/usr/bin/pkexec")
        .arg("/usr/bin/pacman")
        .args(["-U", "--noconfirm"])
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not start the administrator approval: {e}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Update was cancelled or pacman could not install the package ({}). {}",
            output.status,
            detail.trim()
        ));
    }
    Ok(())
}

/// Repack the verified Arch package as `ryotune-omni-<version>_1` in a private local repository and
/// install it from there. xbps-create and xbps-rindex run unprivileged; only the final
/// xbps-install goes through polkit.
async fn install_xbps(package: &Path, staging: &Path, requested: &str) -> Result<(), String> {
    let root = staging.join("root");
    let repo = staging.join("repo");
    let (from, into, wanted) = (package.to_owned(), root.clone(), requested.to_owned());
    tokio::task::spawn_blocking(move || unpack(&from, &into, &wanted))
        .await
        .map_err(|e| format!("Could not unpack the package: {e}"))??;
    refuse_unrunnable(&root.join("usr/bin")).await?;
    tokio::fs::create_dir(&repo).await.map_err(|e| format!("Could not stage update: {e}"))?;

    let pkgver = format!("ryotune-omni-{requested}_1");
    let depends = void_depends();
    run(
        Command::new("/usr/bin/xbps-create")
            .current_dir(&repo)
            .args(["-q", "-A", "x86_64", "-n", &pkgver, "-l", "GPL-3.0-or-later", "-H", REPOSITORY])
            .args(["-s", "Ryotunes Omni music player and playback daemon"])
            .args(["-D", &depends])
            .arg(&root),
        "build the xbps package",
    )
    .await?;
    run(
        Command::new("/usr/bin/xbps-rindex")
            .arg("-a")
            .arg(repo.join(format!("{pkgver}.x86_64.xbps"))),
        "index the xbps package",
    )
    .await?;

    // -S refreshes the distro repositories so new dependencies resolve. Like pacman above, never
    // kill a running transaction.
    let output = Command::new("/usr/bin/pkexec")
        .arg("/usr/bin/xbps-install")
        .args(["-S", "-y", "-u"])
        .arg(format!("--repository={}", repo.display()))
        .arg("ryotune-omni")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not start the administrator approval: {e}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Update was cancelled or xbps could not install the package ({}). {}",
            output.status,
            detail.trim()
        ));
    }
    Ok(())
}

/// Catch a release built against a newer glibc than this system has before it replaces a
/// working install. Missing libraries are fine here: xbps installs new dependencies.
async fn refuse_unrunnable(bin: &Path) -> Result<(), String> {
    let mut entries =
        tokio::fs::read_dir(bin).await.map_err(|e| format!("Could not check the package: {e}"))?;
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let output = tokio::time::timeout(
            Duration::from_secs(30),
            Command::new("/usr/bin/ldd").arg(entry.path()).kill_on_drop(true).output(),
        )
        .await
        .map_err(|_| "Timed out checking the package binaries.")?
        .map_err(|e| format!("Could not check the package binaries: {e}"))?;
        if needs_newer_glibc(&output.stdout) || needs_newer_glibc(&output.stderr) {
            return Err(format!(
                "This release needs a newer glibc than this system has ({}). Nothing was installed.",
                entry.file_name().to_string_lossy()
            ));
        }
    }
    Ok(())
}

/// ldd: `/usr/lib/libm.so.6: version `GLIBC_2.44' not found (required by ./ryotunesd)`.
fn needs_newer_glibc(ldd: &[u8]) -> bool {
    String::from_utf8_lossy(ldd)
        .lines()
        .any(|line| line.contains("version `GLIBC_") && line.contains("not found"))
}

async fn run(command: &mut Command, what: &str) -> Result<(), String> {
    let output =
        tokio::time::timeout(Duration::from_secs(300), command.kill_on_drop(true).output())
            .await
            .map_err(|_| format!("Timed out trying to {what}."))?
            .map_err(|e| format!("Could not {what}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Could not {what}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// `packaging/void/depends` as an xbps-create `-D` list.
fn void_depends() -> String {
    VOID_DEPENDS
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|name| format!("{name}>=0"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Unpack the package's `usr/` tree into `root`, then check `.PKGINFO` names this exact release.
/// The systemd units are left out: Void has no user systemd, and `ryotunes` starts the daemon
/// itself when it can't use the socket unit.
fn unpack(package: &Path, root: &Path, requested: &str) -> Result<(), String> {
    let bad = |e: std::io::Error| format!("Could not unpack the package: {e}");
    let file = std::fs::File::open(package).map_err(bad)?;
    let decoder = ruzstd::decoding::StreamingDecoder::new(std::io::BufReader::new(file))
        .map_err(|e| format!("Could not unpack the package: {e}"))?;
    std::fs::create_dir(root).map_err(bad)?;
    let mut archive = tar::Archive::new(decoder);
    let mut pkginfo = String::new();
    for entry in archive.entries().map_err(bad)? {
        let mut entry = entry.map_err(bad)?;
        let path = entry.path().map_err(bad)?.into_owned();
        if path == Path::new(".PKGINFO") {
            entry.read_to_string(&mut pkginfo).map_err(bad)?;
        } else if path.starts_with("usr")
            && !path.starts_with("usr/lib/systemd")
            && !entry.unpack_in(root).map_err(bad)?
        {
            return Err("The package contains a path outside its install root.".into());
        }
    }
    if !pkginfo_matches(&pkginfo, requested) {
        return Err(
            "Downloaded package identity does not match the selected Ryotunes Omni release.".into(),
        );
    }
    Ok(())
}

fn pkginfo_matches(pkginfo: &str, requested: &str) -> bool {
    let field =
        |key: &str| pkginfo.lines().find_map(|line| line.strip_prefix(key)?.strip_prefix(" = "));
    field("pkgname") == Some("ryotune-omni")
        && field("pkgver") == Some(format!("{requested}-1").as_str())
        && field("arch") == Some("x86_64")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> Release {
        let name = package_name(tag.trim_start_matches('v'));
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            body: Some("Changes".into()),
            assets: vec![
                Asset { name: name.clone(), size: 100 },
                Asset { name: format!("{name}.sha256"), size: 120 },
            ],
        }
    }

    #[test]
    fn release_series_and_numeric_order() {
        assert!(release_info(Some(release("v1.1.0")), "1.0.9", None).available);
        assert!(!release_info(Some(release("v1.0.9")), "1.1.0", None).available);
        assert!(!release_info(Some(release("v1.1.0")), "1.1.0", None).available);
        assert!(release_info(Some(release("v1.0.0")), "2.5.1", None).available);
        assert!(release_info(Some(release("v2.5.1")), "1.0.0", None).latest_version.is_none());
        assert!(version("1.0.10").is_none());
        assert!(version("1.01.0").is_none());
        assert!(version("1.0.0/../../bad").is_none());
    }

    #[test]
    fn incomplete_or_unpublished_releases_cannot_install() {
        let mut value = release("v1.0.1");
        value.assets.pop();
        let info = release_info(Some(value), "1.0.0", None);
        assert!(info.available && !info.can_install);
        let mut value = release("v1.0.1");
        value.prerelease = true;
        assert!(release_info(Some(value), "1.0.0", None).latest_version.is_none());
        assert!(
            !release_info(Some(release("v1.0.1")), "1.0.0", Some("Unmanaged".into())).can_install
        );
    }

    #[tokio::test]
    async fn downloaded_bytes_must_match_and_never_replace_an_existing_file() {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/package", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0u8; 4096];
                stream.read(&mut request).await.unwrap();
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\npackage",
                    )
                    .await
                    .unwrap();
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("verified");
        let hash = format!("{:x}", Sha256::digest(b"package"));
        download(&url, &path, &hash).await.unwrap();
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"package");
        assert!(download(&url, &dir.path().join("corrupt"), &"0".repeat(64))
            .await
            .unwrap_err()
            .contains("SHA-256"));
        tokio::fs::write(&path, b"keep").await.unwrap();
        assert!(download(&url, &path, &hash).await.is_err());
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"keep");
        server.await.unwrap();
    }

    #[test]
    fn checksum_binds_exactly_one_asset() {
        let hash = "a".repeat(64);
        assert_eq!(checksum(format!("{hash}  package\n").as_bytes(), "package").unwrap(), hash);
        assert!(checksum(format!("{hash}  other\n").as_bytes(), "package").is_err());
        assert!(checksum(format!("{hash}  package\n{hash} other").as_bytes(), "package").is_err());
        assert!(checksum(b"not-a-hash package", "package").is_err());
    }

    #[test]
    fn a_replaced_binary_still_resolves_to_its_package_path() {
        let replaced = PathBuf::from("/usr/bin/ryotune-omni-daemon (deleted)");
        assert_eq!(on_disk(replaced), PathBuf::from("/usr/bin/ryotune-omni-daemon"));
        let intact = PathBuf::from("/usr/bin/ryotune-omni-daemon");
        assert_eq!(on_disk(intact.clone()), intact);
        assert_eq!(package_version("ryotune-omni 1.1.9-1\n").as_deref(), Some("1.1.9"));
        assert_eq!(package_version("ryotunes 1:1.1.9-1\n"), None);
    }

    #[test]
    fn xbps_owner_and_version_name_only_omni() {
        assert_eq!(xbps_version("ryotune-omni-1.2.1_1\n").as_deref(), Some("1.2.1"));
        assert_eq!(xbps_version("ryotune-omni-1.2.1"), None);
        assert_eq!(xbps_version("ryotunes-1.2.1_1"), None);
        assert_eq!(xbps_version("ryotune-omni-cli-1.2.1_1"), None);
        assert!(owns(
            Manager::Xbps,
            b"ryotune-omni-1.2.1_1: /usr/bin/ryotune-omni-daemon (regular file)\n"
        ));
        assert!(!owns(
            Manager::Xbps,
            b"ryotunes-1.2.1_1: /usr/bin/ryotune-omni-daemon (regular file)\n"
        ));
        assert!(!owns(Manager::Xbps, b""));
    }

    #[cfg(unix)]
    fn arch_package(pkginfo: &str) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut add = |path: &str, mode: u32, body: &[u8]| {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(mode);
            header.set_cksum();
            builder.append_data(&mut header, path, body).unwrap();
        };
        add(".PKGINFO", 0o644, pkginfo.as_bytes());
        add("usr/bin/ryotune-omni-daemon", 0o755, b"elf");
        add("usr/lib/systemd/user/ryotune-omni-daemon.service", 0o644, b"[Unit]");
        let tar = builder.into_inner().unwrap();
        ruzstd::encoding::compress_to_vec(&tar[..], ruzstd::encoding::CompressionLevel::Fastest)
    }

    #[cfg(unix)]
    #[test]
    fn unpack_keeps_usr_without_systemd_and_checks_identity() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let package = dir.path().join("ryotune-omni.pkg.tar.zst");
        let good =
            "pkgname = ryotune-omni\npkgbase = ryotune-omni\npkgver = 1.2.2-1\narch = x86_64\n";
        std::fs::write(&package, arch_package(good)).unwrap();

        let root = dir.path().join("root");
        unpack(&package, &root, "1.2.2").unwrap();
        let daemon = std::fs::metadata(root.join("usr/bin/ryotune-omni-daemon")).unwrap();
        assert_eq!(daemon.permissions().mode() & 0o777, 0o755);
        assert!(!root.join("usr/lib/systemd").exists());
        assert!(!root.join(".PKGINFO").exists());

        assert!(unpack(&package, &dir.path().join("other"), "1.2.3").is_err());
        std::fs::write(&package, arch_package(&good.replace("x86_64", "aarch64"))).unwrap();
        assert!(unpack(&package, &dir.path().join("arm"), "1.2.2").is_err());
    }
}
