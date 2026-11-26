//! Ryotunes playback core. Everything that is not a window lives here; the host (Tauri today, the
//! daemon tomorrow) supplies the three traits in [`host`].
pub mod host;

pub mod catalog_match;
pub mod cipher;
pub mod db;
pub mod discord;
pub mod http;
pub mod lastfm;
pub mod listentogether;
pub mod local;
pub mod lyrics;
pub mod media;
pub mod orchestrator;
pub mod potoken;
pub mod radio;
pub mod session;
pub mod soundcloud_bridge;
pub mod spotify;
pub mod spotify_bridge;
pub mod spotify_browse;
pub mod spotify_import;
pub mod spotify_metadata;
pub mod state;
pub mod tidal_bridge;
pub mod update;
