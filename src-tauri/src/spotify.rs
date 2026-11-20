//! Spotify sign-in, catalogue selection and status for the Tauri host. Browsing goes through
//! `ryotunes_core::spotify_browse`; the event payloads match the Linux daemon's `spotify-auth`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ryotunes_core::spotify::Provider;
use ryotunes_core::spotify_browse::SpotifyBrowseError;
use serde_json::{json, Value};
use tauri::State;

use crate::state::AppState;

type St<'a> = State<'a, Arc<AppState>>;

const AUTHORIZE_PREFIX: &str = "https://accounts.spotify.com/authorize?";

static SIGNING_IN: AtomicBool = AtomicBool::new(false);
static AUTH_URL: Mutex<Option<String>> = Mutex::new(None);

/// Clears the in-flight flag however the sign-in task ends.
struct SignInGuard;

impl Drop for SignInGuard {
    fn drop(&mut self) {
        *AUTH_URL.lock().unwrap_or_else(|e| e.into_inner()) = None;
        SIGNING_IN.store(false, Ordering::SeqCst);
    }
}

pub(crate) fn browse_error(e: SpotifyBrowseError) -> String {
    e.to_string()
}

/// Only the catalogues this host can serve. SoundCloud and TIDAL are not wired on Windows yet.
fn host_provider(raw: &str) -> Result<Provider, String> {
    match Provider::parse(raw) {
        Some(p @ (Provider::Youtube | Provider::Spotify)) => Ok(p),
        Some(_) => Err(format!("{raw} is not available in this version.")),
        None => Err(format!("Unknown provider: {raw}")),
    }
}

async fn display_name(state: &AppState) -> Option<String> {
    let client = state.spotify.client().await?;
    client.profile().await.ok().map(|profile| profile.display_name)
}

/// `{signedIn, stored, name, premium, canStream}`. A live session implies Premium: the crate gates
/// both sign-in and restore on it.
async fn status_json(state: &AppState) -> Value {
    let client = state.spotify.client().await;
    let signed_in = client.is_some();
    let can_stream = client.as_ref().is_some_and(|c| c.can_stream());
    let name = if signed_in { display_name(state).await } else { None };
    json!({
        "signedIn": signed_in,
        "stored": state.spotify.stored(),
        "name": name,
        "premium": signed_in.then_some(true),
        "canStream": can_stream,
    })
}

#[cfg(target_os = "windows")]
async fn open_authorization(app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    if !url.starts_with(AUTHORIZE_PREFIX) || url.contains('\0') {
        return Err("Invalid Spotify authorization destination.".into());
    }
    let target: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let instance = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(target.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = instance.0 as isize;
        let result = if code > 32 {
            Ok(())
        } else {
            Err(format!("Windows could not open the default browser (code {code})."))
        };
        let _ = sender.send(result);
    })
    .map_err(|_| "Could not schedule the browser launch.")?;
    receiver.await.map_err(|_| "The browser launch was interrupted.")?
}

#[cfg(not(target_os = "windows"))]
async fn open_authorization(_app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    if !url.starts_with(AUTHORIZE_PREFIX) {
        return Err("Invalid Spotify authorization destination.".into());
    }
    ryotunes_core::lastfm::open_browser(url)
}

#[tauri::command]
pub async fn spotify_status(state: St<'_>) -> Result<Value, String> {
    Ok(status_json(&state).await)
}

/// Starts the browser sign-in and returns at once. Progress arrives as `spotify-auth` events:
/// `url` (also opened in the default browser), then `signed_in` or `error`.
#[tauri::command]
pub async fn spotify_sign_in(app: tauri::AppHandle, state: St<'_>) -> Result<Value, String> {
    if SIGNING_IN.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        let url = AUTH_URL.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(url) = url {
            state.emit("spotify-auth", json!({ "state": "url", "url": url }));
        }
        return Ok(json!({ "started": true }));
    }
    let task = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _guard = SignInGuard;
        let (url_tx, url_rx) = tokio::sync::oneshot::channel::<String>();
        let emitter = task.clone();
        let opener = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(url) = url_rx.await {
                if let Err(message) = open_authorization(&opener, &url).await {
                    tracing::warn!(%message, "spotify: browser launch failed");
                    emitter.emit(
                        "spotify-auth",
                        json!({ "state": "open_failed", "message": message }),
                    );
                }
            }
        });
        let emitter = task.clone();
        let result = task
            .spotify
            .sign_in(move |url| {
                *AUTH_URL.lock().unwrap_or_else(|e| e.into_inner()) = Some(url.clone());
                emitter.emit("spotify-auth", json!({ "state": "url", "url": url }));
                let _ = url_tx.send(url);
            })
            .await;
        match result {
            Ok(()) => {
                let name = display_name(&task).await;
                task.emit("spotify-auth", json!({ "state": "signed_in", "name": name }));
            }
            Err(e) => {
                task.emit(
                    "spotify-auth",
                    json!({ "state": "error", "message": format!("Spotify sign-in failed: {e:#}") }),
                );
            }
        }
    });
    Ok(json!({ "started": true }))
}

#[tauri::command]
pub async fn spotify_sign_out(state: St<'_>) -> Result<Value, String> {
    state.spotify.sign_out().await;
    if state.spotify.selected() == Provider::Spotify {
        state.spotify.select(Provider::Youtube);
        state.db.set_setting("provider", Provider::Youtube.as_str());
        state.emit("provider-changed", json!({ "provider": Provider::Youtube.as_str() }));
    }
    state.emit("spotify-auth", json!({ "state": "signed_out" }));
    Ok(json!({ "signedIn": false }))
}

#[tauri::command]
pub async fn set_provider(state: St<'_>, provider: String) -> Result<Value, String> {
    let selected = host_provider(&provider)?;
    state.db.set_setting("provider", selected.as_str());
    state.spotify.select(selected);
    state.emit("provider-changed", json!({ "provider": selected.as_str() }));
    Ok(json!({ "provider": selected.as_str() }))
}

#[tauri::command]
pub async fn get_provider(state: St<'_>) -> Result<Value, String> {
    Ok(json!({ "provider": state.spotify.selected().as_str() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_youtube_and_spotify_are_selectable_on_this_host() {
        assert_eq!(host_provider("youtube").unwrap(), Provider::Youtube);
        assert_eq!(host_provider("spotify").unwrap(), Provider::Spotify);
        assert!(host_provider("soundcloud").unwrap_err().contains("not available"));
        assert!(host_provider("tidal").unwrap_err().contains("not available"));
        assert!(host_provider("napster").unwrap_err().contains("Unknown provider"));
    }

    #[test]
    fn signed_out_browsing_reads_as_the_sign_in_prompt() {
        assert_eq!(browse_error(SpotifyBrowseError::SignedOut), "Sign in to Spotify");
    }
}
