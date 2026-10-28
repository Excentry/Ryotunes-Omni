//! Main-window lifecycle for desktop builds.
//!
//! On Linux the expensive WebKit UI is not kept resident just to support background audio. Closing
//! the main window with close-to-tray enabled destroys the webview; libmpv, queue state, MPRIS,
//! tray integration and the network/backend state remain in the native process. A tray click or a
//! second launch recreates the window from `tauri.conf.json`, and the frontend seeds itself from the
//! authoritative backend snapshot. If playback is no longer active and no UI is open, the native
//! process exits after a short grace period only when no resumable media session remains.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

use crate::state::AppState;

static BACKGROUND_MODE: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "linux")]
static BUILDING_MAIN: AtomicBool = AtomicBool::new(false);
static IDLE_EXIT_EPOCH: AtomicU64 = AtomicU64::new(0);
static QUITTING: AtomicBool = AtomicBool::new(false);
static MAIN_READY: AtomicBool = AtomicBool::new(false);

const IDLE_EXIT_GRACE: Duration = Duration::from_secs(5 * 60);

/// Stable Wayland identity used by Tauri/GTK and Hyprland. `enableGTKAppId` in tauri.conf.json is
/// what makes this the actual Wayland app id instead of merely Tauri's bundle identifier.
#[cfg(target_os = "linux")]
const RYOTUNES_APP_ID: &str = "dev.excentry.ryotune-omni";
#[cfg(target_os = "linux")]
const RYOTUNES_MAIN_TITLE: &str = "Ryotunes Omni";

/// Ryoku v2.3 installs a persistent `hl.window_rule` drop-in through the Arch replacement package,
/// exactly like Ryoku Settings/Ryowalls. Do not append transient `windowrulev2` values at runtime:
/// those are version-sensitive, accumulate across launches, and in field testing still allowed the
/// first surface to enter the tiling tree. The post-map IPC helper below remains a defensive fallback
/// for source/dev runs where the packaged Ryoku rule is not installed.
#[cfg(target_os = "linux")]
pub fn install_hyprland_map_rules() {}

#[cfg(not(target_os = "linux"))]
pub fn install_hyprland_map_rules() {}

/// A user-facing webview is alive. Hidden cipher/PoToken bridges deliberately do not count.
pub fn has_ui(app: &AppHandle) -> bool {
    app.get_webview_window("main").is_some() || app.get_webview_window(crate::mini::LABEL).is_some()
}

pub fn is_background_mode() -> bool {
    BACKGROUND_MODE.load(Ordering::Acquire)
}

