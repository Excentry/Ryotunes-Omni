use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ryotunes_core::db::Db;
use ryotunes_core::spotify_import::*;
use ryotunes_core::spotify_metadata::{Authorization, MetadataSession, SourceTrack};
use tauri::Manager;

pub const LAB_IDENTIFIER: &str = "dev.excentry.ryotune-omni.import-lab";

#[derive(Clone)]
struct Connected {
    session: MetadataSession,
    owner: ImportOwner,
}

pub struct ImportLab {
    jobs: SpotifyImportJobs,
    db: Arc<Db>,
    connected: Mutex<Option<Connected>>,
    sources: Mutex<HashMap<String, Vec<SourceTrack>>>,
    operation: tokio::sync::Mutex<()>,
    generation: tokio::sync::watch::Sender<u64>,
}

impl ImportLab {
    pub fn new(db: Arc<Db>) -> Self {
        let (generation, _) = tokio::sync::watch::channel(0);
        Self {
            jobs: SpotifyImportJobs::new(db.clone()),
            db,
            connected: Mutex::new(None),
            sources: Mutex::new(HashMap::new()),
            operation: tokio::sync::Mutex::new(()),
            generation,
        }
    }

    pub fn disconnect(&self) -> Result<(), String> {
        let mut connected = self.connected.lock().map_err(|_| "Import session unavailable.")?;
        self.generation.send_modify(|generation| *generation = generation.wrapping_add(1));
        if let Some(previous) = connected.take() {
            self.jobs.invalidate(&previous.owner).map_err(|error| error.to_string())?;
        }
        self.sources.lock().map_err(|_| "Import sources unavailable.")?.clear();
        Ok(())
    }

    fn session(&self) -> Result<Connected, String> {
        self.connected
            .lock()
            .map_err(|_| "Import session unavailable.")?
            .clone()
            .ok_or_else(|| "Connect Spotify for this test session first.".into())
    }

    fn verify(
        &self,
        expected: &Connected,
    ) -> Result<std::sync::MutexGuard<'_, Option<Connected>>, String> {
        let guard = self.connected.lock().map_err(|_| "Import session unavailable.")?;
        if guard.as_ref().map(|value| &value.owner) != Some(&expected.owner) {
            return Err("Spotify session changed. Start a new import.".into());
        }
        Ok(guard)
    }
}

fn authorized(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<Arc<ImportLab>, String> {
    let url = window.url().map_err(|_| "Could not verify the import surface.")?;
    let local = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"))
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && url.host_str() == Some("localhost")
            && url.port() == Some(5183));
    if window.label() != "main" || app.config().identifier != LAB_IDENTIFIER || !local {
        return Err("Spotify import is available only in the isolated import lab.".into());
    }
    app.try_state::<Arc<ImportLab>>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "Import lab is not initialized.".into())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabStatus {
    enabled: bool,
    connected: bool,
    max_occurrences: usize,
    review_jobs: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabReview {
    review: ImportReview,
    sources: Vec<SourceTrack>,
    receipt: Option<ImportReceipt>,
}

#[cfg(target_os = "windows")]
async fn open_authorization(window: &tauri::WebviewWindow, url: &str) -> Result<(), String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    if !url.starts_with("https://accounts.spotify.com/authorize?") || url.contains('\0') {
        return Err("Invalid Spotify authorization destination.".into());
    }
    let target: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window.run_on_main_thread(move || {
        let instance = unsafe {
            ShellExecuteW(
                None, w!("open"), PCWSTR(target.as_ptr()), PCWSTR::null(),
                PCWSTR::null(), SW_SHOWNORMAL,
            )
        };
        let code = instance.0 as isize;
        let result = if code > 32 { Ok(()) } else {
            Err(format!("Windows could not open the default browser (code {code}). Check the HTTPS default app."))
        };
        let _ = sender.send(result);
    }).map_err(|_| "Could not schedule browser launch.")?;
    receiver.await.map_err(|_| "Browser launch was interrupted.")?
}

#[cfg(not(target_os = "windows"))]
async fn open_authorization(_window: &tauri::WebviewWindow, url: &str) -> Result<(), String> {
    ryotunes_core::lastfm::open_browser(url)
}

#[tauri::command]
pub fn spotify_import_status(app: tauri::AppHandle, window: tauri::WebviewWindow) -> LabStatus {
    match authorized(&app, &window) {
        Ok(lab) => {
            let connected = lab.session().ok();
            let review_jobs = connected
                .as_ref()
                .and_then(|session| lab.jobs.review_job_ids(&session.owner).ok())
                .unwrap_or_default();
            LabStatus {
                enabled: true,
                connected: connected.is_some(),
                max_occurrences: ryotunes_core::spotify_metadata::MAX_PLAYLIST_OCCURRENCES,
                review_jobs,
            }
        }
        Err(_) => LabStatus {
            enabled: false,
            connected: false,
            max_occurrences: ryotunes_core::spotify_metadata::MAX_PLAYLIST_OCCURRENCES,
            review_jobs: Vec::new(),
        },
    }
}

