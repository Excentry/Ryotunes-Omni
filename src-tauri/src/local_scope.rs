//! Tauri asset-protocol scope for local artwork. The webview may fetch only the cover files the
//! core hands it, so these host-side calls open exactly those paths and nothing else. Kept out of
//! `ryotunes_core::local` because `asset_protocol_scope` is a Tauri surface.

use std::collections::HashSet;
use std::path::Path;

use innertube::SongItem;
use tauri::{AppHandle, Manager};

use ryotunes_core::db::Db;
use ryotunes_core::host::Paths;

/// Let the webview fetch exactly the artwork we hand it, and nothing else.
///
/// The asset protocol's static scope is empty on purpose: it can't name folders the user picks at
/// runtime, and the usual `"**"` shortcut would put every file on the machine behind a URL the page
/// could fetch. Both the stored path and its canonical form are allowed, because the scope check
/// canonicalizes what it is asked about — without that, a music folder that is a symlink to an
/// external drive gets its covers refused.
pub fn allow_covers(app: &AppHandle, songs: &[SongItem]) {
    let scope = app.asset_protocol_scope();
    let mut seen: HashSet<&str> = HashSet::new();
    for cover in songs.iter().filter_map(|s| s.thumbnail.as_deref()) {
        if !seen.insert(cover) {
            continue;
        }
        let _ = scope.allow_file(cover);
        if let Ok(real) = Path::new(cover).canonicalize() {
            let _ = scope.allow_file(real);
        }
    }
}

/// Allow only Ryotunes-owned cover storage plus individual legacy cover files at startup. The v5
/// rescan migrates those legacy sidecars into owned storage; watched music directories themselves
/// are never recursively exposed to the WebKit asset protocol.
pub fn allow_music_paths(app: &AppHandle, db: &Db, paths: &Paths) {
    let scope = app.asset_protocol_scope();

    // The only recursive renderer-visible directory is owned by Ryotunes itself. Individual
    // historical sidecar covers from v2.4 are allowed just long enough for the v5 rescan to copy
    // them here; the watched music directories themselves are never recursively web-visible.
    let covers = paths.covers_dir();
    let _ = scope.allow_directory(&covers, true);
    if let Ok(real) = covers.canonicalize() {
        let _ = scope.allow_directory(real, true);
    }
    let mut seen: HashSet<String> = HashSet::new();
    for cover in db.local_tracks(None).into_iter().filter_map(|track| track.cover) {
        if Path::new(&cover).starts_with(&covers) || !seen.insert(cover.clone()) {
            continue;
        }
        let _ = scope.allow_file(&cover);
        if let Ok(real) = Path::new(&cover).canonicalize() {
            let _ = scope.allow_file(real);
        }
    }
}