/// Put the main window into Ryotunes' deliberate floating daily-driver geometry.
///
/// Hyprland/Ryoku can remember or apply a maximized state independently of Tauri's JSON window
/// defaults, so relying on `fullscreen:false` alone is not enough. Every cold surface and every
/// tray reconstruction explicitly leaves fullscreen/maximized mode and is sized against the
/// monitor work area (panels excluded). This still leaves the user free to maximize manually
/// after the window has opened.
pub fn enforce_floating_geometry(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    let _ = win.set_fullscreen(false);
    let _ = win.unmaximize();

    let monitor =
        win.current_monitor().ok().flatten().or_else(|| win.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        let _ = win.set_size(PhysicalSize::new(1760u32, 1000u32));
        let _ = win.center();
        return;
    };

    let area = monitor.work_area();
    // v2.3 field target: a large, centered daily-driver canvas matching the accepted Ryoku
    // reference screenshot. The pre-map JSON fallback is 1760×1000 logical; once mapped, this
    // adaptive pass uses the real monitor work area so smaller/larger displays keep modest margins.
    let max_width = area.size.width.saturating_sub(24).max(640);
    let max_height = area.size.height.saturating_sub(24).max(480);
    let min_width = 1120u32.min(max_width);
    let min_height = 700u32.min(max_height);
    let width = (((area.size.width as f64) * 0.92).round() as u32).clamp(min_width, max_width);
    let height = (((area.size.height as f64) * 0.84).round() as u32).clamp(min_height, max_height);

    let x = area.position.x + ((area.size.width.saturating_sub(width)) / 2) as i32;
    let y = area.position.y + ((area.size.height.saturating_sub(height)) / 2) as i32;
    let _ = win.set_size(PhysicalSize::new(width, height));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// Ask Hyprland to treat Ryotunes as a floating daily-driver window.
///
/// Wayland clients cannot universally declare "float me"; tiling vs floating is compositor policy.
/// On Ryoku/Hyprland we can, however, use Hyprland's own IPC without modifying the user's config.
/// The helper is completely inert elsewhere. It looks up this process by PID, uses `setfloating`
/// only when needed (never toggle), then reapplies the work-area geometry after Hyprland has moved
/// the client out of the tiling tree.
#[cfg(target_os = "linux")]
pub fn request_hyprland_float(app: &AppHandle) {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // The client can take a few compositor frames to appear in `hyprctl clients -j` on a cold
        // start/recreated WebView. Retry briefly without blocking the UI or the Tauri event loop.
        for delay in [40u64, 80, 140, 220] {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            let output =
                match std::process::Command::new("hyprctl").args(["clients", "-j"]).output() {
                    Ok(output) if output.status.success() => output,
                    _ => return,
                };
            let Ok(clients) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
                return;
            };
            let Some(client) = clients.as_array().and_then(|rows| {
                let pid = std::process::id() as u64;
                // A mini player can briefly coexist while the main WebView is reconstructed. Match
                // the main surface's stable GTK app id + title, not merely the shared process PID.
                rows.iter()
                    .filter(|row| row.get("pid").and_then(|v| v.as_u64()) == Some(pid))
                    .filter(|row| {
                        row.get("class").and_then(|v| v.as_str()) == Some(RYOTUNES_APP_ID)
                    })
                    .filter(|row| {
                        row.get("title").and_then(|v| v.as_str()) == Some(RYOTUNES_MAIN_TITLE)
                    })
                    .find(|row| !row.get("floating").and_then(|v| v.as_bool()).unwrap_or(false))
                    .or_else(|| {
                        rows.iter()
                            .filter(|row| row.get("pid").and_then(|v| v.as_u64()) == Some(pid))
                            .find(|row| {
                                row.get("title").and_then(|v| v.as_str())
                                    == Some(RYOTUNES_MAIN_TITLE)
                            })
                    })
            }) else {
                continue;
            };
            let Some(address) = client.get("address").and_then(|v| v.as_str()) else { return };
            let floating = client.get("floating").and_then(|v| v.as_bool()).unwrap_or(false);
            if !floating {
                let arg = format!("address:{address}");
                let _ = std::process::Command::new("hyprctl")
                    .args(["dispatch", "setfloating", &arg])
                    .output();
                tokio::time::sleep(Duration::from_millis(35)).await;
            }
            enforce_floating_geometry(&handle);
            return;
        }
    });
}

#[cfg(not(target_os = "linux"))]
pub fn request_hyprland_float(_app: &AppHandle) {}

/// Mark the next main-window destruction as an intentional background transition.
/// v2.3 deliberately does not persist main-window maximize/geometry state: every cold/recreated
/// surface comes back as the same comfortable centered floating window from tauri.conf.json.
pub fn prepare_hibernate(_app: &AppHandle) {
    cancel_idle_exit();
    BACKGROUND_MODE.store(true, Ordering::Release);
}

/// Destroy the Linux main webview after another visible UI (the mini player) has come up.
/// Non-Linux platforms keep their existing hide/show path; WebView2/WKWebView have different
/// process models and have not been field-tested for this lifecycle.
pub fn hibernate_main(app: &AppHandle) {
    MAIN_READY.store(false, Ordering::Release);
    #[cfg(target_os = "linux")]
    {
        let Some(main) = app.get_webview_window("main") else { return };
        prepare_hibernate(app);
        if let Err(e) = main.destroy() {
            BACKGROUND_MODE.store(false, Ordering::Release);
            tracing::warn!(error = %e, "could not hibernate main window");
        }
    }
    #[cfg(not(target_os = "linux"))]
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
}