#[tauri::command]
pub async fn spotify_import_connect(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let lab = authorized(&app, &window)?;
    let _operation =
        lab.operation.try_lock().map_err(|_| "Another import operation is still running.")?;
    lab.disconnect()?;
    let mut generation = lab.generation.subscribe();
    let expected = *generation.borrow_and_update();
    let authorization = Authorization::begin().await?;
    if *lab.generation.borrow() != expected {
        return Err("Spotify authorization cancelled.".into());
    }
    open_authorization(&window, &authorization.url).await?;
    let session = tokio::select! {
        biased;
        _ = generation.changed() => return Err("Spotify authorization cancelled.".into()),
        result = authorization.finish() => result?,
    };
    let mut connected = lab.connected.lock().map_err(|_| "Import session unavailable.")?;
    if *lab.generation.borrow() != expected {
        return Err("Spotify authorization cancelled.".into());
    }
    let owner = ImportOwner {
        window_key: format!("main:{:032x}", rand::random::<u128>()),
        account_key: session.account_key.clone(),
    };
    *connected = Some(Connected { session, owner });
    Ok(())
}

#[tauri::command]
pub fn spotify_import_disconnect(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    authorized(&app, &window)?.disconnect()
}

#[tauri::command]
pub async fn spotify_import_start(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    playlist_url: String,
) -> Result<LabReview, String> {
    let lab = authorized(&app, &window)?;
    let _operation =
        lab.operation.try_lock().map_err(|_| "Another import operation is still running.")?;
    let mut generation = lab.generation.subscribe();
    let connected = lab.session()?;
    let source = tokio::select! {
        biased;
        _ = generation.changed() => return Err("Spotify playlist read cancelled.".into()),
        result = connected.session.playlist(&playlist_url) => result?,
    };
    let _session = lab.verify(&connected)?;
    let started = lab
        .jobs
        .start(
            connected.owner.clone(),
            ImportSource {
                account_key: connected.owner.account_key.clone(),
                playlist_id: source.id,
                snapshot_id: source.snapshot,
                total: source.tracks.len(),
            },
            source.title,
        )
        .map_err(|error| error.to_string())?;
    lab.jobs
        .begin_matching(&connected.owner, &started.job_id)
        .map_err(|error| error.to_string())?;
    let entries = source
        .tracks
        .iter()
        .enumerate()
        .map(|(ordinal, track)| ImportOccurrence {
            occurrence_id: format!("{}:{ordinal}", started.job_id),
            ordinal,
            outcome: track.outcome,
            candidates: Vec::new(),
        })
        .collect();
    let review = lab
        .jobs
        .offer_review(&connected.owner, &started.job_id, entries)
        .map_err(|error| error.to_string())?;
    lab.sources
        .lock()
        .map_err(|_| "Import sources unavailable.")?
        .insert(review.job_id.clone(), source.tracks.clone());
    Ok(LabReview { review, sources: source.tracks, receipt: None })
}

#[tauri::command]
pub fn spotify_import_snapshot(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    job_id: String,
) -> Result<LabReview, String> {
    let lab = authorized(&app, &window)?;
    let connected = lab.session()?;
    let _session = lab.verify(&connected)?;
    let review = lab.jobs.snapshot(&connected.owner, &job_id).map_err(|error| error.to_string())?;
    let sources = lab
        .sources
        .lock()
        .map_err(|_| "Import sources unavailable.")?
        .get(&job_id)
        .cloned()
        .ok_or("Import source is no longer available.")?;
    let receipt = if review.phase == ImportPhase::Committed {
        lab.db
            .spotify_import_receipt(&connected.owner.account_key, &job_id)
            .map_err(|error| error.to_string())?
    } else {
        None
    };
    Ok(LabReview { review, sources, receipt })
}

fn duration_ms(value: Option<&str>) -> Option<u64> {
    value?
        .split(':')
        .try_fold(0_u64, |total, part| {
            total.checked_mul(60)?.checked_add(part.parse::<u64>().ok()?)
        })?
        .checked_mul(1000)
        .filter(|value| *value > 0)
}

fn candidate_artists(song: &innertube::SongItem) -> Vec<String> {
    let artists: Vec<_> = song
        .artist_runs
        .iter()
        .filter(|run| run.id.is_some() && !run.text.trim().is_empty())
        .map(|run| run.text.clone())
        .collect();
    if artists.is_empty() {
        vec![song.artists.clone()]
    } else {
        artists
    }
}

