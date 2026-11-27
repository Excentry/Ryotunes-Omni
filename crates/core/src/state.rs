//! App state: transport, player, db, and the queue/playback manager. UI state.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use std::sync::Arc;

use innertube::{
    AccountIdentity, AccountInfo, AudioQuality, Clients, InnerTube, SongItem, MAIN_CLIENT,
};
use listen_protocol::Track;
use player::{AudioFx, Player};
use tokio::sync::Mutex;

use crate::db::{now_secs, Db};
use crate::discord::DiscordHandle;
use crate::host::{EventSink, LoginFlow, Paths};
use crate::listentogether::LtSession;
use crate::media::MediaHandle;
use crate::media_ref::MediaRef;
use crate::orchestrator::{Orchestrator, PlaybackData, PlaybackPing, ResolveError};

mod tidal_credentials;
pub use tidal_credentials::TidalCredentials;
mod listen_together;
mod now_playing;
mod position;
mod queue;
mod spotify_stream;
use queue::{
    append_page, autoplay_tail_near, drop_duplicates, enqueue_at, guest_insert_index, is_mix,
    merge_radio, next_index, persist_fingerprint, queue_event_payload, queue_fingerprint,
    radio_seed_for, shuffle_new_queue, shuffle_upcoming, splice_radio_into, unshuffled,
    upcoming_queued, QueueState,
};

/// Synthetic browseId for the On Repeat playlist. Not a YouTube id: `get_playlist` intercepts it
/// and builds the page from local play counts, so it must never collide with a real browseId
/// (`VL…` / `MPRE…` / `RD…`).
pub const ON_REPEAT_ID: &str = "RYOTUNES_ON_REPEAT";
/// How far back On Repeat looks. A month is long enough to survive a quiet week and short enough
/// that the list still turns over with what you're actually playing.
pub const ON_REPEAT_WINDOW_SECS: i64 = 30 * 24 * 60 * 60;
/// How many songs it holds.
pub const ON_REPEAT_LIMIT: usize = 20;
/// Recent/Rediscover are also local smart playlists: never pass these ids to YouTube.
pub const RECENTLY_PLAYED_ID: &str = "RYOTUNES_RECENTLY_PLAYED";
pub const REDISCOVER_ID: &str = "RYOTUNES_REDISCOVER";
pub const RECENTLY_PLAYED_WINDOW_SECS: i64 = 7 * 24 * 60 * 60;
pub const REDISCOVER_OLDER_THAN_SECS: i64 = 14 * 24 * 60 * 60;
pub const SMART_PLAYLIST_LIMIT: usize = 40;
pub const LOCAL_PLAYLIST_PREFIX: &str = "RYOTUNES_LOCAL_PLAYLIST:";
/// The device's own Liked Songs: where the heart lands whenever YouTube's rating is not on the
/// table (no Google account, a SoundCloud or local track). A plain local playlist with a fixed
/// id, created on the first like, so it pages, plays and edits like any other.
pub const LIKED_SONGS_ID: &str = "RYOTUNES_LOCAL_PLAYLIST:liked";
pub const LIKED_SONGS_TITLE: &str = "Liked Songs";

/// The persisted SoundCloud OAuth blob (access + rotating refresh token + session cookie).
/// Deliberately NOT in [`UI_SETTINGS`]: like `session_cookie`, it is auth material the
/// renderer must never see; only the daemon's sign-in/rotate paths write it.
pub const SOUNDCLOUD_AUTH_KEY: &str = "soundcloud_auth";
/// The TIDAL session blob. Outside `UI_SETTINGS` for the same reason as SoundCloud's: the renderer
/// may neither read nor overwrite a bearer.
pub const TIDAL_AUTH_KEY: &str = "tidal_auth";
/// A TIDAL client pair the user supplies. Optional: the crate ships a working pair, and this one
/// overrides it field by field, so someone with their own registered application can replace one
/// half and keep the other.
pub const TIDAL_CREDENTIALS_KEY: &str = "tidal_credentials";
/// Per Spotify track: the YouTube Music video it plays as where librespot has no audio path.
pub const SPOTIFY_YOUTUBE_MATCH_PREFIX: &str = "spotify_youtube_match:";

/// Settings the UI is allowed to read *and write*. Session/auth material (`session_cookie`,
/// `selected_identity_json`, `data_sync_id`, `account_json`, `account_selection_pending`,
/// `visitor_data`) and internal blobs (`queue_json`, `queue_index`, `queue_position`) never cross
/// into the renderer: they'd otherwise ship the login credential out on every open, and the
/// renderer can't overwrite them either. Lives here so `settings_snapshot`, the Tauri host and the
/// daemon share one list.
pub const UI_SETTINGS: [&str; 14] = [
    "volume",
    "proxy",
    "quality",
    "enable_history",
    "disabled_stream_clients",
    "discord_rpc",
    "discord_presence_name",
    "close_to_tray",
    "autostart",
    "autoplay",
    "prevent_duplicates",
    "lyrics_boidu",
    "ui_scale",
    "low_resource_mode",
];

/// Where the personal store (Home Shortcuts, sidebar pins, play recency, Home arrangement) lives:
/// one JSON blob under this settings key. It moved off the Svelte app's browser `localStorage`,
/// which a native client can't read, so it belongs to the daemon like any other user state. Not in
/// [`UI_SETTINGS`] on purpose: it is written through the dedicated `set_personal` method (which
/// validates and emits `personal-changed`), never the generic `set_setting` allowlist.
pub const PERSONAL_JSON_KEY: &str = "personal_json";
/// A ceiling so a corrupt or hostile client can't wedge the store with an unbounded blob. Real
/// stores are a few KiB; a MiB is orders of magnitude of headroom and still bounds the write.
const MAX_PERSONAL_BYTES: usize = 1024 * 1024;

pub fn is_local_playlist_id(id: &str) -> bool {
    id.starts_with(LOCAL_PLAYLIST_PREFIX)
}

pub fn is_smart_playlist_id(id: &str) -> bool {
    id == ON_REPEAT_ID || id == RECENTLY_PLAYED_ID || id == REDISCOVER_ID
}

pub struct AppState {
    pub it: InnerTube,
    pub clients: Clients,
    pub player: Player,
    pub db: Arc<Db>,
    pub sink: Arc<dyn EventSink>,
    pub login: Arc<dyn LoginFlow>,
    pub paths: Paths,
    pub orchestrator: Arc<Orchestrator>,
    /// Listen Together session (session protocol). Drives host broadcasts + guest gating.
    pub lt: Arc<LtSession>,
    /// OS media integration (MPRIS/SMTC/NowPlaying). `None` if it failed to init. fail-soft policy.
    media: Option<MediaHandle>,
    /// Discord rich presence. Fed the same track/playback changes as `media`; gated on the
    /// `discord_rpc` setting inside its own thread.
    discord: Option<DiscordHandle>,
    /// Last.fm scrobbler. Same feed again; parks until a session key is set (titlebar button).
    pub lastfm: crate::lastfm::LastfmHandle,
    /// The Spotify provider: sign-in, the client, and the catalogue selector.
    pub spotify: Arc<crate::spotify::SpotifyState>,
    queue: Mutex<QueueState>,
    /// Bumped on every explicit `play`/jump so superseded async resolves discard their result
    /// (cancellation without JoinHandle bookkeeping). stream selection §6.
    generation: AtomicU64,
    /// Bumped before every explicit rating write. A detached rating refresh samples this epoch and
    /// discards an answer that predates a user's newer like/dislike action.
    pub rate_epoch: AtomicU64,
    /// A one-shot resume position `(videoId, secs)` set by `restore_queue` and consumed by the
    /// next `start_current` — applied only when that track is the one being started, so jumping to
    /// a different track first doesn't inherit the old position (UI state).
    pending_seek: std::sync::Mutex<Option<(String, f64)>>,
    /// Mirror of mpv's pause flag (set in `media_set_playing`). Position ticks must consult this
    /// instead of assuming "playing" — mpv fires `time-pos` on seeks while paused too.
    is_playing: AtomicBool,
    /// Latest mpv position (f64 bits) + wall-clock secs of the last DB write, for throttled
    /// resume-position persistence.
    latest_position: AtomicU64,
    last_pos_persist: AtomicU64,
    /// Wall-clock secs of the last position push to the OS media controls (throttled ~1s).
    last_media_push: AtomicU64,
    /// Fingerprint of the item list the UI was last sent (see `queue_fingerprint`). When it is
    /// unchanged, `emit_queue` sends the small `queue-index` event instead of megabytes of JSON:
    /// a Tauri event inlines its payload into a JavaScript source string, so a 5,000-track queue
    /// costs ~26 ms of parse in the webview every time it crosses.
    last_queue_fingerprint: AtomicU64,
    /// Dirty key (`persist_fingerprint`) last written to `queue_json`. Same trick as
    /// `last_queue_fingerprint`, for the persistence side: an advance rewrites 40 bytes rather
    /// than the whole blob.
    last_persisted_fingerprint: AtomicU64,
    /// One-shot sleep-timer style stop: when set, the current track is allowed to finish but no
    /// queued/autoplay successor is started. Kept atomic because the mpv event pump reads it.
    stop_after_current: AtomicBool,
    /// Runtime mirror of Settings → Low resource mode. The renderer owns most of the policy; this
    /// bit lets the native event pump and startup helpers lower their cadence too.
    low_resource_mode: AtomicBool,
    /// The Sound dialog's live audio effects (spec section 7), mirrored to every client through
    /// the `audio-fx` event and the subscribe snapshot. Applied to the player as one `af` chain;
    /// kept here so a newly opened window can be told the current values.
    audio_fx: parking_lot::RwLock<AudioFx>,
    /// The one live Spotify stream, if a `spotify:track:` id is playing: the task watching
    /// librespot's events. A fresh `resolve` of a Spotify id
    /// replaces it (dropping the previous handle and aborting its watcher); a non-Spotify track or
    /// an explicit stop drops it. `None` whenever YouTube/local/radio audio is playing.
    spotify_stream: parking_lot::Mutex<Option<SpotifyStream>>,
    /// The SoundCloud provider (guest api-v2 client), cache dir `<data>/soundcloud`. SoundCloud
    /// tracks stream over plain HTTPS, so unlike `spotify_stream` there is no live handle to hold.
    pub soundcloud: Arc<ryotunes_soundcloud::SoundCloud>,
    /// The TIDAL provider, cache dir `<data>/tidal`. Like SoundCloud it streams over plain HTTPS,
    /// so there is no live handle to hold; unlike SoundCloud it has no guest catalogue, so every
    /// call needs a session.
    pub tidal: Arc<ryotunes_tidal::Tidal>,
    /// Play/like/genre for SoundCloud tracks seen by `resolve`, keyed by `sc:track:` id, so the
    /// now-playing snapshot can render the meta line without another round-trip. Bounded in
    /// `remember_sc_meta`.
    sc_meta: parking_lot::Mutex<std::collections::HashMap<String, ScTrackMeta>>,
    /// Unix secs of the last on-demand visitorData (re)bootstrap attempt, so a failing session
    /// retries on a cooldown instead of hammering sw.js_data on every skipped track.
    last_visitor_try: AtomicU64,
}

/// One live Spotify stream held on [`AppState`]: the task watching librespot's event stream,
/// which owns the [`StreamHandle`]. Aborting the watcher tears it down.
struct SpotifyStream {
    watcher: tokio::task::JoinHandle<()>,
}

/// Play/like/genre for one SoundCloud track, remembered by [`AppState::resolve`] so the
/// now-playing snapshot can render the "plays · likes · genre" meta line. Counts are pre-formatted
/// display strings ("1.2M"); absent fields stay `None`.
#[derive(Clone, Default)]
struct ScTrackMeta {
    plays: Option<String>,
    likes: Option<String>,
    genre: Option<String>,
}

/// Repeat mode for the queue. Serialized lowercase for the UI + `queue_json`.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

/// Which queue a background playlist walk ([`AppState::fill_playlist`]) is filling: the one that's
/// playing, or an "Add to queue" block (whose pages keep their order and carry the block's label).
enum Fill {
    Playing,
    Queued(Option<String>),
}

/// Canonical persisted account selection. `data_sync_id` drives request delegation; every display
/// field beside it belongs to that same identity. `account_json` and the legacy `data_sync_id`
/// setting are written only as atomic projections of this model.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct SelectedIdentity {
    #[serde(default)]
    data_sync_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    handle: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    thumbnail: Option<String>,
    #[serde(default)]
    channel_id: Option<String>,
    #[serde(default)]
    has_multiple_identities: bool,
}

pub enum SignInOutcome {
    Complete,
    SelectionRequired,
}

impl SelectedIdentity {
    fn from_account_info(info: AccountInfo, has_multiple_identities: bool) -> Option<Self> {
        Some(Self {
            data_sync_id: info.data_sync_id,
            name: Some(info.name?),
            handle: info.handle,
            email: info.email,
            thumbnail: info.thumbnail,
            channel_id: info.channel_id,
            has_multiple_identities,
        })
    }

    fn from_identity(
        identity: &AccountIdentity,
        refreshed: AccountInfo,
        has_multiple_identities: bool,
    ) -> Self {
        Self {
            data_sync_id: Some(identity.data_sync_id.clone()),
            name: refreshed.name.or_else(|| Some(identity.name.clone())),
            handle: refreshed.handle.or_else(|| identity.handle.clone()),
            email: refreshed.email.or_else(|| identity.email.clone()),
            thumbnail: refreshed.thumbnail.or_else(|| identity.thumbnail.clone()),
            channel_id: refreshed.channel_id.or_else(|| identity.channel_id.clone()),
            has_multiple_identities,
        }
    }

    fn account_json(&self, signed_in: bool) -> serde_json::Value {
        serde_json::json!({
            "signedIn": signed_in,
            "name": self.name,
            "handle": self.handle,
            "email": self.email,
            "thumbnail": self.thumbnail,
            "channelId": self.channel_id,
            "canSwitch": self.has_multiple_identities,
        })
    }

    fn as_account_identity(&self) -> Option<AccountIdentity> {
        Some(AccountIdentity {
            name: self.name.clone().unwrap_or_default(),
            handle: self.handle.clone(),
            email: self.email.clone(),
            thumbnail: self.thumbnail.clone(),
            channel_id: self.channel_id.clone(),
            data_sync_id: self.data_sync_id.clone()?,
            is_selected: false,
        })
    }
}

fn selected_identity_from_db(db: &Db) -> Option<SelectedIdentity> {
    db.get_setting("selected_identity_json")
        .and_then(|json| serde_json::from_str::<SelectedIdentity>(&json).ok())
        .filter(|identity| identity.name.is_some() || identity.data_sync_id.is_some())
        .or_else(|| {
            let account = db
                .get_setting("account_json")
                .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())?;
            let string = |key: &str| {
                account
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            };
            Some(SelectedIdentity {
                data_sync_id: db.get_setting("data_sync_id").filter(|id| !id.is_empty()),
                name: string("name"),
                handle: string("handle"),
                email: string("email"),
                thumbnail: string("thumbnail"),
                channel_id: string("channelId"),
                has_multiple_identities: account
                    .get("canSwitch")
                    .and_then(serde_json::Value::as_bool)
                    // Pre-switcher account_json has no flag. Keep the action discoverable until
                    // the startup identity refresh determines whether this legacy user has one
                    // channel or several.
                    .unwrap_or(true),
            })
        })
}

fn auth_selection_pending(db: &Db) -> bool {
    db.get_setting("account_selection_pending").as_deref() == Some("true")
}

/// Startup compatibility: prefer the canonical model, then fall back to the pre-switcher setting.
pub fn persisted_data_sync_id(db: &Db) -> Option<String> {
    selected_identity_from_db(db)
        .and_then(|identity| identity.data_sync_id)
        .or_else(|| db.get_setting("data_sync_id").filter(|id| !id.is_empty()))
}

fn identity_selection_key(data_sync_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    "ryotunes-account-identity-v1".hash(&mut hasher);
    data_sync_id.hash(&mut hasher);
    format!("identity-{:016x}", hasher.finish())
}