/// Bring the application UI back. On Linux this can mean rebuilding a webview that was destroyed
/// while music kept playing; on other platforms it is the traditional hide/show operation.
pub fn show(app: &AppHandle) {
    cancel_idle_exit();
    if let Some(w) = app.get_webview_window("main") {
        // A fully mounted existing surface can be revealed immediately. A cold/reconstructed
        // WebView normally stays hidden only until `frontend_ready`. If that handshake has not
        // arrived yet, arm a short native failsafe instead of returning forever: a second launcher
        // click must always be able to recover a hidden surface.
        if MAIN_READY.load(Ordering::Acquire) {
            BACKGROUND_MODE.store(false, Ordering::Release);
            enforce_floating_geometry(app);
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
            request_hyprland_float(app);
            crate::mini::close(app);
        } else {
            arm_reveal_failsafe(app, Duration::from_millis(220));
        }
        return;
    }

    #[cfg(target_os = "linux")]
    recreate_linux(app);
}

/// Bounded native recovery for the hidden-until-mounted handshake.
///
/// A hidden WebKitGTK toplevel can throttle presentation callbacks. The frontend normally invokes
/// `frontend_ready` directly from Svelte's `onMount`, but release builds must never be able to get
/// stuck tray-only if that bridge message is delayed or lost. After the deadline, reveal the
/// existing main surface and mark it ready so launcher/tray clicks remain functional. This is only
/// a fallback: successful frontend readiness makes the task a no-op.
pub fn arm_reveal_failsafe(app: &AppHandle, delay: Duration) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(delay).await;
        if MAIN_READY.load(Ordering::Acquire) || QUITTING.load(Ordering::Acquire) {
            return;
        }
        let Some(w) = handle.get_webview_window("main") else { return };

        tracing::warn!("frontend readiness deadline expired; revealing main window natively");
        enforce_floating_geometry(&handle);
        if let Err(e) = w.show() {
            tracing::error!(error = %e, "native main-window reveal failsafe failed");
            return;
        }
        MAIN_READY.store(true, Ordering::Release);
        BACKGROUND_MODE.store(false, Ordering::Release);
        let _ = w.unminimize();
        let _ = w.set_focus();
        request_hyprland_float(&handle);
        crate::mini::close(&handle);
    });
}

/// Authoritative reveal handshake from a mounted frontend. Cold start and reconstructed WebViews
/// both arrive here while hidden, after their first real UI tree exists. Geometry is set while
/// hidden; Hyprland's persistent Ryoku rule supplies float/center at map time; only then do we show
/// and focus. The mini player is destroyed *after* the main surface is visible, so its expand button
/// can never leave the user with playback but no visible Ryotunes window.
pub fn frontend_ready(app: &AppHandle) -> Result<(), String> {
    let Some(w) = app.get_webview_window("main") else {
        return Err("main window is missing".into());
    };
    MAIN_READY.store(true, Ordering::Release);
    cancel_idle_exit();
    enforce_floating_geometry(app);
    w.show().map_err(|e| format!("show main window: {e}"))?;
    let _ = w.unminimize();
    let _ = w.set_focus();
    BACKGROUND_MODE.store(false, Ordering::Release);
    // Only a fallback when the Ryoku config drop-in is absent (e.g. `cargo tauri dev`).
    request_hyprland_float(app);
    crate::mini::close(app);
    Ok(())
}

#[cfg(target_os = "linux")]
fn recreate_linux(app: &AppHandle) {
    use tauri::WebviewWindowBuilder;

    if BUILDING_MAIN.swap(true, Ordering::AcqRel) {
        return;
    }
    // Keep ExitRequested suppressed until a real replacement exists. If creation fails the tray
    // remains the recovery surface and a later click/second launch can retry.
    BACKGROUND_MODE.store(true, Ordering::Release);
    MAIN_READY.store(false, Ordering::Release);

    let handle = app.clone();
    let dispatch = app.run_on_main_thread(move || {
        let result = (|| -> Result<(), String> {
            let config = handle
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .cloned()
                .ok_or_else(|| "main window config is missing".to_string())?;
            let win = WebviewWindowBuilder::from_config(&handle, &config)
                .map_err(|e| e.to_string())?
                .build()
                .map_err(|e| e.to_string())?;

            crate::tune_webview_labelled(&handle, "main");
            enforce_floating_geometry(&handle);
            // Keep the reconstructed surface hidden until Svelte has mounted. `initWin()` reveals
            // and focuses it, so tray reopen cannot expose an unpainted/black WebKit frame. The
            // compositor map-time rules are already installed before this WebView exists.
            request_hyprland_float(&handle);
            Ok(())
        })();

        BUILDING_MAIN.store(false, Ordering::Release);
        match result {
            Ok(()) => {
                // Stay in background mode until the mounted frontend acknowledges readiness and is
                // actually shown. Keeping the mini alive here fixes the expand-to-main race.
                // If the bridge never acknowledges, native recovery guarantees a visible surface.
                cancel_idle_exit();
                arm_reveal_failsafe(&handle, Duration::from_millis(1500));
            }
            Err(e) => tracing::error!(error = %e, "could not recreate main window"),
        }
    });

    if let Err(e) = dispatch {
        BUILDING_MAIN.store(false, Ordering::Release);
        tracing::error!(error = %e, "could not dispatch main-window recreation");
    }
}