#[tauri::command]
pub async fn spotify_import_search(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<crate::AppState>>,
    job_id: String,
    occurrence_id: String,
    expected_revision: u64,
    query: String,
) -> Result<ImportReview, String> {
    let lab = authorized(&app, &window)?;
    let _operation =
        lab.operation.try_lock().map_err(|_| "Another import operation is still running.")?;
    if query.trim().is_empty() || query.len() > 500 {
        return Err("Enter a search of 1 to 500 bytes.".into());
    }
    let mut generation = lab.generation.subscribe();
    let connected = lab.session()?;
    let current =
        lab.jobs.snapshot(&connected.owner, &job_id).map_err(|error| error.to_string())?;
    if current.phase != ImportPhase::Review || current.revision != expected_revision {
        return Err("Review changed. Reload this import.".into());
    }
    let _occurrence = current
        .occurrences
        .iter()
        .find(|entry| entry.occurrence_id == occurrence_id)
        .ok_or("Unknown source occurrence.")?;
    let songs = tokio::select! {
        biased;
        _ = generation.changed() => return Err("Candidate search cancelled.".into()),
        result = state.search(&query) => result.map_err(|_| "YouTube Music search failed. Retry or skip this entry.")?,
    };
    let candidates = songs
        .into_iter()
        .filter(|song| !song.title.trim().is_empty() && !song.artists.trim().is_empty())
        .take(5)
        .map(|song| {
            let artists = candidate_artists(&song);
            ImportCandidate {
                candidate_id: format!("candidate-{:032x}", rand::random::<u128>()),
                video_id: song.video_id,
                title: song.title,
                artists,
                duration_ms: duration_ms(song.duration.as_deref()),
            }
        })
        .collect();
    let _session = lab.verify(&connected)?;
    lab.jobs
        .offer_candidates(&connected.owner, &job_id, &occurrence_id, expected_revision, candidates)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn spotify_import_commit(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    request: ImportCommitRequest,
) -> Result<ImportReceipt, String> {
    let lab = authorized(&app, &window)?;
    let connected = lab.session()?;
    let _session = lab.verify(&connected)?;
    lab.jobs.commit(&connected.owner, &request).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn spotify_import_cancel(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    job_id: String,
) -> Result<ImportReview, String> {
    let lab = authorized(&app, &window)?;
    let connected = lab.session()?;
    let _session = lab.verify(&connected)?;
    let result = lab.jobs.cancel(&connected.owner, &job_id).map_err(|error| error.to_string())?;
    lab.jobs.release(&connected.owner, &job_id).map_err(|error| error.to_string())?;
    lab.sources.lock().map_err(|_| "Import sources unavailable.")?.remove(&job_id);
    Ok(result)
}

#[tauri::command]
pub fn spotify_import_readback(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    job_id: String,
) -> Result<Vec<ryotunes_core::db::spotify_import::ImportedPlaylistEntry>, String> {
    let lab = authorized(&app, &window)?;
    let connected = lab.session()?;
    let _session = lab.verify(&connected)?;
    let snapshot =
        lab.jobs.snapshot(&connected.owner, &job_id).map_err(|error| error.to_string())?;
    if snapshot.phase != ImportPhase::Committed {
        return Err("This import has not committed.".into());
    }
    let receipt = lab
        .db
        .spotify_import_receipt(&connected.owner.account_key, &job_id)
        .map_err(|error| error.to_string())?
        .ok_or("Import receipt is missing.")?;
    lab.db.imported_playlist_entries(&receipt.playlist_id).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_credits_preserve_linked_artists_without_splitting_band_names() {
        let song: innertube::SongItem = serde_json::from_value(serde_json::json!({
            "video_id":"aaaaaaaaaaa","title":"Song","artists":"AC/DC & Another Artist",
            "artist_runs":[
                {"text":"AC/DC","id":"UC-band"},
                {"text":" & "},
                {"text":"Another Artist","id":"UC-artist"}
            ]
        }))
        .unwrap();
        assert_eq!(candidate_artists(&song), vec!["AC/DC", "Another Artist"]);
        let without_runs = innertube::SongItem { artist_runs: Vec::new(), ..song };
        assert_eq!(candidate_artists(&without_runs), vec!["AC/DC & Another Artist"]);
    }

    #[test]
    fn candidate_duration_is_checked_and_handles_hour_fields() {
        assert_eq!(duration_ms(Some("1:02:03")), Some(3_723_000));
        assert_eq!(duration_ms(Some("3:00")), Some(180_000));
        assert_eq!(duration_ms(Some("live")), None);
        assert_eq!(duration_ms(Some("0:00")), None);
        assert_eq!(duration_ms(Some("18446744073709551615:00")), None);
    }

    #[test]
    fn disconnect_invalidates_pending_authorization_generation() {
        let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
        let lab = ImportLab::new(db);
        let mut generation = lab.generation.subscribe();
        lab.disconnect().unwrap();
        assert!(generation.has_changed().unwrap());
        assert_eq!(*generation.borrow_and_update(), 1);
        assert!(lab.session().is_err());
    }
}