fn identity_snapshot(identity: &AccountIdentity, selected: bool) -> serde_json::Value {
    serde_json::json!({
        "selectionKey": identity_selection_key(&identity.data_sync_id),
        "name": identity.name,
        "handle": identity.handle,
        "email": identity.email,
        "thumbnail": identity.thumbnail,
        "channelId": identity.channel_id,
        "selected": selected,
    })
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        it: InnerTube,
        clients: Clients,
        player: Player,
        db: Arc<Db>,
        sink: Arc<dyn EventSink>,
        login: Arc<dyn LoginFlow>,
        paths: Paths,
        orchestrator: Arc<Orchestrator>,
        lt: Arc<LtSession>,
        media: Option<MediaHandle>,
        discord: Option<DiscordHandle>,
        lastfm: crate::lastfm::LastfmHandle,
    ) -> Self {
        let low_resource_mode = db.get_setting("low_resource_mode").as_deref() == Some("true");
        let selected = db
            .get_setting("provider")
            .as_deref()
            .and_then(crate::spotify::Provider::parse)
            .unwrap_or(crate::spotify::Provider::Youtube);
        let spotify =
            Arc::new(crate::spotify::SpotifyState::new(paths.data_dir.join("spotify"), selected));
        let soundcloud =
            Arc::new(ryotunes_soundcloud::SoundCloud::new(paths.data_dir.join("soundcloud")));
        // A captured SoundCloud sign-in survives restarts like the YouTube cookie does: the
        // JSON blob lives under a settings key outside UI_SETTINGS, so the renderer can
        // neither read nor overwrite it. Every token rotation writes the new pair back
        // through this hook (SoundCloud's refresh token is single-use — losing a rotation
        // dead-ends the session), and the restore is validated against /me by the host.
        if let Some(raw) = db.get_setting(SOUNDCLOUD_AUTH_KEY) {
            if let Ok(auth) = serde_json::from_str::<ryotunes_soundcloud::SoundcloudAuth>(&raw) {
                soundcloud.set_auth(Some(auth));
            }
        }
        let rotate_db = Arc::clone(&db);
        soundcloud.set_on_rotate(std::sync::Arc::new(move |auth| {
            match serde_json::to_string(&auth) {
                Ok(json) => rotate_db.set_setting(SOUNDCLOUD_AUTH_KEY, &json),
                Err(e) => {
                    tracing::error!(error = %e, "soundcloud: cannot serialize rotated auth")
                }
            }
        }));
        let tidal = Arc::new(ryotunes_tidal::Tidal::new(paths.data_dir.join("tidal")));
        // The crate ships a credential pair so signing in needs nothing but the button. A stored
        // pair overrides it field by field, so someone supplying only their own device id keeps
        // the shipped PKCE half rather than losing hi-res.
        if let Some(raw) = db.get_setting(TIDAL_CREDENTIALS_KEY) {
            if let Ok(creds) = serde_json::from_str::<TidalCredentials>(&raw) {
                tidal.set_credentials(creds.into());
            }
        }
        if let Some(raw) = db.get_setting(TIDAL_AUTH_KEY) {
            match serde_json::from_str::<ryotunes_tidal::TidalAuth>(&raw) {
                Ok(auth) => tidal.set_auth(Some(auth)),
                Err(e) => tracing::warn!(error = %e, "tidal: stored session did not parse"),
            }
        }
        let tidal_db = Arc::clone(&db);
        tidal.set_on_rotate(move |auth| match serde_json::to_string(&auth) {
            Ok(json) => tidal_db.set_setting(TIDAL_AUTH_KEY, &json),
            Err(e) => tracing::error!(error = %e, "tidal: cannot serialize rotated auth"),
        });
        AppState {
            it,
            clients,
            player,
            db,
            sink,
            login,
            paths,
            orchestrator,
            lt,
            media,
            discord,
            lastfm,
            spotify,
            soundcloud,
            tidal,
            sc_meta: parking_lot::Mutex::new(std::collections::HashMap::new()),
            last_visitor_try: AtomicU64::new(0),
            queue: Mutex::new(QueueState::default()),
            is_playing: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            rate_epoch: AtomicU64::new(0),
            pending_seek: std::sync::Mutex::new(None),
            latest_position: AtomicU64::new(0),
            last_pos_persist: AtomicU64::new(0),
            last_media_push: AtomicU64::new(0),
            last_queue_fingerprint: AtomicU64::new(0),
            last_persisted_fingerprint: AtomicU64::new(0),
            stop_after_current: AtomicBool::new(false),
            low_resource_mode: AtomicBool::new(low_resource_mode),
            audio_fx: parking_lot::RwLock::new(AudioFx::default()),
            spotify_stream: parking_lot::Mutex::new(None),
        }
    }

    /// Serialize `payload` and fan it out through the host's event sink. Replaces the old
    /// per-window emit; a payload that will not serialize is logged, never panicked.
    pub fn emit<T: serde::Serialize>(&self, event: &'static str, payload: T) {
        match serde_json::to_value(payload) {
            Ok(v) => self.sink.emit(event, v),
            Err(e) => tracing::warn!(event, error = %e, "event payload not serializable"),
        }
    }

    fn quality(&self) -> AudioQuality {
        match self.db.get_setting("quality").as_deref() {
            Some("LOW") => AudioQuality::Low,
            Some("AUTO") => AudioQuality::Auto,
            _ => AudioQuality::High,
        }
    }

    /// The TIDAL tier to ask for. Clamped by the provider to whatever the session is actually
    /// entitled to, so a hi-res setting on a device-flow token yields LOSSLESS rather than a
    /// thirty second preview.
    fn tidal_quality(&self) -> ryotunes_tidal::Quality {
        self.db
            .get_setting("tidal_quality")
            .as_deref()
            .and_then(ryotunes_tidal::Quality::parse)
            .unwrap_or(ryotunes_tidal::Quality::Lossless)
    }

    /// User-disabled stream clients — comma-separated setting. Also the force-fail lever for the
    /// rustypipe-solo acceptance test; `RYOTUNES_DISABLED_CLIENTS` env overrides for quick testing.
    fn disabled_clients(&self) -> HashSet<String> {
        let raw = std::env::var("RYOTUNES_DISABLED_CLIENTS")
            .ok()
            .or_else(|| self.db.get_setting("disabled_stream_clients"))
            .unwrap_or_default();
        raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    }

    // --- auth (authentication flow) ------------------------------------------------------------------

    /// Sign in with the Cookie header captured from the login webview (authentication flow Path A).
    ///
    /// `account_menu` validates the cookie and provides the active header. `accounts_list` then
    /// discovers every server-selectable identity. A persisted matching identity wins over
    /// Google's current default; a new multi-channel login pauses before finalization and asks the
    /// UI to choose.
    pub(crate) async fn apply_cookie(&self, cookie: String) -> Result<SignInOutcome, String> {
        let cookie = cookie.trim().to_owned();
        if innertube::cookie_sapisid(&cookie).is_none() {
            return Err("Sign-in didn't complete — try signing in again.".into());
        }
        let previous_cookie = self.it.cookie();
        let previous_data_sync_id = self.it.data_sync_id();
        let persisted_identity = selected_identity_from_db(&self.db);
        let persisted_id = persisted_identity
            .as_ref()
            .and_then(|identity| identity.data_sync_id.clone())
            .or_else(|| self.db.get_setting("data_sync_id").filter(|id| !id.is_empty()));

        self.it.set_cookie(Some(cookie.clone()));
        // Discovery must not inherit a stale/default identity. The persisted id is restored only
        // after the fresh accounts list proves that this cookie can act as it.
        self.it.set_data_sync_id(None);
        let client =
            self.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let active = match self.it.account_menu(client).await {
            // A valid, authenticating cookie returns the account header (name). No name means the
            // session didn't actually authenticate — reject it up front so we don't "succeed" into
            // a silently-empty library.
            Ok(i) if i.name.is_some() => i,
            Ok(_) => {
                self.restore_auth_transport(previous_cookie, previous_data_sync_id);
                return Err("That session didn't authenticate — sign in again.".into());
            }
            // Auth didn't take (network) — roll back so we're not half-logged-in.
            Err(e) => {
                self.restore_auth_transport(previous_cookie, previous_data_sync_id);
                return Err(format!("Sign-in failed: {e}"));
            }
        };

        let active_visitor_data = active.visitor_data.clone();

        // Discovery must go out with the login-bound visitorData account_menu just handed us, not
        // the anonymous bootstrap id still sitting in the session: pairing that id with login
        // cookies is what gets accounts_list rejected, and a rejected list reads below as "this
        // Google account has one channel". Every success path persists this same value; hand the
        // old one back until then so a rolled-back sign-in isn't left holding it.
        let previous_visitor_data = self.it.visitor_data();
        if active_visitor_data.is_some() {
            self.it.set_visitor_data(active_visitor_data.clone());
        }
        // Losing the list endpoint must not regress ordinary one-channel sign-in. It only removes
        // the optional switcher for this login attempt; account_menu still supplies a valid active
        // identity exactly as older releases used it.
        let identities = match self.it.account_identities(client).await {
            Ok(identities) => identities,
            Err(error) => {
                tracing::warn!(%error, "could not discover alternate YouTube identities");
                Vec::new()
            }
        };
        self.it.set_visitor_data(previous_visitor_data);

        let chosen = persisted_id
            .as_deref()
            .and_then(|id| identities.iter().find(|identity| identity.data_sync_id == id));
        if let Some(identity) = chosen {
            if identities.len() == 1 {
                let selected = SelectedIdentity::from_identity(identity, active, false);
                self.persist_selected_identity(selected, active_visitor_data.as_deref())
                    .inspect_err(|_| {
                        self.restore_auth_transport(
                            previous_cookie.clone(),
                            previous_data_sync_id.clone(),
                        );
                        self.restore_session_cookie_setting(previous_cookie.as_deref());
                    })?;
                return Ok(SignInOutcome::Complete);
            }
            self.activate_identity(identity, identities.len() > 1, client).await.inspect_err(
                |_| {
                    self.restore_auth_transport(
                        previous_cookie.clone(),
                        previous_data_sync_id.clone(),
                    );
                    self.restore_session_cookie_setting(previous_cookie.as_deref());
                },
            )?;
            return Ok(SignInOutcome::Complete);
        }

        // A missing/unreadable list must not replace a previously selected channel with Google's
        // current default. Revalidate the stored server-issued id directly; account_menu either
        // confirms it and refreshes metadata, or the login fails closed.
        //
        // Only when the list is empty. A list that came back and does *not* contain the persisted
        // id means these cookies belong to a different Google account, so the stored id is theirs
        // to drop: forcing it here would delegate account A's channel onto account B's cookie and
        // fail every sign-in until the user signs out first.
        if identities.is_empty() {
            if let Some(data_sync_id) = persisted_id.as_deref() {
                let identity = persisted_identity
                    .as_ref()
                    .and_then(SelectedIdentity::as_account_identity)
                    .unwrap_or_else(|| AccountIdentity {
                        name: String::new(),
                        handle: None,
                        email: None,
                        thumbnail: None,
                        channel_id: None,
                        data_sync_id: data_sync_id.to_owned(),
                        is_selected: false,
                    });
                self.activate_identity(
                    &identity,
                    persisted_identity.as_ref().is_some_and(|saved| saved.has_multiple_identities),
                    client,
                )
                .await
                .inspect_err(|_| {
                    self.restore_auth_transport(
                        previous_cookie.clone(),
                        previous_data_sync_id.clone(),
                    );
                    self.restore_session_cookie_setting(previous_cookie.as_deref());
                })?;
                return Ok(SignInOutcome::Complete);
            }
        }

        if identities.len() == 1 {
            // One channel stays seamless. Reuse the already-fetched active header instead of
            // introducing another network request or a new delegated id into the historical
            // single-account path.
            let selected = SelectedIdentity::from_account_info(active, false)
                .ok_or("That session didn't return an account identity")?;
            self.persist_selected_identity(selected, active_visitor_data.as_deref()).inspect_err(
                |_| {
                    self.restore_auth_transport(
                        previous_cookie.clone(),
                        previous_data_sync_id.clone(),
                    );
                    self.restore_session_cookie_setting(previous_cookie.as_deref());
                },
            )?;
            return Ok(SignInOutcome::Complete);
        }

        if identities.len() > 1 {
            // No persisted choice for these cookies: atomically keep the authenticated cookie,
            // mark the login unfinished, and remove stale projections. A restart will reopen the
            // required picker instead of silently acting as YouTube's default channel.
            self.it.set_data_sync_id(None);
            self.db.set_pending_auth_selection(&cookie).map_err(|error| {
                self.restore_auth_transport(previous_cookie.clone(), previous_data_sync_id.clone());
                self.restore_session_cookie_setting(previous_cookie.as_deref());
                error.to_string()
            })?;
            self.persist_visitor_data(active_visitor_data.as_deref());
            self.emit("account-selection-required", ());
            return Ok(SignInOutcome::SelectionRequired);
        }

        // No selectable token was exposed (the normal historical single-channel response). Keep
        // the seamless path and persist the active header + response-context dataSyncId together.
        let selected = SelectedIdentity::from_account_info(active, false)
            .ok_or("That session didn't return an account identity")?;
        self.persist_selected_identity(selected, active_visitor_data.as_deref()).inspect_err(
            |_| {
                self.restore_auth_transport(previous_cookie.clone(), previous_data_sync_id.clone());
                self.restore_session_cookie_setting(previous_cookie.as_deref());
            },
        )?;
        Ok(SignInOutcome::Complete)
    }

    /// Fresh switcher rows for the account picker. Raw delegated ids never cross the Tauri
    /// boundary; the UI receives a process-local opaque selector and display metadata only.
    pub async fn account_identities(&self) -> Result<Vec<serde_json::Value>, String> {
        if !self.it.is_logged_in() {
            return Err("Sign in before switching channels.".into());
        }
        let client =
            self.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let identities = self.it.account_identities(client).await.map_err(|e| e.to_string())?;
        let selected_id = self.it.data_sync_id();
        // Nothing is selected yet during a forced first pick, so YouTube's own default marker
        // would be a lie: the user hasn't chosen it, and picking it is still a required step.
        let pending = auth_selection_pending(&self.db);
        Ok(identities
            .iter()
            .map(|identity| {
                let selected = !pending
                    && selected_id
                        .as_deref()
                        .map(|id| id == identity.data_sync_id)
                        .unwrap_or(identity.is_selected);
                identity_snapshot(identity, selected)
            })
            .collect())
    }

    /// Switch without a Google re-login. The selector is resolved against a fresh server list and
    /// account_menu must succeed under a one-off context for that exact identity before the shared
    /// transport, persistence, or UI is updated.
    pub async fn switch_account(&self, selection_key: &str) -> Result<serde_json::Value, String> {
        if !self.it.is_logged_in() {
            return Err("Sign in before switching channels.".into());
        }
        let client =
            self.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let identities = self.it.account_identities(client).await.map_err(|e| e.to_string())?;
        let identity = identities
            .iter()
            .find(|identity| identity_selection_key(&identity.data_sync_id) == selection_key)
            .ok_or(
                "That YouTube channel is no longer available. Refresh the list and try again.",
            )?;
        self.activate_identity(identity, identities.len() > 1, client).await
    }

    async fn activate_identity(
        &self,
        identity: &AccountIdentity,
        has_multiple_identities: bool,
        client: &innertube::YouTubeClient,
    ) -> Result<serde_json::Value, String> {
        let refreshed =
            match self.it.account_menu_for_identity(client, &identity.data_sync_id).await {
                Ok(info) if info.name.is_some() => info,
                Ok(_) => {
                    return Err(
                        "YouTube did not confirm that channel. Try signing in again.".into()
                    );
                }
                Err(error) => {
                    return Err(format!("Couldn't switch YouTube channel: {error}"));
                }
            };
        let visitor_data = refreshed.visitor_data.clone();
        let selected =
            SelectedIdentity::from_identity(identity, refreshed, has_multiple_identities);
        self.persist_selected_identity(selected, visitor_data.as_deref())
    }

    fn persist_selected_identity(
        &self,
        selected: SelectedIdentity,
        visitor_data: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let account = selected.account_json(true);
        let selected_json = serde_json::to_string(&selected).map_err(|e| e.to_string())?;
        let account_json = account.to_string();
        let session_cookie = self
            .it
            .cookie()
            .ok_or("The YouTube session expired before the channel could be saved.")?;
        self.db
            .set_auth_identity(
                &session_cookie,
                &selected_json,
                selected.data_sync_id.as_deref(),
                &account_json,
            )
            .map_err(|e| format!("Couldn't save the selected YouTube channel: {e}"))?;
        self.persist_visitor_data(visitor_data);
        self.forget_playlist_index();
        self.it.set_data_sync_id(selected.data_sync_id.clone());
        self.emit("auth-changed", &account);
        Ok(account)
    }

    /// The playlist membership index belongs to one account, so a sign-out or a channel switch
    /// empties it. Its timestamp goes too, or the next sync would think it was still fresh.
    fn forget_playlist_index(&self) {
        self.db.clear_playlist_index();
        self.db.delete_setting("playlist_index_synced_at");
    }

    fn restore_auth_transport(&self, cookie: Option<String>, data_sync_id: Option<String>) {
        self.it.set_cookie(cookie);
        self.it.set_data_sync_id(data_sync_id);
    }

    fn restore_session_cookie_setting(&self, cookie: Option<&str>) {
        if let Some(cookie) = cookie {
            self.db.set_setting("session_cookie", cookie);
        } else {
            self.db.delete_setting("session_cookie");
        }
    }

    fn persist_visitor_data(&self, visitor_data: Option<&str>) {
        if let Some(visitor_data) = visitor_data {
            self.it.set_visitor_data(Some(visitor_data.to_owned()));
            self.db.set_setting("visitor_data", visitor_data);
        }
    }

    pub async fn sign_out(&self) {
        self.it.set_cookie(None);
        self.it.set_data_sync_id(None);
        self.db.delete_setting("session_cookie");
        self.forget_playlist_index();
        let _ = self.db.clear_auth_identity();
        self.emit("auth-changed", serde_json::json!({ "signedIn": false }));
    }

    /// Current account for the UI. New installs derive it from the canonical selected identity;
    /// legacy `account_json` remains a read fallback so existing databases migrate without a wipe.
    pub fn account_snapshot(&self) -> serde_json::Value {
        if auth_selection_pending(&self.db) && self.it.is_logged_in() {
            return serde_json::json!({
                "signedIn": true,
                "selectionRequired": true,
                "canSwitch": true,
            });
        }
        if let Some(selected) = selected_identity_from_db(&self.db) {
            return selected.account_json(self.it.is_logged_in());
        }
        let mut v = self
            .db
            .get_setting("account_json")
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        v["signedIn"] = serde_json::json!(self.it.is_logged_in());
        v
    }

    /// The UI-visible settings as a JSON object: every [`UI_SETTINGS`] key present in the DB,
    /// value as a string. This is what `get_settings` returns; the Tauri host layers its OS
    /// autostart reconciliation on top, and the daemon serves it verbatim. Kept here so the two
    /// hosts can never disagree about which settings the renderer may see.
    pub fn settings_snapshot(&self) -> serde_json::Value {
        let map: serde_json::Map<String, serde_json::Value> = self
            .db
            .all_settings()
            .into_iter()
            .filter(|(k, _)| UI_SETTINGS.contains(&k.as_str()))
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();
        serde_json::Value::Object(map)
    }

    /// The stored personal blob (Home Shortcuts, pins, play recency, Home arrangement), or `{}`
    /// when nothing has been saved yet. The daemon only stores it; its shape and reducers live in
    /// the clients (`ui/src/lib/personal.ts`, `client/lib/personal.js`).
    pub fn personal(&self) -> serde_json::Value {
        read_personal(&self.db)
    }

    /// Replace the personal blob. Rejects a non-object or a blob over [`MAX_PERSONAL_BYTES`] so a
    /// corrupt client can't wedge the store, persists it under [`PERSONAL_JSON_KEY`], then emits
    /// `personal-changed` with the blob so every other client (the mini window, a second shell)
    /// follows the same store.
    pub fn set_personal(&self, blob: serde_json::Value) -> Result<(), String> {
        let json = validate_personal_blob(&blob)?;
        self.db.set_setting(PERSONAL_JSON_KEY, &json);
        self.emit("personal-changed", &blob);
        Ok(())
    }

    async fn resolve(
        self: &std::sync::Arc<Self>,
        video_id: &str,
        is_upload: bool,
    ) -> Result<PlaybackData, ResolveError> {
        let media_ref = MediaRef::parse(video_id);
        // A local file is its own "stream": no network, no cache, no extraction (local.rs).
        // The renderer sees local ids for legitimate library rows, but it is not authoritative:
        // require the path to have come from our native scan before handing it to mpv.
        if let MediaRef::Local(path) = media_ref {
            if !self.db.has_local_track(path) {
                return Err(ResolveError::LocalMissing(path.to_owned()));
            }
            return crate::local::playback_data(video_id, path).map_err(|_| {
                // Gone since the last scan. Forget it here rather than at the next scan, and say
                // so — the UI drops the row (and any Shortcuts tile) on the spot instead of
                // leaving something that can only fail again.
                let removed = crate::local::forget_missing(&self.db, path);
                self.emit("local-changed", serde_json::json!({ "removed": removed }));
                ResolveError::LocalMissing(path.to_owned())
            });
        }
        // Live radio is already a direct stream URL. The station record is cached separately so
        // a one-item radio queue can be restored after a restart without treating its synthetic id
        // as a YouTube video id.
        if matches!(media_ref, MediaRef::Radio(_)) {
            let station = self
                .db
                .get_radio_station(video_id)
                .and_then(|json| serde_json::from_str::<crate::radio::RadioStation>(&json).ok())
                .and_then(crate::radio::normalize_station)
                .ok_or_else(|| ResolveError::AllClientsFailed(video_id.to_owned()))?;
            return Ok(crate::radio::playback_data(&station));
        }
        // A Spotify track streams as raw PCM through a per-session FIFO, not a cacheable URL, so it
        // resolves ahead of the latency cache / HEAD / orchestrator path and never touches any of
        // them. Opening the stream starts librespot decoding into the FIFO immediately; mpv is
        // pointed at that FIFO with the rawaudio demuxer options.
        if let MediaRef::SpotifyTrack(tid) = media_ref {
            let Some(client) = self.spotify.client().await else {
                return Err(ResolveError::SignInRequired(
                    "Sign in to Spotify to play this".to_owned(),
                ));
            };
            if !client.can_stream() {
                return self.resolve_spotify_via_youtube(video_id, tid, &client).await;
            }
            let handle = client
                .stream(tid)
                .map_err(|e| ResolveError::AllClientsFailed(format!("{video_id}: {e}")))?;
            let fifo = handle.fifo_path().to_string_lossy().into_owned();
            // Watch librespot's events off the mpv pump. An Unavailable track never fills the FIFO,
            // so mpv would just block forever — the watcher surfaces it as an error + skip.
            let watcher = {
                let me = std::sync::Arc::clone(self);
                let vid = video_id.to_owned();
                tokio::spawn(async move { me.watch_spotify_stream(vid, handle).await })
            };
            let previous = self.spotify_stream.lock().replace(SpotifyStream { watcher });
            if let Some(prev) = previous {
                prev.watcher.abort();
            }
            return Ok(PlaybackData {
                video_id: video_id.to_owned(),
                stream_url: fifo,
                itag: 0,
                mpv_options: spotify_mpv_options(),
                headers: Default::default(),
                expires_in_seconds: 0,
                loudness_db: None,
                playback_ping: None,
                // The queue item already carries title/artists/thumbnail/duration; skip the extra
                // `client.track` round-trip so playback starts without waiting on it.
                title: None,
                artists: None,
                duration: None,
                thumbnail: None,
                is_video: Some(false),
                stream_client: "spotify".to_owned(),
            });
        }
        // Latency cache first (UI state) — honor expiry, never a source of truth.
        // 60s safety margin: a URL that expires mid-load/mid-buffer fails as Raw(-13).
        let now = now_secs();
        if let Some(c) = self.db.get_stream(video_id, now + 60) {
            tracing::debug!(video_id, "stream url cache hit");
            // Cached URL carries no fresh metadata; the UI already has it from the queue item.
            return Ok(PlaybackData {
                video_id: video_id.to_owned(),
                stream_url: c.url,
                itag: c.itag,
                mpv_options: Vec::new(),
                headers: Default::default(),
                expires_in_seconds: c.expires_at - now,
                loudness_db: c.loudness_db,
                playback_ping: c
                    .ping_url
                    .zip(c.ping_client)
                    .map(|(url, client)| PlaybackPing { url, client }),
                title: None,
                artists: None,
                duration: None,
                thumbnail: None,
                // Cached alongside the URL: a hit skips `/player`, so without it a replay inside
                // the cache window (hours, and every track you just listened to) would fall back
                // to the queue row's flag, which is exactly the thing that can't be trusted.
                is_video: c.is_video,
                stream_client: "cache".to_owned(),
            });
        }
        // A SoundCloud track streams over plain HTTPS: an HLS m3u8 mpv reads directly, no FIFO and
        // no YouTube-specific headers. Its signed URL expires in ~1h, so cap the latency cache at
        // 30 min and re-resolve after. Resolving fetches the track (transcodings + counts) then
        // picks an HLS transcoding; a cache hit above already returned without any of this.
        if let MediaRef::SoundCloudTrack(id) = media_ref {
            let track = self
                .soundcloud
                .track(id)
                .await
                .map_err(|e| ResolveError::AllClientsFailed(format!("{video_id}: {e}")))?;
            let url = self
                .soundcloud
                .stream_url(&track)
                .await
                .map_err(|e| ResolveError::AllClientsFailed(format!("{video_id}: {e}")))?;
            self.remember_sc_meta(video_id, &track);
            const SC_CACHE_SECS: i64 = 30 * 60;
            self.db.put_stream(
                video_id,
                &crate::db::CachedStream {
                    url: url.clone(),
                    itag: 0,
                    expires_at: now + SC_CACHE_SECS,
                    loudness_db: None,
                    is_video: Some(false),
                    ping_url: None,
                    ping_client: None,
                },
                now,
            );
            return Ok(PlaybackData {
                video_id: video_id.to_owned(),
                stream_url: url,
                itag: 0,
                mpv_options: Vec::new(),
                headers: Default::default(),
                expires_in_seconds: SC_CACHE_SECS,
                loudness_db: None,
                playback_ping: None,
                title: None,
                artists: None,
                duration: None,
                thumbnail: None,
                is_video: Some(false),
                stream_client: "soundcloud".to_owned(),
            });
        }
        // A TIDAL track streams over plain HTTPS too, but in one of two shapes: a progressive file
        // mpv opens directly, or a DASH presentation that arrives inline and has to be written out
        // first. Neither is cached: the urls are short-lived and signed per request, and a stale
        // one fails mid-track rather than at resolve time.
        if let MediaRef::TidalTrack(id) = media_ref {
            let want = self.tidal_quality();
            // A signed-out TIDAL is a sign-in prompt, not a dead row: flattening it into
            // AllClientsFailed sends the queue handler down the skip path, and a whole TIDAL
            // queue then walks itself to the tail one "Skipped" toast at a time.
            let stream = self.tidal.stream(id, want).await.map_err(|e| match e {
                ryotunes_tidal::Error::SignedOut => {
                    ResolveError::SignInRequired("Sign in to TIDAL to play this".to_owned())
                }
                e => ResolveError::AllClientsFailed(format!("{video_id}: {e}")),
            })?;
            let url = match stream.source {
                ryotunes_tidal::Source::Direct(url) => url,
                ryotunes_tidal::Source::Presentation { xml } => {
                    let path = self.paths.data_dir.join("tidal").join(format!("{id}.mpd"));
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    std::fs::write(&path, xml)
                        .map_err(|e| ResolveError::AllClientsFailed(format!("{video_id}: {e}")))?;
                    format!("file://{}", path.display())
                }
            };
            return Ok(PlaybackData {
                video_id: video_id.to_owned(),
                stream_url: url,
                itag: 0,
                mpv_options: Vec::new(),
                headers: Default::default(),
                expires_in_seconds: 0,
                loudness_db: None,
                playback_ping: None,
                title: None,
                artists: None,
                duration: None,
                thumbnail: None,
                is_video: Some(false),
                stream_client: "tidal".to_owned(),
            });
        }
        let data = match self
            .orchestrator
            .resolve(video_id, is_upload, self.quality(), &self.disabled_clients())
            .await
        {
            Ok(d) => d,
            // Every InnerTube client answered "confirm you're not a bot": the session's
            // visitorData is missing or stale (YouTube only honours the anonymous fallback
            // clients with one). Fetch a fresh identity — once per cooldown window — and retry
            // this resolve exactly once. Without this the one-shot startup bootstrap leaving a
            // user unable to play ANY track until a restart was unrepairable.
            Err(ResolveError::BotGated(vid)) => {
                if self.refresh_visitor_data().await.is_none() {
                    return Err(ResolveError::AllClientsFailed(vid));
                }
                self.orchestrator
                    .resolve(video_id, is_upload, self.quality(), &self.disabled_clients())
                    .await
                    .map_err(|e| match e {
                        // The fresh token didn't help (or the IP itself is flagged): report the
                        // same generic failure users saw before, never the internal variant.
                        ResolveError::BotGated(v) => ResolveError::AllClientsFailed(v),
                        other => other,
                    })?
            }
            Err(e) => return Err(e),
        };
        // Never cache rustypipe URLs: googlevideo serves them only for bounded-Range requests,
        // which mpv doesn't send → LOADING_FAILED(-13). Caching one poisons the videoId for ~6h.
        if data.stream_client != "rustypipe" && !is_upload {
            self.db.put_stream(
                video_id,
                &crate::db::CachedStream {
                    url: data.stream_url.clone(),
                    itag: data.itag,
                    expires_at: now + data.expires_in_seconds.max(0),
                    loudness_db: data.loudness_db,
                    is_video: data.is_video,
                    ping_url: data.playback_ping.as_ref().map(|p| p.url.clone()),
                    ping_client: data.playback_ping.as_ref().map(|p| p.client.clone()),
                },
                now,
            );
        }
        Ok(data)
    }

    /// The on-demand visitorData (re)bootstrap a bot-gated resolve triggers: fetch a fresh
    /// anonymous identity from sw.js_data, install it in the shared session, and persist it so
    /// the next launch starts with one. Returns `Some(vd)` when a *new* token is in place,
    /// `None` when the fetch failed or the cooldown suppressed the attempt — callers then treat
    /// the gate as a plain failure rather than hammering Google.
    ///
    /// Cooldown lives here rather than in the orchestrator's retry because a whole broken queue
    /// skips track-per-track: every skip would otherwise pay one sw.js_data round trip.
    pub async fn refresh_visitor_data(self: &std::sync::Arc<Self>) -> Option<String> {
        const RETRY_COOLDOWN_SECS: u64 = 5 * 60;
        let now = now_secs().max(0) as u64;
        let last = self.last_visitor_try.swap(now, Ordering::Relaxed);
        if last != 0 && now.saturating_sub(last) < RETRY_COOLDOWN_SECS {
            tracing::debug!("visitorData refresh on cooldown; skipping");
            return None;
        }
        match self.it.fetch_visitor_data().await {
            Ok(vd) => {
                tracing::info!("visitorData re-bootstrapped after bot gate");
                self.it.set_visitor_data(Some(vd.clone()));
                self.db.set_setting("visitor_data", &vd);
                Some(vd)
            }
            Err(e) => {
                tracing::warn!(error = %e, "visitorData refresh failed");
                None
            }
        }
    }

    /// Play a Spotify track as its YouTube Music equivalent where librespot has no audio path.
    /// The match is remembered per track, so a replay skips the search. The returned data keeps the
    /// Spotify id, so the queue row and the UI stay on the Spotify track.
    async fn resolve_spotify_via_youtube(
        self: &std::sync::Arc<Self>,
        video_id: &str,
        track_id: &str,
        client: &ryotunes_spotify::Client,
    ) -> Result<PlaybackData, ResolveError> {
        let key = format!("{SPOTIFY_YOUTUBE_MATCH_PREFIX}{track_id}");
        let youtube_id = match self.db.get_setting(&key) {
            Some(id) => id,
            None => {
                let track = client
                    .track(track_id)
                    .await
                    .map_err(|e| ResolveError::AllClientsFailed(format!("{video_id}: {e}")))?;
                let artists: Vec<String> = if track.artist_refs.is_empty() {
                    track.artists.split(", ").map(str::to_owned).collect()
                } else {
                    track.artist_refs.iter().map(|artist| artist.name.clone()).collect()
                };
                let source = crate::catalog_match::MatchSource {
                    title: track.name.clone(),
                    artists,
                    duration_ms: u64::try_from(track.duration.as_millis())
                        .ok()
                        .filter(|ms| *ms > 0),
                };
                let id = self.find_youtube_equivalent(&source).await.ok_or_else(|| {
                    ResolveError::NoMatch(format!(
                        "No YouTube Music match for {} — {}",
                        track.name, track.artists
                    ))
                })?;
                self.db.set_setting(&key, &id);
                id
            }
        };
        let mut data = Box::pin(self.resolve(&youtube_id, false)).await?;
        tracing::info!(video_id, youtube_id, "spotify: playing the YouTube Music equivalent");
        data.video_id = video_id.to_owned();
        data.stream_client = format!("youtube-match/{}", data.stream_client);
        Ok(data)
    }

    /// Search YouTube Music for `source`: first by title and primary artist, then by title alone
    /// when that finds nothing valid. Only a candidate that passes every matching rule is used.
    async fn find_youtube_equivalent(
        &self,
        source: &crate::catalog_match::MatchSource,
    ) -> Option<String> {
        if !crate::catalog_match::searchable(source) {
            return None;
        }
        let queries = [format!("{} {}", source.title, source.artists[0]), source.title.clone()];
        for query in queries {
            let Ok(songs) = self.search(&query).await else { continue };
            let candidates: Vec<_> = songs
                .iter()
                .filter(|song| !song.is_video)
                .map(|song| crate::catalog_match::MatchCandidate {
                    id: song.video_id.clone(),
                    title: song.title.clone(),
                    artists: song_artists(song),
                    duration_ms: song
                        .duration
                        .as_deref()
                        .and_then(crate::catalog_match::duration_ms),
                })
                .collect();
            if let Some(index) = crate::catalog_match::best_for_playback(source, &candidates) {
                return Some(candidates[index].id.clone());
            }
        }
        None
    }

    /// Speculatively resolve one stream into the normal latency cache without touching playback.
    /// The UI calls this only after settled search intent / a short hover, so a subsequent Play can
    /// skip the /player + PoToken + HEAD waterfall and go straight to mpv. Failures are silent.
    pub async fn prefetch_stream(self: &std::sync::Arc<Self>, video_id: String, is_upload: bool) {
        // A Spotify id would open a real librespot stream into the shared FIFO - never on spec.
        // Without the FIFO it resolves to its YouTube Music equivalent, which is safe to warm.
        if crate::spotify::streams_via_fifo(&video_id) {
            return;
        }
        let _ = self.resolve(&video_id, is_upload).await;
    }

    /// Start one Internet Radio station as a native live stream. Listen Together deliberately
    /// rejects it for v2.4 because remote peers cannot resolve this machine's cached station record.
    pub async fn play_radio_station(
        self: &std::sync::Arc<Self>,
        station: crate::radio::RadioStation,
    ) -> Result<(), String> {
        if self.lt.is_guest().await || self.lt.is_host().await {
            return Err("Live Radio is unavailable while Listen Together is active.".into());
        }
        let station = crate::radio::normalize_station(station)
            .ok_or_else(|| "This station did not provide a playable HTTP(S) stream.".to_string())?;
        let item = crate::radio::song_item(&station);
        let json = serde_json::to_string(&station).map_err(|e| e.to_string())?;
        self.db.put_radio_station(&item.video_id, &json);

        let uuid = station.station_uuid.clone();
        tokio::spawn(async move {
            crate::radio::count_click(&uuid).await;
        });

        self.play_tracks(vec![item], Some(0), None, Some("Live Radio".into()), false, None).await;
        Ok(())
    }

    /// Start a fresh queue from one track (a search-result click), then hydrate the radio via
    /// `next` and prime the gapless lookahead.
    pub async fn play_song(self: &std::sync::Arc<Self>, seed: SongItem) {
        if self.lt.is_guest().await {
            // Guests follow the host; clicking a song adds it to the shared queue instead
            // (Spotify-Jam-style). The host client auto-approves and stamps who added it.
            self.lt.suggest(song_to_track(&seed)).await;
            return;
        }
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let video_id = seed.video_id.clone();

        {
            let mut q = self.queue.lock().await;
            // Unplayed manual adds survive a context switch (Spotify semantics): they follow the
            // new track, ahead of its radio (hydration appends behind them).
            let mut carried = upcoming_queued(&q.items, q.current);
            // A local file has no radio behind it (see below), so don't promise one in the header.
            q.source_name = (!matches!(MediaRef::parse(&seed.video_id), MediaRef::Local(_)))
                .then(|| format!("{} Radio", seed.title));
            q.items = vec![seed];
            q.items.append(&mut carried);
            q.current = 0;
            q.played_from = 0; // new queue, nothing played in it yet
            q.lookahead_loaded = None;
            q.radio_seed = None; // single-song queue → autoplay re-seeds from the last track
            q.radio = false;
            // Shuffle is sticky across queues: keep it ON (re-snapshotted after radio hydration).
            q.shuffle_orig = q.shuffle_orig.is_some().then(|| q.items.clone());
        }

        if !self.start_current(gen).await {
            return;
        }

        // A local file isn't a videoId YouTube has ever heard of: asking for its radio is a
        // guaranteed-useless request, and offline (where local music earns its keep) it's a
        // guaranteed-failing one.
        if matches!(MediaRef::parse(&video_id), MediaRef::Local(_) | MediaRef::Radio(_)) {
            self.prime_lookahead(gen).await;
            return;
        }

        // Hydrate up-next radio (browse parser) — non-fatal if it fails. Seed the radio playlist
        // directly (`RDAMVM<videoId>`): a bare next(videoId) returns only the seed song + an
        // automixPreviewVideoRenderer, so the queue would never grow past one track.
        let radio_id = format!("RDAMVM{video_id}");
        match self
            .it
            .next(
                self.clients.get(innertube::METADATA_CLIENT).unwrap(),
                Some(&video_id),
                Some(&radio_id),
            )
            .await
        {
            Ok(next) => {
                let mut q = self.queue.lock().await;
                if self.generation.load(Ordering::SeqCst) != gen {
                    return; // superseded
                }
                for item in next.items {
                    if item.video_id != video_id {
                        q.items.push(item);
                    }
                }
                // Shuffle on → the radio hydration is part of the queue: snapshot it as the
                // "original" order, then shuffle the upcoming tracks. (Runs before the lookahead
                // is primed, so nothing stale is in mpv.)
                if q.shuffle_orig.is_some() {
                    q.shuffle_orig = Some(q.items.clone());
                    let cur = q.current;
                    shuffle_upcoming(&mut q.items, cur);
                }
                drop(q);
                self.emit_queue().await;
                // The early autoplay extension may already have persisted a shorter queue; without
                // this, a restart restores whichever of the two happened to be written last.
                self.persist_queue().await;
            }
            Err(e) => tracing::warn!(error = %e, "next() radio hydration failed"),
        }

        self.prime_lookahead(gen).await;
    }

    /// Play a finite list of tracks (a playlist/album). `start` is the clicked track; `None`
    /// means "just play the playlist" (the header Play button) — first track, or any track when
    /// shuffle is on. Unlike `play_song` this seeds NO radio — the given items *are* the queue
    /// (browse parser: playlist playback). `source_id` (the page's playlist/album playlist id) makes
    /// autoplay continue with that context's radio when the queue runs out. `source_name` is the
    /// page title for the queue panel's "Next from" header; `shuffle` (the page Shuffle buttons)
    /// turns shuffle ON for this queue — the backend owns the randomization, so un-shuffle can
    /// restore the true order and every re-shuffle is fresh.
    ///
    /// `continuation` is the playlist page's next-page token, if it has one: the queue is seeded
    /// from the tracks the page had loaded and the rest is walked in the background
    /// ([`Self::fill_playlist`]) so a long playlist starts playing now instead of after ~50
    /// chained round trips.
    #[allow(clippy::too_many_arguments)]
    pub async fn play_tracks(
        self: &std::sync::Arc<Self>,
        items: Vec<SongItem>,
        start: Option<usize>,
        source_id: Option<String>,
        source_name: Option<String>,
        shuffle: bool,
        continuation: Option<String>,
    ) {
        if items.is_empty() {
            return;
        }
        if self.lt.is_guest().await {
            self.emit_guest_hint();
            return;
        }
        // A mix has no "rest of the playlist" worth walking (see `is_mix`) — drop the token.
        let continuation = continuation.filter(|_| !is_mix(source_id.as_deref()));
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut q = self.queue.lock().await;
            // Sticky across queues, or explicitly requested by a page Shuffle button.
            let keep_shuffled = q.shuffle_orig.is_some() || shuffle;
            let start = match start {
                Some(i) => i.min(items.len() - 1),
                None if keep_shuffled => {
                    rand::Rng::gen_range(&mut rand::thread_rng(), 0..items.len())
                }
                None => 0,
            };
            // Unplayed manual adds survive a context switch (Spotify semantics) — spliced back in
            // right after the new current track, ahead of the new context.
            let carried = upcoming_queued(&q.items, q.current);
            q.items = items;
            q.current = start;
            q.lookahead_loaded = None;
            q.radio_seed = radio_seed_for(source_id);
            q.source_name = source_name;
            q.radio = false; // a chosen playlist/album; `start_radio` sets it back on for its own
            if keep_shuffled {
                // Snapshot the real playlist order (for un-shuffle), then play the clicked track
                // first with everything else shuffled behind it. Carried adds are spliced in
                // after — un-shuffle drops them, same as adds made while shuffled (snapshot
                // semantics, see toggle_shuffle).
                q.shuffle_orig = Some(q.items.clone());
                let start = q.current;
                q.current = shuffle_new_queue(&mut q.items, start);
            }
            // Whatever the queue starts on is where the played run starts: the tracks in front of
            // a playlist opened at track 7 were never heard.
            q.played_from = q.current;
            let at = q.current + 1;
            for (k, item) in carried.into_iter().enumerate() {
                q.items.insert(at + k, item);
            }
        }
        // start_current emits now-playing + queue + persists; prime the gapless lookahead after.
        if self.start_current(gen).await {
            self.prime_lookahead(gen).await;
        }
        if let Some(token) = continuation {
            let me = self.clone();
            tokio::spawn(async move { me.fill_playlist(gen, token, Fill::Playing).await });
        }
    }

    /// Start a radio (browse parser): an endless YouTube-generated queue seeded on a song, artist,
    /// album or playlist. `kind` is one of `song` / `artist` / `album` / `playlist` and `id` the
    /// matching videoId or browseId; `name` is what the queue header calls it.
    ///
    /// A radio *is* a playlist id — the whole feature is a prefix convention plus one `/next`
    /// call. Songs and playlists build theirs client-side (`RDAMVM…` / `RDAMPL…`); artists can't,
    /// so theirs comes off the artist page (`radio_playlist_id`). Once started it continues
    /// through the same autoplay path as any other queue ([`Self::extend_queue_radio`]), which is
    /// why this only has to install a first page and a seed.
    pub async fn start_radio(
        self: &std::sync::Arc<Self>,
        kind: &str,
        id: &str,
        name: Option<String>,
    ) -> Result<(), String> {
        if self.lt.is_guest().await {
            self.emit_guest_hint();
            return Ok(());
        }
        // Guarded here rather than in each menu: everything without a YouTube item behind it
        // (files on disk, the locally-built On Repeat) would otherwise reach `/next` as an id
        // YouTube has never heard of.
        if matches!(MediaRef::parse(id), MediaRef::Local(_))
            || id.starts_with(crate::local::ALBUM_PREFIX)
            || id.starts_with(crate::local::ARTIST_PREFIX)
            || id == ON_REPEAT_ID
        {
            return Err("This has no radio behind it.".into());
        }
        let client = self.clients.get(innertube::METADATA_CLIENT).ok_or("no metadata client")?;
        // Resolve the seed to (videoId?, radio playlist id). Album and artist need a page fetch:
        // an album's radio keys off its audio playlist, not its `MPRE…` browseId, and an artist
        // radio id is server-supplied.
        let (video_id, playlist_id) = match kind {
            "song" => (Some(id.to_owned()), format!("RDAMVM{id}")),
            "playlist" => (None, radio_seed_for(Some(id.to_owned())).unwrap()),
            "album" => {
                let page = self.it.album(client, id).await.map_err(|e| e.to_string())?;
                let pl = page.playlist_id.ok_or("This album has no radio.")?;
                (None, radio_seed_for(Some(pl)).unwrap())
            }
            "artist" => {
                let page = self.it.artist(client, id).await.map_err(|e| e.to_string())?;
                match page.radio_playlist_id {
                    Some(pl) => (None, pl),
                    // No radio button on the header: fall back to a radio on this artist's most
                    // played track, which is roughly what that button seeds anyway.
                    None => {
                        let top = page.top_songs.first().ok_or("This artist has no radio.")?;
                        (Some(top.video_id.clone()), format!("RDAMVM{}", top.video_id))
                    }
                }
            }
            other => return Err(format!("unknown radio kind: {other}")),
        };

        let (items, seed) = self.fetch_radio(video_id.as_deref(), &playlist_id).await?;
        let title = name.map(|n| format!("{n} Radio"));

        // Radio on the song that's already playing: splice instead of replacing, so the track
        // keeps playing without a re-buffer (Metrolist's `startRadioSeamlessly`).
        let playing_seed = {
            let q = self.queue.lock().await;
            video_id.is_some() && q.items.get(q.current).map(|i| &i.video_id) == video_id.as_ref()
        };
        if playing_seed && !self.player.is_idle() {
            self.splice_radio(items, seed, title).await;
            return Ok(());
        }
        // The seed song comes back inside the first page (usually first, but the panel decides) —
        // start there so a radio on song X actually opens on X.
        let start = video_id
            .as_ref()
            .and_then(|v| items.iter().position(|i| &i.video_id == v))
            .unwrap_or(0);
        self.play_tracks(items, Some(start), Some(seed), title, false, None).await;
        self.queue.lock().await.radio = true;
        Ok(())
    }

    /// Fetch a radio's first page, escalating when YouTube hands back a dead one. Returns the
    /// tracks plus the playlist id that actually produced them (autoplay's seed, which is not
    /// necessarily the one asked for).
    ///
    /// A `RDAMVM…` radio for an obscure or region-locked track routinely answers with the seed
    /// song and nothing else, and "start radio" then looks like it did nothing. So: ask the song
    /// what mix it belongs to (`automixPreviewVideoRenderer`) and take that instead.
    ///
    /// Note: two rungs, not Metrolist's three — the third scrapes the Related tab, which needs
    /// a whole endpoint + an ATV-only filter to salvage the cases these two already miss. Add it
    /// if songs turn up that reach here and still come back empty.
    async fn fetch_radio(
        &self,
        video_id: Option<&str>,
        playlist_id: &str,
    ) -> Result<(Vec<SongItem>, String), String> {
        const NO_RADIO: &str = "YouTube has no radio for this.";
        let client = self.clients.get(innertube::METADATA_CLIENT).ok_or("no metadata client")?;
        let first =
            self.it.next(client, video_id, Some(playlist_id)).await.map_err(|e| e.to_string())?;
        if first.items.len() > 1 {
            return Ok((first.items, playlist_id.to_owned()));
        }
        let Some(video_id) = video_id else { return Err(NO_RADIO.into()) };
        let bare = self.it.next(client, Some(video_id), None).await.map_err(|e| e.to_string())?;
        if let Some(mix) = bare.automix_playlist_id {
            let page = self
                .it
                .next(client, Some(video_id), Some(&mix))
                .await
                .map_err(|e| e.to_string())?;
            if page.items.len() > 1 {
                return Ok((page.items, mix));
            }
        }
        if bare.items.len() > 1 {
            return Ok((bare.items, format!("RDAMVM{video_id}")));
        }
        Err(NO_RADIO.into())
    }

    /// Install a radio behind the track that's already playing: history and the current song stay,
    /// everything after them becomes the radio. Manual "add to queue" items are carried rather
    /// than destroyed (same rule as a context switch in `play_tracks`) — Metrolist silently drops
    /// them, which on a desktop app is just losing the user's work.
    async fn splice_radio(
        self: &std::sync::Arc<Self>,
        items: Vec<SongItem>,
        seed: String,
        title: Option<String>,
    ) {
        {
            let mut q = self.queue.lock().await;
            splice_radio_into(&mut q, items, seed, title);
            // Whatever mpv had primed as the gapless next belongs to the old queue.
            if q.lookahead_loaded.take().is_some() {
                let _ = self.player.clear_playlist();
            }
        }
        self.emit_queue().await;
        self.persist_queue().await;
        self.prime_lookahead(self.generation.load(Ordering::SeqCst)).await;
        self.lt_broadcast_queue().await;
    }

    /// Walk the rest of a playlist in the background and append it to the playing queue, page by
    /// page. The alternative (what the UI used to do) is loading every page *before* starting
    /// playback: continuation tokens are chained, so that's ~50 sequential round trips on a
    /// 5000-track playlist, all of them before the first note.
    ///
    /// Shuffle stays a real shuffle. Each arriving page is mixed through the *unplayed* tail
    /// ([`append_page`]), so the only track not drawn from the full playlist is the one already
    /// playing — and the walk is long finished before it ends.
    ///
    /// Guarded by `gen`: if the user starts something else mid-walk, the pages are dropped rather
    /// than appended to a queue they don't belong to. [`Fill`] says which queue the pages join.
    async fn fill_playlist(self: &std::sync::Arc<Self>, gen: u64, mut token: String, fill: Fill) {
        // Note: ~5k tracks at 100/page. A bound so a playlist that keeps handing out tokens
        // can't walk forever; raise it if a real playlist ever hits the cap.
        const MAX_PAGES: usize = 50;
        let Some(client) = self.clients.get(innertube::METADATA_CLIENT) else { return };
        let mut pages = 0;
        // The first page goes out at once so the panel visibly starts filling; after that the
        // walk emits at most once a second. Every page is a *full* payload (the rows changed),
        // and a 50-page walk of a 5,000-track playlist sends ~67 MB of JSON through
        // `webview.eval` if each one is announced. The final state is emitted after the loop.
        let mut last_emit: Option<std::time::Instant> = None;
        for _ in 0..MAX_PAGES {
            let page = match self.it.playlist_continuation(client, &token).await {
                Ok(page) => page,
                // A page that fails ends the walk — better a short queue than a stalled one.
                Err(e) => {
                    tracing::warn!(error = %e, "playlist fill stopped early");
                    break;
                }
            };
            if self.generation.load(Ordering::SeqCst) != gen {
                return; // another queue owns the state now — don't touch it, don't persist
            }
            if page.items.is_empty() {
                break; // an empty page is the end, token or not
            }
            let mut items = page.items;
            // The rest of an added playlist belongs to the same block: same markers, same heading,
            // and its order left alone (shuffle is about the playlist that's playing).
            if let Fill::Queued(from) = &fill {
                for item in &mut items {
                    item.queued_end = true;
                    item.queued_from = from.clone();
                }
            }
            {
                let mut q = self.queue.lock().await;
                append_page(&mut q, items, matches!(fill, Fill::Playing));
                // An append can retarget a primed repeat-all wrap (index 0 → the new tail); drop
                // the lookahead when it stops pointing at what plays next, same check as
                // `insert_queued_song`. `append_page` leaves a still-valid slot alone, so the
                // common case re-primes to a no-op instead of re-resolving on every page.
                let expected = next_index(q.items.len(), q.current, q.repeat);
                if q.lookahead_loaded.is_some() && q.lookahead_loaded != expected {
                    q.lookahead_loaded = None;
                    let _ = self.player.clear_playlist();
                }
            }
            pages += 1;
            if last_emit.is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(1)) {
                last_emit = Some(std::time::Instant::now());
                self.emit_queue().await;
            }
            self.prime_lookahead(gen).await;
            match page.continuation {
                Some(next) => token = next,
                None => break,
            }
        }
        // Persist/broadcast once at the end, not per page: the queue JSON grows with every page and
        // nothing else reads it mid-walk.
        if pages > 0 {
            tracing::info!(pages, "playlist fill appended pages");
            self.emit_queue().await; // whatever the throttle above held back
            self.persist_queue().await;
            self.lt_broadcast_queue().await;
        }
    }

    /// Play/pause toggle that also starts a *restored* (or exhausted) queue: if mpv has nothing
    /// loaded but the queue is non-empty, load the current track (applying any resume position);
    /// otherwise just toggle mpv. Keeps the UI's single play/pause button working after a restart.
    pub async fn resume_or_toggle(self: &std::sync::Arc<Self>) {
        if self.lt.is_guest().await {
            return; // guest playback is host-driven
        }
        if !self.player.has_loaded_media() {
            let (idx, has_items) = {
                let q = self.queue.lock().await;
                (q.current, !q.items.is_empty())
            };
            if has_items {
                self.play_index(idx).await;
                return;
            }
        }
        let _ = self.player.toggle();
    }

    /// Deterministic MPRIS Play: resume if media is loaded, or start the restored/current queue
    /// entry when mpv is idle. Unlike Toggle, receiving Play while already playing is a no-op.
    /// This path uses the event-driven loaded-media mirror rather than synchronously querying mpv.
    pub async fn resume(self: &std::sync::Arc<Self>) {
        if self.lt.is_guest().await {
            return;
        }
        if !self.player.has_loaded_media() {
            let (idx, has_items) = {
                let q = self.queue.lock().await;
                (q.current, !q.items.is_empty())
            };
            if has_items {
                self.play_index(idx).await;
            }
            return;
        }
        let _ = self.player.play();
    }

    /// Jump to a specific queue index.
    pub async fn play_index(self: &std::sync::Arc<Self>, index: usize) {
        if self.lt.is_guest().await {
            self.emit_guest_hint(); // guest playback is host-driven
            return;
        }
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut q = self.queue.lock().await;
            if index >= q.items.len() {
                return;
            }
            q.seek_to(index);
            q.lookahead_loaded = None;
        }
        if self.start_current(gen).await {
            self.prime_lookahead(gen).await;
        }
    }

    /// Advance the queue after a track ended (EOF) or died (load error). Don't assume mpv
    /// gaplessly transitioned — ask it: if a lookahead was primed mpv is already playing the
    /// next entry (just sync pointer + UI); if mpv went idle (lookahead absent/failed, or the
    /// track errored on a single-entry playlist) load the next track explicitly, otherwise
    /// playback silently stalls while the UI shows a phantom "now playing".
    pub async fn on_track_ended(self: &std::sync::Arc<Self>) {
        if self.lt.is_guest().await {
            return; // the host drives track changes for guests; don't auto-advance locally
        }
        // "Stop after current" is deliberately checked before queue mutation/autoplay. Enabling it
        // also clears mpv's primed lookahead, so EOF cannot race into the next URL before this event
        // reaches us. It is one-shot: play/next afterwards behaves normally.
        if self.stop_after_current.swap(false, Ordering::SeqCst) {
            tracing::info!("stop-after-current reached");
            self.emit("stop-after-current", false);
            self.emit("playback-state", "paused");
            self.media_set_playing(false);
            self.persist_queue().await;
            return;
        }
        let (has_next, primed) = {
            let mut q = self.queue.lock().await;
            match next_index(q.items.len(), q.current, q.repeat) {
                Some(next) => {
                    let primed = q.lookahead_loaded == Some(next);
                    q.seek_to(next); // repeat-all wraps to 0, which starts the played run over
                    (true, primed)
                }
                None => (false, false),
            }
        };
        if !has_next {
            // Try autoplay before declaring the queue dead (the early trigger usually already
            // extended it; this is the fallback when that didn't land in time). Off the pump:
            // spawn, and let the task either continue playback or do the pause bookkeeping —
            // pausing here and un-pausing a second later would flicker every consumer (UI,
            // MPRIS, Discord).
            let me = self.clone();
            tokio::spawn(async move {
                let gen = me.generation.fetch_add(1, Ordering::SeqCst) + 1;
                if me.extend_queue_radio(gen).await > 0 {
                    {
                        let mut q = me.queue.lock().await;
                        q.current += 1; // the first appended track
                        q.lookahead_loaded = None; // start_current's loadfile replaces mpv's playlist
                    }
                    if me.start_current(gen).await {
                        me.prime_lookahead(gen).await;
                    }
                } else {
                    tracing::info!("queue exhausted");
                    me.emit("playback-state", "paused");
                    // mpv goes idle without flipping its pause flag, so no Paused event will fire
                    // — tell the OS widget + Discord ourselves or they show "playing" forever
                    // past the last song.
                    me.media_set_playing(false);
                }
            });
            return;
        }
        if !primed || self.player.is_idle() {
            // No gapless handoff happened. `is_idle` alone is not enough: right at EOF mpv can
            // still report not-idle before it settles, and with nothing primed that would send us
            // into the gapless branch below — a phantom "now playing" over a silent player. If we
            // never primed this index, mpv cannot have advanced into it; load explicitly. Bump the
            // generation so any in-flight lookahead resolve discards itself (double-enqueue).
            let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            tracing::info!("no primed lookahead at track end — loading next explicitly");
            // start_current's loadfile *replaces* mpv's playlist, so whatever was recorded here is
            // gone either way — clear it like every other load path (play_index, on_track_failed,
            // play_tracks) does. Left stale, the re-prime below can no-op as "already primed"
            // against an entry mpv no longer holds, and the next track end finds nothing to
            // advance into.
            self.queue.lock().await.lookahead_loaded = None;
            if self.start_current(gen).await {
                self.prime_lookahead(gen).await;
            }
            return;
        }
        // mpv already advanced into the primed lookahead. Sync pointer + UI, prime the next.
        let gen = self.generation.load(Ordering::SeqCst);
        {
            let mut q = self.queue.lock().await;
            // `af` is global in mpv and the appended entry couldn't carry its own, so the new track
            // is playing at the *previous* one's gain until this lands.
            if let Some(gain) = q.lookahead_gain.take() {
                if let Err(e) = self.player.set_gain(gain) {
                    tracing::warn!(error = %e, "applying lookahead loudness gain failed");
                }
            }
            // The primed entry is what's playing now — nothing is primed beyond it. (Also what
            // lets a single-item repeat-all queue re-prime itself instead of "already primed".)
            q.lookahead_loaded = None;
            q.current_client = q.lookahead_client.take();
            // New track is now playing → fresh history state (mirrors start_current).
            q.playback_ping = q.lookahead_playback_ping.take();
            q.cpn = innertube::generate_cpn();
            q.history_pinged = false;
            q.duration = 0.0;
        }
        if let Some(item) = self.current_item().await {
            self.emit_now_playing(&item, "gapless");
            self.refresh_rating(&item.video_id, gen);
        }
        self.emit_queue().await;
        self.persist_queue().await; // index advanced without an explicit load → persist it
                                    // Listen Together host: announce the gapless advance to the room.
        self.lt_broadcast_current_track(0, true).await;
        tracing::info!("advanced to next track (gapless)");
        // Prime off the pump, not on it. `prime_lookahead` resolves the next stream over the
        // network, and this fn is awaited by the mpv event pump — blocking here stops mpv's events
        // being drained for the length of a round-trip, which delays the new track's `duration`
        // (its progress bar) and, worse, the *next* track-end. The generation guard inside already
        // makes a superseded resolve discard itself, so it's safe to detach.
        let me = self.clone();
        tokio::spawn(async move {
            me.prime_lookahead(gen).await;
            me.extend_queue_radio(gen).await; // autoplay early trigger (no-op unless tail is near)
        });
    }

    /// A track died at the player layer (dead/403 URL). If WEB_REMIX served it, record the failure
    /// so the next resolve for this id bypasses WEB_REMIX (stream selection §2). Then retry the SAME
    /// track once — the re-resolve now runs the direct-URL fallback clients, which is what makes
    /// PoToken-enforced/niche videos playable — before giving up and advancing the queue.
    /// Returns true when the track was retried (so the caller can skip the error toast).
    pub async fn on_track_failed(self: &std::sync::Arc<Self>) -> bool {
        let (video_id, client, already_retried) = {
            let q = self.queue.lock().await;
            let vid = q.items.get(q.current).map(|i| i.video_id.clone());
            let retried = vid.is_some() && vid == q.retried;
            (vid, q.current_client.clone(), retried)
        };
        if let (Some(vid), Some(c)) = (video_id, client) {
            // Whatever served it, the URL is dead — evict so the retry can't replay it from cache.
            self.db.evict_stream(&vid);
            if c == MAIN_CLIENT {
                tracing::warn!(video_id = %vid, "WEB_REMIX stream failed on GET — marking + evicting");
                self.orchestrator.mark_web_remix_failed(&vid).await;
            }
            // Retry once for WEB_REMIX-served and cache-served URLs. A failure from a fallback
            // client, or a second failure of the same id, advances as before.
            if (c == MAIN_CLIENT || c == "cache") && !already_retried {
                {
                    let mut q = self.queue.lock().await;
                    q.retried = Some(vid.clone());
                    q.lookahead_loaded = None; // start_current's loadfile replaces mpv's playlist
                }
                tracing::info!(video_id = %vid, "retrying failed track via fallback clients");
                let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
                if self.start_current(gen).await {
                    self.prime_lookahead(gen).await;
                }
                return true;
            }
        }
        self.on_track_ended().await;
        false
    }

    /// Resolve + load the current track into mpv (replace). Returns false if resolve failed or the
    /// request was superseded.
    async fn start_current(self: &std::sync::Arc<Self>, gen: u64) -> bool {
        // Resolve the current track, auto-skipping any that no client can play (dead / region-locked
        // videos — stream selection "no client could resolve") instead of stalling the queue on them.
        // Bounded: each failure advances current by one, so the loop terminates at the queue tail.
        let (mut item, data) = loop {
            if self.generation.load(Ordering::SeqCst) != gen {
                return false; // user moved on
            }
            let Some(item) = self.current_item().await else { return false };
            let resolved = self.resolve(&item.video_id, item.is_upload).await;
            // A resolve takes seconds; a skip during it bumps the generation. Re-check before
            // acting on the result: an abandoned failure would otherwise move `current` under the
            // track that's already playing and leave a stale error banner (nothing clears it, the
            // new track's now-playing fired before the banner appeared).
            if self.generation.load(Ordering::SeqCst) != gen {
                return false;
            }
            match resolved {
                Ok(d) => break (item, d),
                // A deleted local file is gone for good, so it leaves the queue outright rather
                // than being skipped over — a row that can only ever fail again is noise. Every
                // other failure is transient in principle (network, region, a cipher self-heal),
                // so those keep their place.
                Err(ResolveError::LocalMissing(_)) => {
                    let exhausted = {
                        let mut q = self.queue.lock().await;
                        let cur = q.current;
                        if cur < q.items.len() {
                            q.items.remove(cur);
                        }
                        q.lookahead_loaded = None;
                        // `current` now points at what followed it. When it was the tail, step
                        // back onto the last surviving track: an index past the end leaves the
                        // transport pointing at nothing, and Play does nothing at all.
                        let exhausted = q.current >= q.items.len();
                        if exhausted {
                            let last = q.items.len().saturating_sub(1);
                            q.seek_to(last);
                        }
                        exhausted
                    };
                    self.emit_queue().await;
                    self.persist_queue().await;
                    self.lt_broadcast_queue().await;
                    self.emit_notice(&format!("{} is no longer on your disk", item.title));
                    if exhausted {
                        return false;
                    }
                }
                Err(e) => {
                    // A provider stream that needs the user signed in is not "unavailable":
                    // skipping past it row-by-row turns a whole Spotify queue into a storm of
                    // "Skipped (unavailable)" toasts and hides the one action that fixes it.
                    // Stop the queue on the spot and say what is actually wrong.
                    if let ResolveError::SignInRequired(message) = &e {
                        tracing::warn!(video_id = item.video_id, message, "playback needs sign-in");
                        self.emit_error(&item.video_id, message);
                        return false;
                    }
                    let mut q = self.queue.lock().await;
                    // Deliberately ignores repeat-all: wrapping the unplayable-skip would spin
                    // forever on a queue where nothing resolves. Skips stop at the tail.
                    if q.current + 1 >= q.items.len() {
                        drop(q);
                        self.emit_error(&item.video_id, &e.to_string()); // nothing left to skip to
                        return false;
                    }
                    q.current += 1;
                    q.lookahead_loaded = None;
                    drop(q);
                    match &e {
                        ResolveError::NoMatch(message) => {
                            self.emit_notice(&format!("Skipped: {message}"))
                        }
                        _ => self.emit_skip(&item.title),
                    }
                    self.emit_queue().await;
                }
            }
        };
        if self.generation.load(Ordering::SeqCst) != gen {
            return false; // user moved on
        }
        // Resume a restored position, but only for the exact track it was saved against (any first
        // play consumes it, so jumping elsewhere doesn't inherit it). It rides on loadfile as the
        // per-file `start` option: an absolute seek issued right after loadfile reaches mpv before
        // the file is loaded and is rejected, so the track started from zero.
        let resume_at = self
            .pending_seek
            .lock()
            .unwrap()
            .take()
            .filter(|(vid, _)| *vid == item.video_id)
            .map(|(_, pos)| pos)
            .filter(|_| {
                !matches!(
                    MediaRef::parse(&item.video_id),
                    MediaRef::Radio(_)
                        | MediaRef::SpotifyTrack(_)
                        | MediaRef::SpotifyOther(_)
                        | MediaRef::SpotifyArtist(_)
                        | MediaRef::SpotifyAlbum(_)
                        | MediaRef::SpotifyPlaylist(_)
                )
            });
        let mpv_options = with_resume_position(&data.mpv_options, resume_at);
        if let Err(e) = self.player.load(
            &data.stream_url,
            &data.headers,
            loudness_gain(data.loudness_db),
            &media_title(&item.title, &item.artists),
            &mpv_options,
        ) {
            self.emit_error(&item.video_id, &e.to_string());
            return false;
        }
        let _ = self.player.play();
        // Items played from cards/radio can arrive without a duration; the player response knows
        // the exact length of the cut we stream. Backfill before emitting — lyrics matching keys
        // on it (a wrong-cut LRCLIB match plays lyrics seconds off the audio).
        backfill_metadata(
            &mut item,
            data.duration.as_deref(),
            data.artists.as_deref(),
            data.is_video,
        );
        {
            let mut q = self.queue.lock().await;
            q.current_client = Some(data.stream_client.clone());
            // Fresh play → fresh history state (write API §registerPlayback).
            q.playback_ping = data.playback_ping.clone();
            q.cpn = innertube::generate_cpn();
            q.history_pinged = false;
            q.duration = 0.0;
            let cur = q.current;
            if let Some(qi) = q.items.get_mut(cur) {
                // Same repair on the queue's own copy: it's what the queue panel renders, what
                // `persist_queue` saves, and what `record_play` writes into On Repeat.
                if qi.video_id == item.video_id {
                    backfill_metadata(
                        qi,
                        data.duration.as_deref(),
                        data.artists.as_deref(),
                        data.is_video,
                    );
                }
            }
        }
        if crate::spotify::streams_via_fifo(&item.video_id) {
            // mpv can't measure a FIFO's length, so seed the duration from the queue item and push
            // it exactly like `on_duration` would (mpv's own `duration` events are ignored while a
            // Spotify track plays).
            self.seed_spotify_duration(&item).await;
        } else {
            // Any non-Spotify track means the previous Spotify stream (if any) is finished: drop
            // its handle and stop its watcher. `resolve` already replaced it for a Spotify track.
            self.drop_spotify_stream();
        }
        self.emit_now_playing(&item, &data.stream_client);
        // Search/autoplay/restored rows often carry no live likeStatus. Refresh the playing track
        // asynchronously without delaying audio start.
        self.refresh_rating(&item.video_id, gen);
        // We just told mpv to play, but its `pause` flag was already `false`, so no property event
        // will announce it (see `Player::is_playing`). Say so ourselves — otherwise MPRIS and
        // Discord never learn the track started. After `emit_now_playing`, so the new track is the
        // current one before anything renders it as playing.
        self.media_set_playing(true);
        self.emit_queue().await;
        self.persist_queue().await;
        // Listen Together host: announce the new track (fresh play → position 0, playing).
        self.lt_broadcast_current_track(0, true).await;
        // Autoplay early trigger: extend the queue while the tail still plays, so the gapless
        // lookahead can prime into the continuation. The near-tail guard inside makes this a
        // no-op for almost every track start. Detached — never on the caller's path.
        let me = self.clone();
        tokio::spawn(async move { me.extend_queue_radio(gen).await });
        true
    }

    /// Resolve the next queue item and append it to mpv for a gapless transition.
    ///
    /// An upcoming track no client can resolve is skipped *here*, while the current track is
    /// still playing — deferring it to the track boundary pauses playback while the whole client
    /// waterfall fails again on the event pump (and used to wedge until a manual skip).
    /// Note: at most 3 removals per prime so a network outage can't eat the whole queue.
    async fn prime_lookahead(self: &std::sync::Arc<Self>, gen: u64) {
        for _ in 0..3 {
            let next_idx = {
                let q = self.queue.lock().await;
                // Repeat-one primes nothing: mpv loops the file itself (next_index → None).
                let Some(next) = next_index(q.items.len(), q.current, q.repeat) else { return };
                if q.lookahead_loaded == Some(next) {
                    return; // already primed
                }
                // A Spotify track streams through one per-client FIFO: resolving a Spotify *next*
                // would start a second librespot decode into it (corrupting the current track), and
                // a FIFO can't be gaplessly appended to a URL either. When either side of the
                // boundary is Spotify, don't prime — `on_track_ended` loads across it explicitly.
                let boundary_spotify = [q.current, next].iter().any(|&i| {
                    q.items.get(i).is_some_and(|it| crate::spotify::streams_via_fifo(&it.video_id))
                });
                if boundary_spotify {
                    return;
                }
                next
            };
            let (next_video, next_title, next_upload) = {
                let q = self.queue.lock().await;
                match q.items.get(next_idx) {
                    Some(item) => (item.video_id.clone(), item.title.clone(), item.is_upload),
                    None => return,
                }
            };
            match self.resolve(&next_video, next_upload).await {
                Ok(d) => {
                    self.enqueue_lookahead(gen, next_idx, &next_video, d).await;
                    return;
                }
                Err(e) => {
                    tracing::warn!(video_id = %next_video, error = %e, "lookahead resolve failed — dropping from queue");
                    if self.generation.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    // The queue can shift under a resolve — only remove the slot if it still
                    // holds the track that failed. (Also refuses next_idx == current, i.e. the
                    // single-item repeat-all case, via remove_from_queue's own guard.)
                    let same = {
                        let q = self.queue.lock().await;
                        q.items.get(next_idx).map(|i| i.video_id == next_video).unwrap_or(false)
                    };
                    if !same {
                        return;
                    }
                    // Box::pin: remove_from_queue can itself re-prime (async recursion).
                    Box::pin(self.remove_from_queue(next_idx)).await;
                    self.emit_skip(&next_title);
                }
            }
        }
    }

    /// Second half of [`Self::prime_lookahead`]: hand the resolved stream to mpv and record it.
    async fn enqueue_lookahead(
        self: &std::sync::Arc<Self>,
        gen: u64,
        next_idx: usize,
        next_video: &str,
        data: PlaybackData,
    ) {
        if self.generation.load(Ordering::SeqCst) != gen {
            return;
        }
        let mut q = self.queue.lock().await;
        // The queue can change under a resolve (a guest add inserts at current+1) — enqueueing
        // then would gaplessly play the wrong song. Verify the slot still holds the same track.
        if q.items.get(next_idx).map(|i| i.video_id != next_video).unwrap_or(true) {
            tracing::debug!(index = next_idx, "queue changed during lookahead resolve — dropped");
            return;
        }
        // Another prime already claimed this slot while we resolved. Two run concurrently at the
        // autoplay trigger point — `start_current` spawns `extend_queue_radio` (which primes after
        // extending) and its caller primes right after, both on the same generation, so the
        // pre-resolve "already primed" check in `prime_lookahead` can't see the other one. Appending
        // again leaves a *duplicate* entry in mpv's playlist, and since the queue advances one index
        // per end-file, that offsets mpv from `current` for the rest of the session: the dup replays
        // while the UI shows the track after it.
        if q.lookahead_loaded == Some(next_idx) {
            tracing::debug!(
                index = next_idx,
                "lookahead already primed by a concurrent resolve — dropped"
            );
            return;
        }
        // Headers are global in mpv; the direct-URL clients need none beyond UA, which the
        // current track already set. Just append the URL.
        let title =
            q.items.get(next_idx).map(|i| media_title(&i.title, &i.artists)).unwrap_or_default();
        if let Err(e) = self.player.enqueue(&data.stream_url, &title) {
            tracing::warn!(error = %e, "enqueue lookahead failed");
            return;
        }
        q.lookahead_loaded = Some(next_idx);
        q.lookahead_client = Some(data.stream_client.clone());
        q.lookahead_gain = Some(loudness_gain(data.loudness_db));
        q.lookahead_playback_ping = data.playback_ping.clone();
        // Same backfill as start_current: a gapless advance emits this item straight from the
        // queue, so the repair has to land before it becomes the current track.
        if let Some(qi) = q.items.get_mut(next_idx) {
            backfill_metadata(qi, data.duration.as_deref(), data.artists.as_deref(), data.is_video);
        }
        tracing::debug!(index = next_idx, "gapless lookahead primed");
    }

    /// Push play/pause state + the current position to the OS media controls (fail-soft policy) and
    /// Discord. The single choke point for play/pause, so both stay in step with mpv. Discord gets
    /// the flag only — its position flows exclusively through the ticks, so a stale
    /// `current_position()` here (the last tick can predate a track change) can't poison its
    /// timeline.
    pub fn media_set_playing(&self, playing: bool) {
        self.is_playing.store(playing, Ordering::Relaxed);
        if let Some(m) = &self.media {
            m.set_playback(playing, self.current_position());
        }
        if let Some(d) = &self.discord {
            d.set_playing(playing);
        }
    }

    /// Toggle Discord presence at runtime (the `discord_rpc` setting). Turning it off clears the
    /// presence and closes the socket; turning it on re-pushes the current track.
    pub fn set_discord_enabled(&self, on: bool) {
        if let Some(d) = &self.discord {
            d.set_enabled(on);
        }
    }

    pub fn set_discord_name(&self, name: String) {
        if let Some(d) = &self.discord {
            d.set_name(name);
        }
    }

    pub fn discord_status(&self) -> serde_json::Value {
        let enabled = self.db.get_setting("discord_rpc").as_deref() == Some("true");
        let status = if !enabled {
            "disabled"
        } else {
            self.discord.as_ref().map(DiscordHandle::status).unwrap_or("unavailable")
        };
        serde_json::json!({ "enabled": enabled, "status": status })
    }

    pub fn set_low_resource_mode(&self, on: bool) {
        self.low_resource_mode.store(on, Ordering::Release);
    }

    pub fn low_resource_mode(&self) -> bool {
        self.low_resource_mode.load(Ordering::Acquire)
    }

    /// Explicit Quit teardown. Close-to-tray deliberately does NOT use this: a paused loaded track
    /// must remain resumable from MPRIS/QS while the window is hibernated.
    pub async fn shutdown_for_quit(&self) {
        self.flush_position();
        let _ = self.player.stop();
        // Tear down any live Spotify stream + its watcher; nothing resumes librespot after Quit.
        self.drop_spotify_stream();
        self.is_playing.store(false, Ordering::Release);

        // The user-facing media island is the first thing Quit must remove. MediaHandle::shutdown
        // returns only after the platform controls object has been dropped, so DBus/SMTC cannot
        // outlive an explicit quit while slower best-effort integrations finish their cleanup.
        if let Some(m) = &self.media {
            m.shutdown();
        }
        if let Some(d) = &self.discord {
            d.shutdown();
        }
        // Leaving a Listen Together room is courtesy, not permission to exit. A wedged session
        // mutex/socket must never turn Quit into a multi-second hang.
        let _ = tokio::time::timeout(std::time::Duration::from_millis(500), self.lt.leave()).await;
    }

    /// Latest mpv position (secs) — for OS scrubber updates + relative media-key seeks.
    pub fn current_position(&self) -> f64 {
        f64::from_bits(self.latest_position.load(Ordering::Relaxed))
    }

    /// Cheap authoritative play-state mirror for background lifecycle decisions.
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    /// Whether mpv still owns a loaded track that can be resumed without rebuilding playback.
    /// A paused track is therefore a live background session even though `is_playing()` is false.
    pub fn has_resumable_session(&self) -> bool {
        self.player.has_loaded_media()
    }

    fn emit_error(&self, video_id: &str, message: &str) {
        tracing::error!(video_id, message, "playback error");
        self.emit("playback-error", serde_json::json!({ "videoId": video_id, "message": message }));
    }

    /// Guest tried a host-only playback action — explain instead of silently ignoring.
    fn emit_guest_hint(&self) {
        self.emit(
            "playback-notice",
            serde_json::json!({ "message": "The host controls playback — click a song to add it to the session queue" }),
        );
    }

    /// A transient toast. Same channel as [`Self::emit_skip`], for messages that phrase themselves.
    fn emit_notice(&self, message: &str) {
        tracing::info!(message, "playback notice");
        self.emit("playback-notice", serde_json::json!({ "message": message }));
    }

    /// A track was auto-skipped because no client could resolve it — a transient toast, not the
    /// persistent error banner: the queue keeps playing, so this shouldn't read as a failure.
    fn emit_skip(&self, title: &str) {
        tracing::warn!(title, "skipping unplayable track");
        self.emit(
            "playback-notice",
            serde_json::json!({ "message": format!("Skipped (unavailable): {title}") }),
        );
    }

    /// Autoplay enabled? Default on; only an explicit `"false"` disables it (mirrors
    /// `history_enabled`).
    fn autoplay_enabled(&self) -> bool {
        self.db.get_setting("autoplay").map(|v| v != "false").unwrap_or(true)
    }

    /// Extend the queue with radio continuation when it's nearly out (autoplay). Returns how many
    /// tracks were appended. Guards: setting on, repeat Off, not a guest, tail near (last two
    /// tracks), generation unchanged across the network call. Continuation matches where the queue
    /// came from: playlist/album radio (`radio_seed`) or song radio seeded from the last track.
    /// Dedupes against the entire current queue; caps at `AUTOPLAY_BATCH` per hop. When the radio
    /// returns nothing new, playback later stops exactly as pre-autoplay (no retry loop).
    async fn extend_queue_radio(self: &std::sync::Arc<Self>, gen: u64) -> usize {
        const AUTOPLAY_BATCH: usize = 20;
        if !self.autoplay_enabled() || self.lt.is_guest().await {
            return 0;
        }
        enum AutoplaySource {
            Radio { last_video: String, seed: String },
            Soundcloud { id: u64 },
        }
        let source = {
            let q = self.queue.lock().await;
            if q.repeat != RepeatMode::Off {
                return 0; // the queue never exhausts under repeat
            }
            if !autoplay_tail_near(q.items.len(), q.current) {
                return 0;
            }
            let Some(last) = q.items.last() else { return 0 };
            // A SoundCloud tail continues via the track's own related feed, not YouTube radio.
            if let MediaRef::SoundCloudTrack(id) = MediaRef::parse(&last.video_id) {
                AutoplaySource::Soundcloud { id }
            } else {
                // Nothing to continue from when the queue ends on a local file, a live-radio
                // stream, or a Spotify track: none is a YouTube videoId a radio can seed from.
                if q.radio_seed.is_none()
                    && matches!(
                        MediaRef::parse(&last.video_id),
                        MediaRef::Local(_)
                            | MediaRef::Radio(_)
                            | MediaRef::SpotifyTrack(_)
                            | MediaRef::SpotifyOther(_)
                            | MediaRef::SpotifyArtist(_)
                            | MediaRef::SpotifyAlbum(_)
                            | MediaRef::SpotifyPlaylist(_)
                    )
                {
                    return 0;
                }
                let seed =
                    q.radio_seed.clone().unwrap_or_else(|| format!("RDAMVM{}", last.video_id));
                AutoplaySource::Radio { last_video: last.video_id.clone(), seed }
            }
        };
        // Snapshot → network → re-lock, same discipline as `prime_lookahead`; the generation
        // check below is what makes it safe. The queue can grow during the fetch: playing a single
        // song triggers this while `play_song` is still hydrating the same song's radio. So the
        // tail check and the dedupe run again against the queue as it is after the fetch.
        let fresh: Vec<SongItem> = match &source {
            AutoplaySource::Radio { last_video, seed } => {
                let Some(client) = self.clients.get(innertube::METADATA_CLIENT) else { return 0 };
                match self.it.next(client, Some(last_video), Some(seed)).await {
                    Ok(next) => next.items,
                    Err(e) => {
                        tracing::warn!(error = %e, "autoplay radio fetch failed");
                        return 0;
                    }
                }
            }
            AutoplaySource::Soundcloud { id } => match self.soundcloud.related(*id).await {
                Ok(tracks) => tracks.iter().map(crate::soundcloud_bridge::track_to_song).collect(),
                Err(e) => {
                    tracing::warn!(error = %e, "autoplay soundcloud related fetch failed");
                    return 0;
                }
            },
        };
        if self.generation.load(Ordering::SeqCst) != gen {
            return 0; // user moved on while we fetched
        }
        let added = {
            let mut q = self.queue.lock().await;
            if autoplay_tail_near(q.items.len(), q.current) {
                let existing: HashSet<String> =
                    q.items.iter().map(|i| i.video_id.clone()).collect();
                merge_radio(&mut q.items, fresh, existing, AUTOPLAY_BATCH)
            } else {
                0
            }
        };
        if added > 0 {
            tracing::info!(added, "autoplay extended the queue");
            self.emit_queue().await;
            self.persist_queue().await;
            self.lt_broadcast_queue().await;
            // Appending at the tail never touches a primed lookahead slot; this covers the
            // "current was last, nothing was primed" case.
            self.prime_lookahead(gen).await;
        }
        added
    }

    /// Force-clear everything playback remembers about how to get a stream: the SQLite URL +
    /// lyrics cache, mpv's on-disk audio bytes, the stored PoToken, and the per-video WEB_REMIX
    /// blacklist. A stale cached stream URL or a stale/missing `visitorData` both surface as mpv
    /// 403s ("YouTube rejected the stream link") that no retry can fix on their own — this is
    /// the manual lever for it.
    ///
    /// `rotate_identity` additionally drops the anonymous YouTube playback identity and
    /// re-bootstraps a fresh one (the settings button; quality changes pass false). Returns
    /// whether a fresh visitorData was installed — false when not rotating or when the fetch
    /// failed (offline). A failed fetch still leaves the token deleted, so the next launch or
    /// bot-gated resolve re-bootstraps it anyway.
    pub async fn clear_caches(&self, rotate_identity: bool) -> bool {
        self.db.clear_stream_cache();
        // The stored PoToken is a cache too, and "clear caches" is where someone goes when
        // playback has started behaving oddly. Dropping it costs one BotGuard bootstrap.
        self.db.delete_setting("potoken_session");
        if let Ok(entries) = std::fs::read_dir(&self.paths.cache_dir) {
            for e in entries.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
        self.orchestrator.clear_web_remix_failures().await;
        // The blacklist is an in-memory hint derived from cached URLs; with the cache wiped,
        // WEB_REMIX gets another chance.
        if !rotate_identity {
            return false;
        }
        // Drop the persisted anonymous identity before fetching, so a fetch failure still leaves
        // the daemon in the "re-bootstrap" state instead of holding the stale token it just
        // complained about. Clearing the cooldown lets the on-demand bot-gate heal fire again.
        self.db.delete_setting("visitor_data");
        self.it.set_visitor_data(None);
        self.last_visitor_try.store(0, Ordering::Relaxed);
        match self.it.fetch_visitor_data().await {
            Ok(vd) => {
                self.it.set_visitor_data(Some(vd.clone()));
                self.db.set_setting("visitor_data", &vd);
                tracing::info!("playback caches force-cleared; visitorData re-bootstrapped");
                true
            }
            Err(e) => {
                tracing::warn!(error = %e, "visitorData refresh during cache clear failed");
                false
            }
        }
    }

    // --- Listen Together (session protocol) --------------------------------------------------------
}