/// True once an explicit/idle Quit has begun. Window-destroy and ExitRequested handlers use this
/// to avoid re-entering background logic while integrations are being torn down.
pub fn is_quitting() -> bool {
    QUITTING.load(Ordering::Acquire)
}

/// One authoritative application-exit path. It is intentionally asynchronous so Listen Together
/// can leave cleanly, but media/Discord owner threads acknowledge their teardown before exit.
pub fn request_quit(app: &AppHandle) {
    if QUITTING.swap(true, Ordering::AcqRel) {
        return;
    }
    cancel_idle_exit();
    BACKGROUND_MODE.store(false, Ordering::Release);
    crate::mini::save_position(app);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state =
            handle.try_state::<std::sync::Arc<AppState>>().map(|state| state.inner().clone());
        if let Some(state) = state {
            state.shutdown_for_quit().await;
        }
        handle.exit(0);
    });
}

/// Invalidate a pending idle shutdown. Playback resume and UI restore both call this so an older
/// empty-session deadline can never terminate a newly active session.
pub fn cancel_idle_exit() {
    IDLE_EXIT_EPOCH.fetch_add(1, Ordering::AcqRel);
}

fn idle_exit_eligible(background: bool, has_ui: bool, playing: bool) -> bool {
    background && !has_ui && !playing
}

/// With no user-facing UI and no active music playback, keep the lightweight tray/backend alive
/// for five minutes and then leave cleanly. A paused track counts as no playback for this policy;
/// resuming playback or reopening a window invalidates the deadline through [`cancel_idle_exit`].
/// The decision is event-driven and never performs a synchronous mpv property query here.
pub fn schedule_idle_exit(app: &AppHandle) {
    let playing = app
        .try_state::<std::sync::Arc<AppState>>()
        .map(|state| state.is_playing())
        .unwrap_or(false);
    if !idle_exit_eligible(is_background_mode(), has_ui(app), playing) {
        cancel_idle_exit();
        return;
    }

    let epoch = IDLE_EXIT_EPOCH.fetch_add(1, Ordering::AcqRel) + 1;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(IDLE_EXIT_GRACE).await;
        if IDLE_EXIT_EPOCH.load(Ordering::Acquire) != epoch {
            return;
        }
        let playing = handle
            .try_state::<std::sync::Arc<AppState>>()
            .map(|state| state.is_playing())
            .unwrap_or(false);
        if !idle_exit_eligible(is_background_mode(), has_ui(&handle), playing) {
            return;
        }
        tracing::info!("background playback is inactive; exiting idle Ryotunes Omni process");
        request_quit(&handle);
    });
}

/// Linux/glibc can keep native allocator arenas after a large UI teardown. One delayed trim is
/// enough here; doing it in a tight loop would trade RAM cosmetics for CPU wakeups.
#[cfg(target_os = "linux")]
pub fn trim_after_hibernate() {
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        unsafe { libc::malloc_trim(0) };
    });
}

#[cfg(not(target_os = "linux"))]
pub fn trim_after_hibernate() {}

#[cfg(test)]
mod tests {
    use super::idle_exit_eligible;

    #[test]
    fn idle_exit_requires_background_without_ui_or_active_playback() {
        assert!(idle_exit_eligible(true, false, false));
        assert!(!idle_exit_eligible(false, false, false));
        assert!(!idle_exit_eligible(true, true, false));
        assert!(!idle_exit_eligible(true, false, true));
    }
}
