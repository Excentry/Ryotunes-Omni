//! Spotify browsing for any host: each call takes the shared [`SpotifyState`], reads the crate and
//! returns the same app shapes the YouTube path returns, through [`spotify_bridge`]. The Tauri host
//! uses these directly; the Linux daemon has its own copies in `ryotunesd` and can move onto these.

use std::fmt;
use std::sync::Arc;

use innertube::{
    AlbumPage, ArtistPage, BrowseItem, PlaylistPage, PlaylistSort, SearchResults, Section, SongItem,
};
use ryotunes_spotify::Client;

use crate::spotify::SpotifyState;
use crate::spotify_bridge;

/// How many liked tracks [`Client::liked`] returns per page.
const LIKED_PAGE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyBrowseError {
    /// No usable session: the host shows the sign-in state instead of an error.
    SignedOut,
    Failed(String),
}

impl fmt::Display for SpotifyBrowseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignedOut => f.write_str("Sign in to Spotify"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for SpotifyBrowseError {}

pub type Result<T> = std::result::Result<T, SpotifyBrowseError>;

fn failed(e: anyhow::Error) -> SpotifyBrowseError {
    SpotifyBrowseError::Failed(format!("Spotify: {e:#}"))
}

async fn client(spotify: &SpotifyState) -> Result<Arc<Client>> {
    spotify.client().await.ok_or(SpotifyBrowseError::SignedOut)
}

pub async fn search_all(spotify: &SpotifyState, query: &str) -> Result<SearchResults> {
    let results = client(spotify).await?.search(query).await.map_err(failed)?;
    Ok(spotify_bridge::search_all(&results))
}

pub async fn search_songs(spotify: &SpotifyState, query: &str) -> Result<Vec<SongItem>> {
    let results = client(spotify).await?.search(query).await.map_err(failed)?;
    Ok(spotify_bridge::search_songs(&results))
}

pub async fn search_cards(
    spotify: &SpotifyState,
    query: &str,
    category: &str,
) -> Result<Vec<BrowseItem>> {
    let results = client(spotify).await?.search(query).await.map_err(failed)?;
    Ok(spotify_bridge::search_cards(&results, category))
}

/// The liked-songs card followed by every playlist in the library, owned or followed.
pub async fn library(spotify: &SpotifyState) -> Result<Vec<BrowseItem>> {
    let playlists = client(spotify).await?.playlists(200).await.map_err(failed)?;
    let mut items = vec![spotify_bridge::liked_card()];
    items.extend(playlists.iter().map(spotify_bridge::playlist_to_card));
    Ok(items)
}

pub async fn library_albums(spotify: &SpotifyState) -> Result<Vec<BrowseItem>> {
    let albums = client(spotify).await?.saved_albums(200).await.map_err(failed)?;
    Ok(albums.iter().map(spotify_bridge::album_to_card).collect())
}

pub async fn library_artists(spotify: &SpotifyState) -> Result<Vec<BrowseItem>> {
    let artists = client(spotify).await?.saved_artists(200).await.map_err(failed)?;
    Ok(artists.iter().map(spotify_bridge::saved_artist_to_card).collect())
}

/// `pid` is the bare playlist id; `liked` pages every saved track.
pub async fn playlist(
    spotify: &SpotifyState,
    pid: &str,
    sort: Option<PlaylistSort>,
    desc: bool,
) -> Result<PlaylistPage> {
    let client = client(spotify).await?;
    if pid == "liked" {
        let mut tracks = Vec::new();
        let mut page = 0u32;
        loop {
            let batch = client.liked(page).await.map_err(failed)?;
            let full = batch.len() >= LIKED_PAGE;
            tracks.extend(batch);
            if !full {
                break;
            }
            page += 1;
        }
        Ok(spotify_bridge::liked_page(&tracks, sort, desc))
    } else {
        let detail = client.playlist(pid).await.map_err(failed)?;
        Ok(spotify_bridge::playlist_page(&detail, sort, desc))
    }
}

/// `id` is the bare album id.
pub async fn album(spotify: &SpotifyState, id: &str) -> Result<AlbumPage> {
    let detail = client(spotify).await?.album(id).await.map_err(failed)?;
    Ok(spotify_bridge::album_page(&detail))
}

/// `id` is the bare artist id.
pub async fn artist(spotify: &SpotifyState, id: &str) -> Result<ArtistPage> {
    let artist = client(spotify).await?.artist(id).await.map_err(failed)?;
    Ok(spotify_bridge::artist_page(id, &artist))
}

/// Spotify's made-for-you shelves for the merged Home. Best effort: a missing session or a failed
/// read contributes nothing, so Spotify can never blank the page.
pub async fn home_sections(spotify: &SpotifyState) -> Vec<Section> {
    let Some(client) = spotify.client().await else {
        return Vec::new();
    };
    match client.home().await {
        Ok(feed) => spotify_bridge::home_page(&feed).sections,
        Err(e) => {
            tracing::warn!(error = %e, "spotify: home shelves unavailable");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spotify::Provider;

    #[tokio::test]
    async fn every_call_reports_signed_out_without_a_session() {
        let dir = tempfile::tempdir().unwrap();
        let spotify = SpotifyState::new(dir.path().to_path_buf(), Provider::Spotify);
        assert_eq!(search_all(&spotify, "q").await.unwrap_err(), SpotifyBrowseError::SignedOut);
        assert_eq!(search_songs(&spotify, "q").await.unwrap_err(), SpotifyBrowseError::SignedOut);
        assert_eq!(
            search_cards(&spotify, "q", "albums").await.unwrap_err(),
            SpotifyBrowseError::SignedOut
        );
        assert_eq!(library(&spotify).await.unwrap_err(), SpotifyBrowseError::SignedOut);
        assert_eq!(library_albums(&spotify).await.unwrap_err(), SpotifyBrowseError::SignedOut);
        assert_eq!(library_artists(&spotify).await.unwrap_err(), SpotifyBrowseError::SignedOut);
        assert_eq!(
            playlist(&spotify, "liked", None, false).await.unwrap_err(),
            SpotifyBrowseError::SignedOut
        );
        assert_eq!(album(&spotify, "a").await.unwrap_err(), SpotifyBrowseError::SignedOut);
        assert_eq!(artist(&spotify, "a").await.unwrap_err(), SpotifyBrowseError::SignedOut);
        assert!(home_sections(&spotify).await.is_empty());
    }

    #[test]
    fn signed_out_reads_as_the_sign_in_prompt() {
        assert_eq!(SpotifyBrowseError::SignedOut.to_string(), "Sign in to Spotify");
    }
}