pub fn song_to_track(s: &SongItem) -> Track {
    Track {
        id: s.video_id.clone(),
        title: s.title.clone(),
        artist: s.artists.clone(),
        thumbnail: s.thumbnail.clone(),
        duration_ms: parse_duration_ms(s.duration.as_deref()),
        queued_by: s.queued_by.clone(),
    }
}

fn track_to_song(t: &Track) -> SongItem {
    SongItem {
        video_id: t.id.clone(),
        title: t.title.clone(),
        artists: t.artist.clone(),
        artist_id: None,
        artist_runs: Vec::new(),
        album: None,
        album_id: None,
        duration: if t.duration_ms > 0 { Some(format_duration(t.duration_ms)) } else { None },
        play_count: None,
        thumbnail: t.thumbnail.clone(),
        set_video_id: None,
        added_by: None,
        added_by_avatar: None,
        rating: None,
        queued_by: t.queued_by.clone(),
        queued: false,
        queued_end: false,
        queued_from: None,
        autoplay: false,
        is_video: false,
        is_upload: false,
        // The Listen Together wire shape carries no badge, so a mirrored guest queue shows none.
        explicit: false,
    }
}

/// A song's credited artists: the linked runs when YouTube sent them, else the artist line split on
/// its separators. Band names are never split on other punctuation.
fn song_artists(song: &SongItem) -> Vec<String> {
    let linked: Vec<String> = song
        .artist_runs
        .iter()
        .filter(|run| run.id.is_some())
        .map(|run| run.text.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    if !linked.is_empty() {
        return linked;
    }
    song.artists
        .replace(" & ", ", ")
        .split(", ")
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Parse a `"m:ss"` / `"h:mm:ss"` duration string to ms (0 if absent/unparseable).
fn parse_duration_ms(s: Option<&str>) -> i64 {
    let Some(s) = s else { return 0 };
    let parts: Vec<i64> = s.split(':').filter_map(|p| p.trim().parse().ok()).collect();
    let secs = match parts.as_slice() {
        [s] => *s,
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => 0,
    };
    secs * 1000
}

fn format_duration(ms: i64) -> String {
    let total = ms / 1000;
    match total / 3600 {
        0 => format!("{}:{:02}", total / 60, total % 60),
        h => format!("{}:{:02}:{:02}", h, (total % 3600) / 60, total % 60),
    }
}

/// Fill a missing item duration from the player response's `lengthSeconds` (e.g. "167") — the
/// exact length of the cut we stream. Never overwrites an existing duration.
/// Repair a queue item from the `/player` response, which is the one thing every entry path (search,
/// album, card, radio, a restored queue, a row replayed out of On Repeat) goes through.
///
/// `length_seconds` fills a duration items from cards/radio arrive without. `author` is
/// `videoDetails.author` from the MAIN client, i.e. YouTube's own artist for the track: it repairs
/// an artist string that is missing (album rows ship the artist column empty) or that is a whole
/// display subtitle rather than a name ("Miley Cyrus • Plastic Hearts • 2020"). A "•" never appears
/// in a real artist line, collabs use "&" and ",". Both shapes reach the player bar, the OS widget
/// and Last.fm, and a wrong artist there is worse than a missing one: it scrobbles as another
/// artist entirely. Rows persisted before this existed are healed the next time they play, because
/// the caller writes the repaired item back into the queue.
fn backfill_metadata(
    item: &mut SongItem,
    length_seconds: Option<&str>,
    author: Option<&str>,
    is_video: Option<bool>,
) {
    if item.duration.is_none() {
        if let Some(secs) = length_seconds.and_then(|s| s.trim().parse::<i64>().ok()) {
            item.duration = Some(format_duration(secs * 1000));
        }
    }
    // YouTube's own media-type verdict outranks whatever the row was parsed with: a card played
    // from a shelf never carried the flag, an album row keeps it after being swapped to its audio
    // id, and a mirrored or restored row has no flag at all. Overwrite, don't OR: a stale `true`
    // on an audio track would put the still album-art "video" on screen (video playback).
    if let Some(is_video) = is_video {
        item.is_video = is_video;
    }
    if let Some(author) = author.map(str::trim).filter(|a| !a.is_empty()) {
        if item.artists.trim().is_empty() || item.artists.contains('•') {
            item.artists = author.to_owned();
            // The per-artist links belong to the string we just replaced; the UI renders them in
            // place of `artists` when they're there, so a stale set would put the bad line back.
            item.artist_runs.clear();
        }
    }
}

/// The level to come up at: what the user left the slider on last run. Written by the UI on
/// commit rather than by `set_volume`, which a drag calls every frame (and every settings write
/// is an fsync). mpv would start at 100 otherwise.
pub fn saved_volume(db: &Db) -> i64 {
    let v = db.get_setting("volume").and_then(|s| s.parse().ok());
    v.filter(|v| (0..=100).contains(v)).unwrap_or(100)
}

/// Read the personal blob out of the DB, degrading anything that is missing, unparseable, or not a
/// JSON object to `{}`. The clients hydrate tolerantly on top of this, so a corrupt store costs the
/// user their shortcuts, never a crash.
fn read_personal(db: &Db) -> serde_json::Value {
    db.get_setting(PERSONAL_JSON_KEY)
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Validate a personal blob and return its serialized form. Rejects a non-object (the store is one
/// keyed record, never a bare array/scalar) and anything over [`MAX_PERSONAL_BYTES`].
fn validate_personal_blob(blob: &serde_json::Value) -> Result<String, String> {
    if !blob.is_object() {
        return Err("personal store must be a JSON object".into());
    }
    let json = serde_json::to_string(blob).map_err(|e| e.to_string())?;
    if json.len() > MAX_PERSONAL_BYTES {
        return Err(format!(
            "personal store too large: {} bytes (max {MAX_PERSONAL_BYTES})",
            json.len()
        ));
    }
    Ok(json)
}

/// Per-track loudness gain (dB) from YouTube's `loudnessDb`. Attenuate
/// only toward reference loudness: loud masters get pulled down, quieter tracks aren't boosted,
/// so there's no clipping and no limiter to add.
///
/// `loudnessDb` is measured against **-14 LUFS** (YouTube's own response proves it:
/// `perceptualLoudnessDb == loudnessDb - 14`, i.e. the track's absolute LUFS). Applying it raw
/// normalizes to -14 like the *video* site, but YouTube **Music** only pulls down what exceeds
/// **-7 LUFS**, so raw made us ~6 dB quieter than YTM web on a typical modern master and left
/// most tracks attenuated that YTM never touches at all. Normalize to the same -7 target.
///
/// `None` means "no filter", and that's most tracks now: below the target there is nothing to
/// attenuate, so mpv gets its `af` cleared rather than a `volume=0dB` no-op in the chain.
// Note: attenuate-only, clamped to -24 dB. If quiet tracks feel too soft, allow positive gain
// plus an `alimiter` af to catch the resulting peaks.
fn loudness_gain(loudness_db: Option<f64>) -> Option<f64> {
    let gain = TARGET_LUFS - (loudness_db? - 14.0);
    (gain < -0.05).then(|| gain.max(-24.0))
}

/// "Artist – Title" for mpv's per-file media title (what PipeWire and mpv's own metadata show).
fn media_title(title: &str, artists: &str) -> String {
    match (title.trim(), artists.trim()) {
        ("", "") => "Ryotunes Omni".to_owned(),
        (t, "") => t.to_owned(),
        ("", a) => a.to_owned(),
        (t, a) => format!("{a} – {t}"),
    }
}

/// The per-file mpv options for a Spotify track. librespot writes raw S16LE / 44100 Hz / stereo
/// PCM into a FIFO that mpv can't probe, so the rawaudio demuxer must be told the exact format.
/// Must stay byte-for-byte in step with the format librespot's `pipe` backend emits.
fn spotify_mpv_options() -> Vec<String> {
    vec![
        "demuxer=rawaudio".to_owned(),
        "demuxer-rawaudio-format=s16le".to_owned(),
        "demuxer-rawaudio-rate=44100".to_owned(),
        "demuxer-rawaudio-channels=2".to_owned(),
    ]
}

/// The per-file options for loading a track, plus mpv's `start` option when playback should resume
/// partway in. A position that is not a positive finite number adds nothing.
fn with_resume_position(options: &[String], resume_at: Option<f64>) -> Vec<String> {
    let mut options = options.to_vec();
    if let Some(pos) = resume_at.filter(|pos| pos.is_finite() && *pos > 0.0) {
        options.push(format!("start={pos:.3}"));
    }
    options
}

/// Loudness target, matching YouTube Music's own player rather than the video site's -14.
const TARGET_LUFS: f64 = -7.0;

/// Thin metadata/browse/player API the host commands forward into. Each body is the logic that
/// used to live inline in a `#[tauri::command]`, moved here so the host stays a pure adapter.
impl AppState {
    fn metadata_client(&self) -> Result<&innertube::YouTubeClient, String> {
        self.clients.get(innertube::METADATA_CLIENT).ok_or_else(|| "metadata client missing".into())
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SongItem>, String> {
        let client = self.metadata_client()?;
        let result = self.it.search_songs(client, query).await.map_err(|e| e.to_string())?;
        Ok(result.items)
    }

    pub async fn search_page(&self, query: &str) -> Result<innertube::SearchResult, String> {
        let client = self.metadata_client()?;
        self.it.search_songs(client, query).await.map_err(|e| e.to_string())
    }

    pub async fn search_page_more(&self, token: &str) -> Result<innertube::SearchResult, String> {
        let client = self.metadata_client()?;
        self.it.search_songs_continuation(client, token).await.map_err(|e| e.to_string())
    }

    pub async fn search_all(&self, query: &str) -> Result<innertube::SearchResults, String> {
        let client = self.metadata_client()?;
        self.it.search_all(client, query).await.map_err(|e| e.to_string())
    }

    pub async fn search_cards(
        &self,
        query: &str,
        category: &str,
    ) -> Result<Vec<innertube::BrowseItem>, String> {
        let client = self.metadata_client()?;
        self.it.search_cards(client, query, category).await.map_err(|e| e.to_string())
    }

    pub async fn search_cards_page(
        &self,
        query: &str,
        category: &str,
    ) -> Result<innertube::SearchCardPage, String> {
        let client = self.metadata_client()?;
        self.it.search_cards_page(client, query, category).await.map_err(|e| e.to_string())
    }

    pub async fn search_cards_more(
        &self,
        token: &str,
    ) -> Result<innertube::SearchCardPage, String> {
        let client = self.metadata_client()?;
        self.it.search_cards_continuation(client, token).await.map_err(|e| e.to_string())
    }

    pub async fn search_all_more(&self, token: &str) -> Result<innertube::SearchResults, String> {
        let client = self.metadata_client()?;
        self.it.search_all_continuation(client, token).await.map_err(|e| e.to_string())
    }

    pub async fn home(&self, params: Option<&str>) -> Result<innertube::HomePage, String> {
        let client = self.metadata_client()?;
        self.it.home(client, params).await.map_err(|error| {
            tracing::warn!(error = %error, "Home feed request failed");
            error.to_string()
        })
    }

    pub async fn home_more(&self, token: &str) -> Result<innertube::HomePage, String> {
        let client = self.metadata_client()?;
        self.it.home_continuation(client, token).await.map_err(|error| {
            tracing::warn!(error = %error, "Home continuation request failed");
            error.to_string()
        })
    }

    /// Signed out there is no YouTube library grid, and the Library page merges local saves in, so
    /// "nothing of yours on YouTube" is an empty answer rather than an error.
    pub async fn library_albums(&self) -> Result<Vec<innertube::BrowseItem>, String> {
        if !self.it.is_logged_in() {
            return Ok(Vec::new());
        }
        let client = self.metadata_client()?;
        self.it.library_albums(client).await.map_err(|e| e.to_string())
    }

    pub async fn library_artists(&self) -> Result<Vec<innertube::BrowseItem>, String> {
        if !self.it.is_logged_in() {
            return Ok(Vec::new());
        }
        let client = self.metadata_client()?;
        self.it.library_artists(client).await.map_err(|e| e.to_string())
    }

    pub async fn playlist_more(
        &self,
        token: &str,
    ) -> Result<innertube::PlaylistContinuation, String> {
        let client = self.metadata_client()?;
        self.it.playlist_continuation(client, token).await.map_err(|e| e.to_string())
    }

    /// A YouTube album page, or a local album/artist page built from SQLite so it opens offline.
    pub async fn album(&self, id: &str) -> Result<innertube::AlbumPage, String> {
        if let Some(key) = id.strip_prefix(crate::local::ALBUM_PREFIX) {
            return Ok(crate::local::album_page(&self.db, key));
        }
        if let Some(name) = id.strip_prefix(crate::local::ARTIST_PREFIX) {
            return Ok(crate::local::artist_page(&self.db, name));
        }
        let client = self.metadata_client()?;
        self.it.album(client, id).await.map_err(|e| e.to_string())
    }

    pub async fn artist(&self, id: &str) -> Result<innertube::ArtistPage, String> {
        let client = self.metadata_client()?;
        self.it.artist(client, id).await.map_err(|e| e.to_string())
    }

    pub async fn browse_grid(
        &self,
        id: &str,
        params: Option<&str>,
    ) -> Result<Vec<innertube::BrowseItem>, String> {
        let client = self.metadata_client()?;
        self.it.browse_grid(client, id, params).await.map_err(|e| e.to_string())
    }

    pub fn set_volume(&self, volume: i64) -> Result<(), String> {
        if !(0..=100).contains(&volume) {
            return Err("Volume must be between 0 and 100.".into());
        }
        self.player.set_volume(volume).map_err(|e| e.to_string())?;
        // There is one volume and there can be two windows (the mini player). Without this the one
        // that didn't move the slider keeps showing the old level and lies about what you're hearing.
        self.emit("volume", volume);
        Ok(())
    }

    pub fn set_playback_params(&self, speed: f64, semitones: i32) -> Result<(), String> {
        // The tempo/pitch steppers are the two oldest fields of the fx set; a change to either
        // keeps whatever reverb/bass/width the Sound dialog left in place.
        self.set_audio_fx(AudioFx { speed, semitones, ..self.audio_fx() })
    }

    /// The current audio-fx values, for a snapshot or the Sound dialog opening cold.
    pub fn audio_fx(&self) -> AudioFx {
        *self.audio_fx.read()
    }

    /// Apply the Sound dialog's effects. Ranges are validated here (the daemon is the one place
    /// that must not trust a client), applied to the player as one `af` chain, then — only after a
    /// successful apply — stored and echoed to every client as `audio-fx` so all windows agree.
    pub fn set_audio_fx(&self, fx: AudioFx) -> Result<(), String> {
        if !fx.speed.is_finite() || !(0.25..=2.0).contains(&fx.speed) {
            return Err("Tempo must be between 0.25× and 2.00×.".into());
        }
        if !(-12..=12).contains(&fx.semitones) {
            return Err("Pitch must be between -12 and +12 semitones.".into());
        }
        if !fx.reverb.is_finite() || !(0.0..=1.0).contains(&fx.reverb) {
            return Err("Reverb must be between 0 and 1.".into());
        }
        if !fx.bass_db.is_finite() || !(-6.0..=12.0).contains(&fx.bass_db) {
            return Err("Bass must be between -6 and +12 dB.".into());
        }
        if !fx.width.is_finite() || !(0.0..=1.0).contains(&fx.width) {
            return Err("Width must be between 0 and 1.".into());
        }
        self.player.set_fx(&fx).map_err(|e| e.to_string())?;
        *self.audio_fx.write() = fx;
        self.emit("audio-fx", fx);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        append_page, autoplay_tail_near, backfill_metadata, drop_duplicates, enqueue_at,
        format_duration, guest_insert_index, is_mix, loudness_gain, media_title, merge_radio,
        next_index, parse_duration_ms, persist_fingerprint, queue_event_payload, queue_fingerprint,
        radio_seed_for, read_personal, shuffle_new_queue, shuffle_upcoming, splice_radio_into,
        spotify_mpv_options, unshuffled, upcoming_queued, validate_personal_blob,
        with_resume_position, QueueState, RepeatMode, PERSONAL_JSON_KEY,
    };
    use crate::db::Db;

    #[test]
    fn media_metadata_uses_the_omni_product_name_when_track_fields_are_empty() {
        assert_eq!(media_title("", ""), "Ryotunes Omni");
    }

    #[test]
    fn personal_round_trips_and_rejects_bad_blobs() {
        let db = Db::open(std::path::Path::new(":memory:")).unwrap();
        // Nothing saved yet reads as an empty object, not null or an error.
        assert_eq!(read_personal(&db), serde_json::json!({}));
        // A real store round-trips through SQLite byte for byte.
        let blob = serde_json::json!({
            "picks": [{ "id": "VL123", "kind": "playlist", "title": "Mix", "lastUsedAt": 42 }],
            "pins": ["VL123"],
            "recent": { "VL123": { "id": "VL123", "kind": "playlist", "at": 99 } },
            "home": { "order": ["@recent", "@familiar"], "hidden": [], "seen": ["Listen again"] }
        });
        db.set_setting(PERSONAL_JSON_KEY, &validate_personal_blob(&blob).unwrap());
        assert_eq!(read_personal(&db), blob);
        // A non-object is refused: the store is one keyed record, never a bare array or scalar.
        assert!(validate_personal_blob(&serde_json::json!([1, 2, 3])).is_err());
        assert!(validate_personal_blob(&serde_json::json!("nope")).is_err());
        assert!(validate_personal_blob(&serde_json::json!(7)).is_err());
        // Over a MiB is refused so a corrupt client can't wedge the store.
        let huge = serde_json::json!({ "x": "y".repeat(1024 * 1024) });
        assert!(validate_personal_blob(&huge).is_err());
        // A stored value that somehow isn't an object degrades to {} rather than propagating.
        db.set_setting(PERSONAL_JSON_KEY, "[1,2,3]");
        assert_eq!(read_personal(&db), serde_json::json!({}));
        db.set_setting(PERSONAL_JSON_KEY, "not json at all");
        assert_eq!(read_personal(&db), serde_json::json!({}));
    }

    #[test]
    fn queue_fingerprint_tracks_the_rows_not_their_metadata() {
        let row = |id: &str| innertube::SongItem { video_id: id.into(), ..Default::default() };
        let a = vec![row("a"), row("b"), row("c")];
        // Same rows in the same order: the UI already has them.
        assert_eq!(queue_fingerprint(&a), queue_fingerprint(&[row("a"), row("b"), row("c")]));
        // A reorder, an append and a removal all have to be visible.
        assert_ne!(queue_fingerprint(&a), queue_fingerprint(&[row("a"), row("c"), row("b")]));
        assert_ne!(queue_fingerprint(&a), queue_fingerprint(&[row("a"), row("b")]));
        assert_ne!(
            queue_fingerprint(&a),
            queue_fingerprint(&[row("a"), row("b"), row("c"), row("d")])
        );
        // A metadata repair on an existing row does not: it rides on the index event instead.
        let mut patched = a.clone();
        patched[0].duration = Some("3:11".into());
        assert_eq!(queue_fingerprint(&a), queue_fingerprint(&patched));
        // `restore_queue` compares these as strings off disk, not as numbers, so the string form
        // has to discriminate as well as the hash does: a stamp that matched everything would
        // silently re-open the wrong-track gap the stamp exists to close.
        assert_ne!(
            queue_fingerprint(&a).to_string(),
            queue_fingerprint(&[row("a"), row("b")]).to_string()
        );
    }

    // `move_in_queue` can drag one occurrence of a repeated videoId past the other (a track queued
    // twice from two different playlists, say). The panel tells the two rows apart by the fields
    // that differ between them, not by videoId alone, so the fingerprint has to as well: otherwise
    // the swap is invisible, `emit_queue` stays on the small `queue-index` event, and the drag
    // looks like it did nothing.
    #[test]
    fn queue_fingerprint_distinguishes_same_video_id_rows_by_queue_metadata() {
        let row = |queued_from: Option<&str>| innertube::SongItem {
            video_id: "x".into(),
            queued_from: queued_from.map(Into::into),
            ..Default::default()
        };
        let a = vec![row(Some("Chill Mix")), row(None)];
        let b = vec![row(None), row(Some("Chill Mix"))];
        assert_ne!(queue_fingerprint(&a), queue_fingerprint(&b));

        // Same story for `set_video_id`, the other field that tells apart two occurrences of the
        // same videoId (one came from a playlist page, so it carries a playlistSetVideoId; a
        // second copy queued manually doesn't).
        let row2 = |set_video_id: Option<&str>| innertube::SongItem {
            video_id: "x".into(),
            set_video_id: set_video_id.map(Into::into),
            ..Default::default()
        };
        let c = vec![row2(Some("1")), row2(Some("2"))];
        let d = vec![row2(Some("2")), row2(Some("1"))];
        assert_ne!(queue_fingerprint(&c), queue_fingerprint(&d));
    }

    // `shuffle_upcoming` only reorders rows after `current`, so toggling shuffle while the last
    // track is playing leaves `items` byte-identical: the rows-only `queue_fingerprint` can't see
    // it, and `persist_queue`'s dirty check has to cover the rest of what `queue_json` stores or
    // the toggle is lost across a restart.
    #[test]
    fn persist_fingerprint_changes_when_shuffle_toggles_with_identical_items() {
        let mut q = QueueState {
            items: vec![innertube::SongItem { video_id: "a".into(), ..Default::default() }],
            ..QueueState::default()
        };
        let off = persist_fingerprint(&q);
        q.shuffle_orig = Some(q.items.clone());
        let on = persist_fingerprint(&q);
        assert_ne!(off, on);
    }

    #[test]
    fn queue_changed_payload_carries_the_rows_and_no_current() {
        let q = QueueState {
            items: vec![
                innertube::SongItem { video_id: "a".into(), ..Default::default() },
                innertube::SongItem { video_id: "b".into(), ..Default::default() },
            ],
            current: 1,
            played_from: 0,
            repeat: RepeatMode::All,
            source_name: Some("Liked Music".into()),
            ..QueueState::default()
        };
        let payload = queue_event_payload(&q, false);
        assert_eq!(payload["items"].as_array().unwrap().len(), 2);
        assert_eq!(payload["items"][0]["video_id"], "a");
        assert_eq!(payload["currentIndex"], 1);
        assert_eq!(payload["playedFrom"], 0);
        assert_eq!(payload["shuffle"], false);
        assert_eq!(payload["repeat"], "all");
        assert_eq!(payload["sourceName"], "Liked Music");
        assert!(payload.get("current").is_none(), "the full payload never carries `current`");
    }

    #[test]
    fn queue_index_payload_patches_the_pointer_without_the_rows() {
        let q = QueueState {
            items: vec![innertube::SongItem { video_id: "a".into(), ..Default::default() }],
            current: 0,
            played_from: 0,
            repeat: RepeatMode::Off,
            source_name: None,
            shuffle_orig: Some(vec![innertube::SongItem {
                video_id: "a".into(),
                ..Default::default()
            }]),
            ..QueueState::default()
        };
        let payload = queue_event_payload(&q, true);
        assert!(payload.get("items").is_none(), "the small payload never carries `items`");
        assert_eq!(payload["currentIndex"], 0);
        assert_eq!(payload["shuffle"], true);
        assert_eq!(payload["repeat"], "off");
        assert!(payload["sourceName"].is_null(), "an unnamed queue is null, not absent");
        assert_eq!(payload["current"]["video_id"], "a");
    }

    #[test]
    fn queue_index_payload_of_an_empty_queue_points_past_the_end_safely() {
        let q = QueueState { repeat: RepeatMode::One, ..QueueState::default() };
        let payload = queue_event_payload(&q, true);
        assert_eq!(payload["currentIndex"], 0);
        assert_eq!(payload["repeat"], "one");
        assert!(payload["current"].is_null(), "no row exists, so current is null");
    }

    #[test]
    fn durations_round_trip_past_an_hour() {
        assert_eq!(format_duration(191_000), "3:11");
        assert_eq!(format_duration(5_468_000), "1:31:08");
        // The parser is the inverse for both shapes (queue rows persist the string).
        assert_eq!(parse_duration_ms(Some("1:31:08")), 5_468_000);
        assert_eq!(parse_duration_ms(Some("3:11")), 191_000);
    }
    use std::collections::HashSet;

    // `by.is_some()` (a named guest/host add) and a nameless solo add are both manual adds:
    // the `queued` marker is what forms the FIFO block, not the name.
    fn song(id: &str, by: Option<&str>) -> innertube::SongItem {
        innertube::SongItem {
            video_id: id.into(),
            title: id.into(),
            artists: String::new(),
            artist_id: None,
            artist_runs: Vec::new(),
            album: None,
            album_id: None,
            duration: None,
            play_count: None,
            thumbnail: None,
            set_video_id: None,
            added_by: None,
            added_by_avatar: None,
            rating: None,
            queued: by.is_some(),
            queued_end: false,
            queued_from: None,
            queued_by: by.map(Into::into),
            autoplay: false,
            is_video: false,
            is_upload: false,
            explicit: false,
        }
    }

    /// The repair every entry path goes through. Covers the two shapes that reached Last.fm as an
    /// artist name: nothing at all (album rows), and a whole display subtitle (song cards, and rows
    /// replayed out of On Repeat that were recorded back when the card parser leaked one).
    #[test]
    fn player_response_repairs_a_missing_or_bogus_artist() {
        let with = |artists: &str, runs: Vec<&str>| innertube::SongItem {
            artists: artists.into(),
            artist_runs: runs
                .into_iter()
                .map(|t| innertube::models::metadata::ArtistRun {
                    text: t.into(),
                    id: Some("UCstale".into()),
                })
                .collect(),
            ..song("v", None)
        };

        // No artist (an album track) → YouTube's own author.
        let mut it = with("", vec![]);
        backfill_metadata(&mut it, None, Some("Delara"), None);
        assert_eq!(it.artists, "Delara");

        // A display subtitle that was parsed as the artist → replaced, and the links that pointed
        // at the old text go with it (the UI renders runs instead of `artists` when they exist).
        for bogus in [
            "Miley Cyrus • Plastic Hearts • 2020",
            "late night slow • 29M views",
            "Song • Dua Lipa",
        ] {
            let mut it = with(bogus, vec![bogus]);
            backfill_metadata(&mut it, None, Some("Dua Lipa"), None);
            assert_eq!(it.artists, "Dua Lipa", "{bogus} should have been replaced");
            assert!(it.artist_runs.is_empty(), "{bogus}: stale links must not survive the swap");
        }

        // A real artist line is never second-guessed, links included. Collabs use "&" and ",".
        let mut it = with("Nicki Minaj, Ice Spice & Aqua", vec!["Nicki Minaj", " & ", "Aqua"]);
        backfill_metadata(&mut it, None, Some("Nicki Minaj"), None);
        assert_eq!(it.artists, "Nicki Minaj, Ice Spice & Aqua");
        assert_eq!(it.artist_runs.len(), 3);

        // Nothing to repair with (rustypipe served the stream, or /player had no author) → as-is.
        for author in [None, Some(""), Some("   ")] {
            let mut it = with("", vec![]);
            backfill_metadata(&mut it, None, author, None);
            assert_eq!(it.artists, "");
        }
    }

    #[test]
    fn player_response_fills_only_a_missing_duration() {
        let mut it = song("v", None);
        backfill_metadata(&mut it, Some("191"), None, None);
        assert_eq!(it.duration.as_deref(), Some("3:11"));
        // A duration the row already carried wins: it's the length of the cut YouTube listed.
        let mut it = innertube::SongItem { duration: Some("3:02".into()), ..song("v", None) };
        backfill_metadata(&mut it, Some("191"), None, None);
        assert_eq!(it.duration.as_deref(), Some("3:02"));
        // Junk from the player response can't blank an existing one.
        let mut it = song("v", None);
        backfill_metadata(&mut it, Some("not-a-number"), None, None);
        assert_eq!(it.duration, None);
    }

    /// The audio-only fetch filter reads this flag, and the rows that reach it are only as
    /// good as the parse that built them: a card played from a shelf carries no flag at all, and an
    /// album row keeps a `true` after being swapped to its audio id. `/player` settles both ways.
    #[test]
    fn player_response_settles_whether_the_track_is_a_video() {
        let mut it = song("v", None);
        backfill_metadata(&mut it, None, None, Some(true));
        assert!(it.is_video);
        // A stale `true` is cleared when YouTube identifies the resolved item as generated audio.
        backfill_metadata(&mut it, None, None, Some(false));
        assert!(!it.is_video);
        // Nothing said (rustypipe, or a cached URL) → whatever the row already claimed stands.
        it.is_video = true;
        backfill_metadata(&mut it, None, None, None);
        assert!(it.is_video);
    }

    #[test]
    fn guest_adds_stack_fifo_after_current() {
        let solo = |id: &str| innertube::SongItem { queued: true, ..song(id, None) };
        // Host playlist [A*, B, C] (playing A): manual add goes right after current, not the end.
        let items = vec![song("a", None), song("b", None), song("c", None)];
        assert_eq!(guest_insert_index(&items, 0), 1);
        // A guest track already up next → the new one queues behind it (FIFO), before B.
        let items = vec![song("a", None), song("g1", Some("kim")), song("b", None)];
        assert_eq!(guest_insert_index(&items, 0), 2);
        // Solo adds (no name, `queued`) form the same FIFO block: the next add lands behind them.
        let items = vec![song("a", None), solo("s1"), song("b", None)];
        assert_eq!(guest_insert_index(&items, 0), 2);
        // Current is the last item → append.
        let items = vec![song("a", None)];
        assert_eq!(guest_insert_index(&items, 0), 1);
        // Empty queue (nothing playing yet) → index 0… clamped, no panic.
        assert_eq!(guest_insert_index(&[], 0), 0);
    }

    // "Start radio" on the song that's already playing: the track keeps playing, the tail is
    // replaced, and the manual adds the user made are not collateral damage (Metrolist drops them).
    #[test]
    fn radio_replaces_the_tail_but_keeps_history_and_manual_adds() {
        let ids = |items: &[innertube::SongItem]| {
            items.iter().map(|i| i.video_id.clone()).collect::<Vec<_>>()
        };
        let mut q = QueueState {
            // played, playing, a manual add, then two tracks of the old context
            items: vec![
                song("done", None),
                song("now", None),
                song("mine", Some("me")),
                song("old1", None),
                song("old2", None),
            ],
            current: 1,
            ..QueueState::default()
        };
        // The radio's first page leads with the seed song — it must not be queued a second time.
        // Nor may it re-offer what's still in the queue (history, or the user's own adds). A track
        // from the replaced tail (`old1`) is fair game: it isn't in the queue any more.
        let page = vec![
            song("now", None),
            song("r1", None),
            song("done", None),
            song("mine", None),
            song("old1", None),
        ];
        splice_radio_into(&mut q, page, "RDAMVMnow".into(), Some("Now Radio".into()));

        assert_eq!(ids(&q.items), ["done", "now", "mine", "r1", "old1"]);
        assert_eq!(q.current, 1); // nothing moved under the playing track
        assert_eq!(q.radio_seed.as_deref(), Some("RDAMVMnow"));
        assert_eq!(q.source_name.as_deref(), Some("Now Radio"));
        // Radio tracks are the queue, not autoplay filler — no "Autoplay" divider under them.
        assert!(q.items.iter().all(|i| !i.autoplay));
    }

    #[test]
    fn radio_started_under_shuffle_stays_shuffled_and_restorable() {
        let mut items = vec![song("now", None)];
        let mut q = QueueState {
            shuffle_orig: Some(items.clone()),
            items: std::mem::take(&mut items),
            current: 0,
            ..QueueState::default()
        };
        let page: Vec<_> = (0..50).map(|i| song(&format!("r{i}"), None)).collect();
        splice_radio_into(&mut q, page, "RDAMVMnow".into(), None);

        assert_eq!(q.items[0].video_id, "now"); // the playing track never moves
                                                // Un-shuffle has the whole radio to restore, not just what was there before it started.
        let orig = q.shuffle_orig.as_ref().unwrap();
        assert_eq!(orig.len(), 51);
        let mut a: Vec<_> = orig.iter().map(|i| i.video_id.clone()).collect();
        let mut b: Vec<_> = q.items.iter().map(|i| i.video_id.clone()).collect();
        a.sort();
        b.sort();
        assert_eq!(a, b); // nothing lost, nothing duplicated
    }

    #[test]
    fn next_index_wraps_only_on_repeat_all() {
        assert_eq!(next_index(3, 2, RepeatMode::Off), None);
        assert_eq!(next_index(3, 2, RepeatMode::All), Some(0));
        assert_eq!(next_index(3, 2, RepeatMode::One), None); // mpv loops the file itself
        assert_eq!(next_index(3, 0, RepeatMode::Off), Some(1));
        assert_eq!(next_index(0, 0, RepeatMode::All), None);
        assert_eq!(next_index(1, 0, RepeatMode::All), Some(0)); // single-item wrap
    }

    // The background playlist walk: a page that lands mid-playback has to reach the *unplayed*
    // tail (otherwise "shuffle" only ever shuffles page 1), without moving the track already
    // handed to mpv for the gapless advance.
    #[test]
    fn appended_page_mixes_into_the_tail_but_leaves_the_primed_slot() {
        let ids = |items: &[innertube::SongItem]| {
            items.iter().map(|i| i.video_id.clone()).collect::<Vec<_>>()
        };
        let page = || (0..100).map(|i| song(&format!("p{i}"), None)).collect::<Vec<_>>();
        let mut tail_positions = Vec::new();

        for _ in 0..20 {
            let items = vec![song("a", None), song("b", None), song("c", None)];
            let mut q = QueueState {
                shuffle_orig: Some(items.clone()),
                items,
                current: 0,
                lookahead_loaded: Some(1), // "b" is already loaded into mpv
                ..QueueState::default()
            };
            append_page(&mut q, page(), true);

            assert_eq!(q.items.len(), 103);
            assert_eq!(q.items[0].video_id, "a"); // playing
            assert_eq!(q.items[1].video_id, "b"); // primed gapless slot, pinned
                                                  // Un-shuffle restores the whole playlist, including pages that arrived after playback
                                                  // started.
            assert_eq!(ids(q.shuffle_orig.as_ref().unwrap()).len(), 103);
            assert_eq!(ids(q.shuffle_orig.as_ref().unwrap())[..3], ["a", "b", "c"]);
            let mut sorted = ids(&q.items);
            sorted.sort();
            let mut expected = ids(q.shuffle_orig.as_ref().unwrap());
            expected.sort();
            assert_eq!(sorted, expected); // nothing lost, nothing duplicated

            tail_positions.push(q.items.iter().position(|i| i.video_id == "c").unwrap());
        }
        // Without the re-shuffle the page would sit behind "c" and it would land at index 2 every
        // time. (Staying at 2 in all 20 runs by chance is (1/101)^20.)
        assert!(tail_positions.iter().any(|&p| p != 2), "page was appended, not mixed in");
    }

    #[test]
    fn appended_page_keeps_order_when_shuffle_is_off() {
        let mut q = QueueState {
            items: vec![song("a", None), song("b", None)],
            current: 0,
            ..QueueState::default()
        };
        append_page(&mut q, vec![song("c", None), song("d", None)], true);
        let ids: Vec<_> = q.items.iter().map(|i| i.video_id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c", "d"]);
        assert!(q.shuffle_orig.is_none());
    }

    // The rest of an added playlist joins the block its first page went into — not the tail of the
    // queue, which is a different place entirely now that "Add to queue" inserts up front. Order
    // kept even under shuffle: the user queued that album, shuffle belongs to what's playing.
    #[test]
    fn appended_page_joins_the_add_to_queue_block_it_belongs_to() {
        let added = |id: &str| innertube::SongItem { queued_end: true, ..song(id, None) };
        let items = vec![song("a", None), added("x1"), song("b", None)];
        let mut q = QueueState {
            shuffle_orig: Some(items.clone()),
            items,
            current: 0,
            ..QueueState::default()
        };
        append_page(&mut q, vec![added("x2"), added("x3")], false);
        let ids: Vec<_> = q.items.iter().map(|i| i.video_id.as_str()).collect();
        assert_eq!(ids, ["a", "x1", "x2", "x3", "b"]);
        // Still on the snapshot, so un-shuffle keeps them.
        assert_eq!(q.shuffle_orig.as_ref().unwrap().len(), 5);
    }

    // The played run ("Previously played") is `played_from..current`, and only moving the pointer
    // backwards may extend it: forward jumps mean those tracks really were passed over.
    #[test]
    fn the_played_run_follows_the_pointer_backwards_only() {
        let mut q = QueueState {
            items: vec![song("a", None), song("b", None), song("c", None), song("d", None)],
            current: 1,
            played_from: 1,
            ..QueueState::default()
        };
        q.seek_to(3); // jumped over "c", so it counts: heard or not, it's behind the playing track
        assert_eq!((q.played_from, q.current), (1, 3));
        q.seek_to(0); // back to the top: nothing is behind it any more
        assert_eq!((q.played_from, q.current), (0, 0));
    }

    // The one that silently breaks: a duplicate sitting *before* the playing track. Removing it
    // renumbers the queue, so `current` has to follow its own song or the insert lands one slot off.
    #[test]
    fn dropping_a_duplicate_before_the_current_track_moves_the_current_index() {
        let mut items =
            vec![song("dup", None), song("a", None), song("b", None), song("dup", None)];
        let mut current = 1; // playing "a"
        let mut played_from = 0; // "dup" was heard, then "a" started
        let ids = HashSet::from(["dup"]);
        assert!(drop_duplicates(&mut items, &mut current, &mut played_from, &ids));

        let left: Vec<_> = items.iter().map(|i| i.video_id.as_str()).collect();
        assert_eq!(left, ["a", "b"]);
        assert_eq!(current, 0); // still playing "a"
        assert_eq!(played_from, 0); // its one played track went with it; the run is empty, not stale
        assert_eq!(guest_insert_index(&items, current), 1); // the add lands right after it

        // The playing track is exempt: "play next" on the current song is a repeat gesture.
        let mut items = vec![song("a", None), song("b", None)];
        let mut current = 0;
        let mut played_from = 0;
        assert!(!drop_duplicates(
            &mut items,
            &mut current,
            &mut played_from,
            &HashSet::from(["a"])
        ));
        assert_eq!(items.len(), 2);
    }

    // "Add to queue" lands at the back of the manual block, ahead of the context — the bug in #26
    // was it landing at the very end, where a radio or a long playlist buries it forever.
    #[test]
    fn add_to_queue_goes_behind_the_manual_block_but_ahead_of_the_context() {
        let queued = |id: &str| innertube::SongItem { queued: true, ..song(id, None) };
        let added = |id: &str| innertube::SongItem { queued_end: true, ..song(id, None) };

        // Plain playlist queue → straight behind the playing track.
        let q = QueueState {
            items: vec![song("a", None), song("b", None), song("c", None)],
            current: 0,
            ..QueueState::default()
        };
        assert_eq!(enqueue_at(&q), 1);

        // Behind a waiting "Play next" block and behind earlier adds, never inside either.
        let q = QueueState {
            items: vec![song("a", None), queued("mine"), added("x1"), song("b", None)],
            current: 0,
            ..QueueState::default()
        };
        assert_eq!(enqueue_at(&q), 3);

        // A radio is no different: the add is heard next instead of after an endless feed.
        let q = QueueState {
            items: vec![song("r1", None), song("r2", None), song("r3", None)],
            current: 0,
            radio: true,
            ..QueueState::default()
        };
        assert_eq!(enqueue_at(&q), 1);

        // Nothing after the playing track: appended, not out of bounds.
        let q = QueueState { items: vec![song("a", None)], current: 0, ..QueueState::default() };
        assert_eq!(enqueue_at(&q), 1);
    }

    #[test]
    fn mixes_are_not_walked() {
        assert!(is_mix(Some("VLRDCLAK5uy_abc"))); // a mix as the playlist page sees it
        assert!(is_mix(Some("RDAMVMxyz")));
        assert!(!is_mix(Some("VLPL123")));
        assert!(!is_mix(Some("VLOLAK5uy_abc")));
        assert!(!is_mix(None));
    }

    #[test]
    fn unshuffle_restores_order_and_current() {
        let orig = vec![song("a", None), song("b", None), song("c", None), song("d", None)];
        let heard = |ids: &[&str]| ids.iter().map(|s| (*s).to_owned()).collect::<HashSet<String>>();
        let ids = |items: &[innertube::SongItem]| {
            items.iter().map(|i| i.video_id.clone()).collect::<Vec<_>>()
        };
        // Shuffle played "c" first → nothing upcoming has been heard, plain restore.
        let (items, idx) = unshuffled(orig.clone(), &heard(&["c"]), "c", 9);
        assert_eq!(ids(&items), ["a", "b", "c", "d"]);
        assert_eq!(idx, 2);
        // A full shuffled pass ending on "c": "d" already played, so it must not come back as
        // up-next; "a"/"b" stay behind the playing track as hidden history.
        let (items, idx) = unshuffled(orig.clone(), &heard(&["a", "b", "c", "d"]), "c", 9);
        assert_eq!(ids(&items), ["a", "b", "c"]);
        assert_eq!(idx, 2);
        // Partial pass: only the heard ones are dropped from upcoming, order otherwise intact.
        let (items, idx) = unshuffled(orig.clone(), &heard(&["a", "c"]), "a", 9);
        assert_eq!(ids(&items), ["a", "b", "d"]);
        assert_eq!(idx, 0);
        // Playing id absent from the snapshot → fallback index, clamped to len-1.
        let (_, idx) = unshuffled(orig, &HashSet::new(), "zz", 9);
        assert_eq!(idx, 3);

        // "Play next" adds are appended to the snapshot as they're made, so a plain restore would
        // drop them behind the whole playlist. They belong right after the playing track.
        let with_adds = vec![
            song("a", None),
            song("b", None),
            song("c", None),
            innertube::SongItem { queued: true, ..song("mine1", None) },
            innertube::SongItem { queued: true, ..song("mine2", None) },
        ];
        let (items, idx) = unshuffled(with_adds, &heard(&["a"]), "a", 9);
        assert_eq!(ids(&items), ["a", "mine1", "mine2", "b", "c"]);
        assert_eq!(idx, 0);
    }

    #[test]
    fn shuffle_preserves_prefix_and_multiset() {
        let ids: Vec<String> = (0..10).map(|i| format!("t{i}")).collect();
        let mut items: Vec<_> = ids.iter().map(|id| song(id, None)).collect();
        shuffle_upcoming(&mut items, 2);
        // The playing track and the already-played prefix stay put…
        for (i, id) in ids.iter().take(3).enumerate() {
            assert_eq!(&items[i].video_id, id);
        }
        // …and the whole queue is a permutation (nothing lost, nothing duplicated).
        let mut got: Vec<_> = items.iter().map(|i| i.video_id.clone()).collect();
        got.sort();
        let mut want = ids.clone();
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn new_queue_shuffle_plays_clicked_track_first() {
        let ids: Vec<String> = (0..10).map(|i| format!("t{i}")).collect();
        let mut items: Vec<_> = ids.iter().map(|id| song(id, None)).collect();
        let cur = shuffle_new_queue(&mut items, 7);
        assert_eq!(cur, 0);
        assert_eq!(items[0].video_id, "t7"); // the clicked track plays first…
        let mut got: Vec<_> = items.iter().map(|i| i.video_id.clone()).collect();
        got.sort();
        let mut want = ids.clone();
        want.sort();
        assert_eq!(got, want); // …and the rest is a permutation of everything else
                               // Degenerate cases: empty and single-item queues don't panic.
        let mut empty: Vec<innertube::SongItem> = vec![];
        assert_eq!(shuffle_new_queue(&mut empty, 3), 0);
        let mut one = vec![song("only", None)];
        assert_eq!(shuffle_new_queue(&mut one, 5), 0);
        assert_eq!(one[0].video_id, "only");
    }

    #[test]
    fn manual_adds_survive_context_switch() {
        let solo = |id: &str| innertube::SongItem { queued: true, ..song(id, None) };
        // Playing B (index 1); Q1/Q2 are unplayed manual adds, Q0 already played — only the
        // unplayed ones carry into a new queue.
        let items = vec![solo("q0"), song("b", None), solo("q1"), song("c", None), solo("q2")];
        let carried = upcoming_queued(&items, 1);
        let ids: Vec<_> = carried.iter().map(|i| i.video_id.as_str()).collect();
        assert_eq!(ids, ["q1", "q2"]);
        // Nothing queued upcoming → nothing carried.
        assert!(upcoming_queued(&[song("a", None)], 0).is_empty());
    }

    #[test]
    fn shuffle_leaves_manual_queue_block_in_place() {
        let solo = |id: &str| innertube::SongItem { queued: true, ..song(id, None) };
        let mut items = vec![song("now", None), solo("q1"), solo("q2")];
        items.extend((0..8).map(|i| song(&format!("t{i}"), None)));
        shuffle_upcoming(&mut items, 0);
        // The manual "Next in queue" block still plays next, in order.
        assert_eq!(items[1].video_id, "q1");
        assert_eq!(items[2].video_id, "q2");
        // Everything is still a permutation (nothing lost).
        assert_eq!(items.len(), 11);
    }

    // A whole album queued with "Play next" is a set of tracks the user shuffles like any other:
    // the block keeps its place at the front, its own tracks get randomized. Songs queued one at a
    // time keep the order they were queued in.
    #[test]
    fn shuffle_randomizes_a_play_next_album_but_not_loose_adds() {
        let solo = |id: &str| innertube::SongItem { queued: true, ..song(id, None) };
        let from = |id: &str, name: &str| innertube::SongItem {
            queued: true,
            queued_from: Some(name.into()),
            ..song(id, None)
        };
        let mut moved = false;
        for _ in 0..20 {
            let mut items = vec![song("now", None), solo("q1"), solo("q2")];
            items.extend((0..12).map(|i| from(&format!("a{i}"), "Album")));
            items.extend((0..4).map(|i| song(&format!("t{i}"), None)));
            shuffle_upcoming(&mut items, 0);
            // The loose adds stay where they were, in order, ahead of the album block.
            assert_eq!(items[1].video_id, "q1");
            assert_eq!(items[2].video_id, "q2");
            // The album block stays a block — it just plays in a different order inside it.
            let block: Vec<_> = items[3..15].iter().map(|i| i.video_id.clone()).collect();
            assert!(block.iter().all(|id| id.starts_with('a')), "the block kept its place");
            moved |= block != (0..12).map(|i| format!("a{i}")).collect::<Vec<_>>();
            let mut sorted = block.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 12, "nothing lost or duplicated");
        }
        assert!(moved, "the album block was never shuffled");
    }

    #[test]
    fn shuffle_keeps_autoplay_after_queue_tracks() {
        let auto = |id: &str| innertube::SongItem { autoplay: true, ..song(id, None) };
        // Playing index 0; upcoming = 4 playlist tracks + 4 autoplay tracks.
        let mut items = vec![song("now", None)];
        items.extend((0..4).map(|i| song(&format!("p{i}"), None)));
        items.extend((0..4).map(|i| auto(&format!("a{i}"))));
        shuffle_upcoming(&mut items, 0);
        // Every playlist track still comes before every autoplay track.
        let flags: Vec<bool> = items[1..].iter().map(|i| i.autoplay).collect();
        assert_eq!(flags, [false, false, false, false, true, true, true, true]);
        assert_eq!(items.len(), 9);
    }

    #[test]
    fn radio_seed_from_source() {
        // Playlist browseIds are VL-prefixed — stripped before building the radio id.
        // A mix is already a radio playlist — it seeds autoplay as itself, not wrapped again.
        assert_eq!(radio_seed_for(Some("VLRDCLAK5uy_x".into())).as_deref(), Some("RDCLAK5uy_x"));
        assert_eq!(radio_seed_for(Some("VLPL123".into())).as_deref(), Some("RDAMPLPL123"));
        // Album audio playlist ids come bare.
        assert_eq!(radio_seed_for(Some("OLAK5uy_x".into())).as_deref(), Some("RDAMPLOLAK5uy_x"));
        // No source (single song / artist top-songs) → no pinned seed.
        assert_eq!(radio_seed_for(None), None);
    }

    #[test]
    fn autoplay_waits_for_the_last_two_tracks() {
        // A single-song play is at the tail; once its radio has hydrated the queue it is not.
        assert!(autoplay_tail_near(1, 0));
        assert!(autoplay_tail_near(5, 3));
        assert!(autoplay_tail_near(5, 4));
        assert!(!autoplay_tail_near(50, 0));
        assert!(!autoplay_tail_near(5, 2));
        assert!(autoplay_tail_near(0, 0));
    }

    #[test]
    fn autoplay_merge_dedupes_and_caps() {
        let mut items = vec![song("a", None), song("b", None)];
        let existing: std::collections::HashSet<String> =
            items.iter().map(|i| i.video_id.clone()).collect();
        // "a" is already queued, "c" appears twice in the radio result — one copy survives.
        let fresh = vec![
            song("a", None),
            song("c", None),
            song("c", None),
            song("d", None),
            song("e", None),
        ];
        let added = merge_radio(&mut items, fresh, existing.clone(), 2);
        assert_eq!(added, 2); // cap honored: c + d, e cut off
        let ids: Vec<_> = items.iter().map(|i| i.video_id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c", "d"]);
        // Appended tracks are marked autoplay (the UI's divider/badge key); originals aren't.
        assert!(items[2].autoplay && items[3].autoplay);
        assert!(!items[0].autoplay && !items[1].autoplay);
        // Nothing new in the radio result → 0, queue untouched (playback then stops as before).
        assert_eq!(merge_radio(&mut items.clone(), vec![song("a", None)], existing, 20), 0);
    }

    #[test]
    fn loudness_gain_attenuates_only_above_the_target() {
        // "As It Was" measures loudnessDb 7.77 ⇒ -6.23 LUFS, 0.77 dB over the -7 target.
        assert_eq!(loudness_gain(Some(7.77)).map(|g| (g * 100.0).round()), Some(-77.0));
        // Exactly on target, and everything below -7 LUFS, gets no filter at all — YTM doesn't
        // touch these either. (Real values: "Levitating" 6.85, "Shape of You" 6.35, "bad guy" 0.11.)
        for l in [7.0, 6.85, 6.35, 0.11, -5.0] {
            assert_eq!(loudness_gain(Some(l)), None, "loudnessDb {l} should not attenuate");
        }
        // Extreme loudness clamps at −24 dB.
        assert_eq!(loudness_gain(Some(40.0)), Some(-24.0));
        // No metadata → no filter.
        assert_eq!(loudness_gain(None), None);
    }

    // The FIFO carries raw PCM mpv can't probe; these exact options are the whole contract with
    // librespot's `pipe` backend (S16LE / 44100 / stereo). A typo would fail playback silently.
    #[test]
    fn spotify_mpv_options_are_the_rawaudio_demuxer_settings() {
        assert_eq!(
            spotify_mpv_options(),
            vec![
                "demuxer=rawaudio",
                "demuxer-rawaudio-format=s16le",
                "demuxer-rawaudio-rate=44100",
                "demuxer-rawaudio-channels=2",
            ]
        );
    }

    #[test]
    fn a_resume_position_loads_as_the_start_option() {
        assert_eq!(with_resume_position(&[], Some(83.4)), vec!["start=83.400"]);
        let options = vec!["demuxer=rawaudio".to_owned()];
        assert_eq!(
            with_resume_position(&options, Some(1.0)),
            vec!["demuxer=rawaudio", "start=1.000"]
        );
        for nothing in [None, Some(0.0), Some(-3.0), Some(f64::NAN), Some(f64::INFINITY)] {
            assert_eq!(with_resume_position(&options, nothing), options, "{nothing:?}");
        }
    }

    // Real Spotify playback needs a Premium session and the librespot pipe backend, which can't run
    // in CI (no credentials, no audio device). This documents the manual acceptance path; run it
    // by hand on the rig against a signed-in Premium account. See `crates/spotify/README.md`.
    #[test]
    #[ignore = "needs a real Spotify Premium session + audio device; run manually on the rig"]
    fn spotify_premium_playback_manual() {
        // 1. Sign in to a Premium account (daemon `spotify_sign_in`), select the Spotify provider.
        // 2. Play a `spotify:track:` item: `resolve` opens a librespot stream, mpv reads the FIFO
        //    with `spotify_mpv_options()`, audio plays; duration is seeded from the queue item.
        // 3. Seek: a plain mpv seek within its cache of the whole decoded track.
        // 4. Pause/resume: mpv only — FIFO backpressure stalls librespot, the pipe stays open.
        // 5. Let it end: the stream watcher closes the FIFO at librespot's EndOfTrack, mpv plays
        //    out its cache and EOFs, `on_track_ended` advances the queue.
        // 6. Play while signed out: `resolve` returns `SignInRequired` and the UI toasts it.
    }
}
