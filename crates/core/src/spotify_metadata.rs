use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

pub const CLIENT_ID_ENV: &str = "RYOTUNES_SPOTIFY_CLIENT_ID";
const REDIRECT: &str = "http://127.0.0.1:8989/login";
const MAX_BODY: usize = 2 * 1024 * 1024;
pub const MAX_PLAYLIST_OCCURRENCES: usize = 1000;

pub struct Authorization {
    listener: TcpListener,
    client_id: String,
    verifier: String,
    state: String,
    pub url: String,
}

#[derive(Clone)]
pub struct MetadataSession {
    client: reqwest::Client,
    token: String,
    expires: Instant,
    pub account_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTrack {
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
    pub outcome: crate::spotify_import::ImportOutcome,
}

pub struct SourcePlaylist {
    pub id: String,
    pub snapshot: String,
    pub title: String,
    pub tracks: Vec<SourceTrack>,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| "Could not initialize Spotify metadata client.".into())
}

async fn json_response(mut response: reqwest::Response) -> Result<serde_json::Value, String> {
    if !response.status().is_success() {
        return Err(format!(
            "Spotify metadata request failed (HTTP {}). No automatic retry.",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "Spotify response interrupted.")? {
        if bytes.len() + chunk.len() > MAX_BODY {
            return Err("Spotify response exceeded the safety limit.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Spotify returned invalid metadata.".into())
}

fn client_id(value: Option<String>) -> Result<String, String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value.bytes().all(|b| b.is_ascii_alphanumeric())
        })
        .ok_or_else(|| format!("Set {CLIENT_ID_ENV} to the Spotify app client ID for this lab."))
}

fn callback_code(target: &str, expected_state: &str) -> Result<String, String> {
    if !target.starts_with("/login?") || target.len() > 8192 {
        return Err("Unexpected authorization callback.".into());
    }
    let url = Url::parse(&format!("http://127.0.0.1:8989{target}"))
        .map_err(|_| "Invalid authorization callback.")?;
    if url.path() != "/login" || url.fragment().is_some() {
        return Err("Unexpected authorization callback.".into());
    }
    let mut parameters = HashMap::new();
    for (key, value) in url.query_pairs() {
        if parameters.insert(key.into_owned(), value.into_owned()).is_some() {
            return Err("Duplicate authorization parameter.".into());
        }
    }
    if parameters.get("state").map(String::as_str) != Some(expected_state) {
        return Err("Authorization state mismatch.".into());
    }
    if parameters.contains_key("error") {
        return Err("Spotify authorization was denied.".into());
    }
    parameters
        .remove("code")
        .filter(|code| !code.is_empty())
        .ok_or_else(|| "Missing authorization code.".into())
}

impl Authorization {
    pub async fn begin() -> Result<Self, String> {
        let client_id = client_id(std::env::var(CLIENT_ID_ENV).ok())?;
        let listener = TcpListener::bind(("127.0.0.1", 8989)).await.map_err(|_| {
            "Spotify callback port 8989 is busy. Close the previous probe and retry."
        })?;
        let verifier = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
        let state = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url = Url::parse("https://accounts.spotify.com/authorize")
            .map_err(|_| "Could not prepare Spotify authorization.")?;
        url.query_pairs_mut().extend_pairs([
            ("client_id", client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", REDIRECT),
            ("state", &state),
            ("scope", "playlist-read-private playlist-read-collaborative"),
            ("code_challenge_method", "S256"),
            ("code_challenge", &challenge),
        ]);
        Ok(Self { listener, client_id, verifier, state, url: url.into() })
    }

    pub async fn finish(self) -> Result<MetadataSession, String> {
        tokio::time::timeout(Duration::from_secs(180), self.receive())
            .await
            .map_err(|_| "Spotify authorization timed out. Try connecting again.".to_string())?
    }

    async fn receive(self) -> Result<MetadataSession, String> {
        let mut code = None;
        for _ in 0..12 {
            let (mut stream, _) =
                self.listener.accept().await.map_err(|_| "Spotify callback listener failed.")?;
            let request = tokio::time::timeout(Duration::from_secs(3), async {
                let mut buffer = Vec::new();
                loop {
                    let byte = stream.read_u8().await.map_err(|_| ())?;
                    buffer.push(byte);
                    if buffer.ends_with(b"\r\n\r\n") {
                        break;
                    }
                    if buffer.len() >= 8192 {
                        return Err(());
                    }
                }
                String::from_utf8(buffer).map_err(|_| ())
            })
            .await;
            let parsed = match request {
                Ok(Ok(request)) => {
                    let mut parts = request.lines().next().unwrap_or_default().split_whitespace();
                    if parts.next() == Some("GET") {
                        callback_code(parts.next().unwrap_or_default(), &self.state)
                    } else {
                        Err("Invalid callback method.".into())
                    }
                }
                _ => Err("Invalid callback request.".into()),
            };
            let denied = parsed
                .as_ref()
                .err()
                .is_some_and(|error| error == "Spotify authorization was denied.");
            let (status, body) = if parsed.is_ok() {
                ("200 OK", "Authorization received. Return to Ryotunes Omni.")
            } else {
                ("400 Bad Request", "Authorization not accepted. Return to Ryotunes Omni.")
            };
            let reply = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            let _ =
                tokio::time::timeout(Duration::from_secs(2), stream.write_all(reply.as_bytes()))
                    .await;
            if denied {
                return Err("Spotify authorization was denied.".into());
            }
            if let Ok(value) = parsed {
                code = Some(value);
                break;
            }
        }
        let code = code.ok_or("No valid Spotify callback received.")?;
        let client = client()?;
        let response = client
            .post("https://accounts.spotify.com/api/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", REDIRECT),
                ("client_id", self.client_id.as_str()),
                ("code_verifier", self.verifier.as_str()),
            ])
            .send()
            .await
            .map_err(|_| "Spotify token exchange failed.")?;
        let value = json_response(response).await?;
        let token = value["access_token"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or("Spotify did not return an access token.")?
            .to_owned();
        if value["token_type"].as_str().map(|value| value.eq_ignore_ascii_case("bearer"))
            != Some(true)
        {
            return Err("Unsupported Spotify token type.".into());
        }
        let seconds = value["expires_in"]
            .as_u64()
            .filter(|seconds| *seconds > 0 && *seconds <= 86_400)
            .ok_or("Invalid Spotify session expiry.")?;
        let mut session = MetadataSession {
            client,
            token,
            expires: Instant::now() + Duration::from_secs(seconds),
            account_key: String::new(),
        };
        let profile = session.get("/me").await?;
        let id = profile["id"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() < 512)
            .ok_or("Spotify profile identity is missing.")?;
        session.account_key = format!("spotify:{:x}", Sha256::digest(id.as_bytes()));
        Ok(session)
    }
}

pub fn playlist_id(raw: &str) -> Result<String, String> {
    let url = Url::parse(raw).map_err(|_| "Paste a Spotify playlist URL.")?;
    let segments: Vec<_> = url.path_segments().ok_or("Invalid playlist URL.")?.collect();
    if url.scheme() != "https"
        || url.host_str() != Some("open.spotify.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || segments.len() != 2
        || segments[0] != "playlist"
        || segments[1].len() != 22
        || !segments[1].bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err("Use https://open.spotify.com/playlist/ followed by the playlist ID.".into());
    }
    Ok(segments[1].into())
}

impl MetadataSession {
    async fn get(&self, path: &str) -> Result<serde_json::Value, String> {
        if Instant::now() >= self.expires {
            return Err("Spotify session expired. Connect again.".into());
        }
        let response = self
            .client
            .get(format!("https://api.spotify.com/v1{path}"))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|_| "Spotify metadata connection failed.")?;
        json_response(response).await
    }

    pub async fn playlist(&self, raw: &str) -> Result<SourcePlaylist, String> {
        let id = playlist_id(raw)?;
        let before = self.get(&format!("/playlists/{id}")).await?;
        let snapshot = before["snapshot_id"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or("Spotify playlist snapshot is missing.")?
            .to_owned();
        let title = before["name"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or("Spotify playlist title is missing.")?
            .to_owned();
        let mut tracks = Vec::new();
        let mut total = None;
        let mut offsets = HashSet::new();
        loop {
            let offset = tracks.len();
            if !offsets.insert(offset) {
                return Err("Spotify pagination did not advance.".into());
            }
            let page = self.get(&format!("/playlists/{id}/items?limit=50&offset={offset}")).await?;
            if append_page(&mut tracks, &mut total, &page)? {
                break;
            }
        }
        let after = self.get(&format!("/playlists/{id}")).await?;
        if after["snapshot_id"].as_str() != Some(snapshot.as_str()) {
            return Err("Spotify playlist changed while reading. No import was staged.".into());
        }
        Ok(SourcePlaylist { id, snapshot, title, tracks })
    }
}

fn append_page(
    tracks: &mut Vec<SourceTrack>,
    total: &mut Option<usize>,
    page: &serde_json::Value,
) -> Result<bool, String> {
    let count = page["total"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or("Spotify playlist total is missing or invalid.")?;
    validate_total(count)?;
    if page["offset"].as_u64() != Some(tracks.len() as u64)
        || total.is_some_and(|value| value != count)
    {
        return Err("Spotify playlist changed during pagination.".into());
    }
    let items = page["items"].as_array().ok_or("Spotify playlist items are missing.")?;
    let next = page.get("next").ok_or("Spotify pagination marker is missing.")?;
    if items.len() > 50 || (!next.is_null() && (!next.is_string() || items.is_empty())) {
        return Err("Invalid Spotify pagination.".into());
    }
    if tracks.len() + items.len() > count {
        return Err("Spotify returned too many occurrences.".into());
    }
    tracks.extend(items.iter().map(source_track));
    *total = Some(count);
    if next.is_null() && tracks.len() != count {
        return Err("Spotify playlist read is incomplete.".into());
    }
    Ok(next.is_null())
}

fn validate_total(count: usize) -> Result<(), String> {
    if count == 0 || count > MAX_PLAYLIST_OCCURRENCES {
        return Err(format!(
            "This experimental importer requires a complete playlist of 1 to {MAX_PLAYLIST_OCCURRENCES} entries."
        ));
    }
    Ok(())
}

fn source_track(entry: &serde_json::Value) -> SourceTrack {
    use crate::spotify_import::ImportOutcome;
    let item = &entry["item"];
    let source_title = item["name"].as_str().filter(|title| !title.trim().is_empty());
    let title = source_title.unwrap_or("Unavailable source entry").to_owned();
    let source_artists = item["artists"].as_array();
    let artists = item["artists"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|artist| artist["name"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let duration_ms = item["duration_ms"].as_u64().filter(|duration| *duration > 0);
    let outcome = if item.is_null() {
        ImportOutcome::SourceError
    } else if entry["is_local"] == true || item["is_local"] == true || item["type"] != "track" {
        ImportOutcome::Unsupported
    } else if source_title.is_none()
        || artists.is_empty()
        || artists.iter().any(|artist| artist.trim().is_empty())
        || source_artists.is_some_and(|values| values.len() != artists.len())
        || duration_ms.is_none()
    {
        ImportOutcome::SourceError
    } else {
        ImportOutcome::LowConfidence
    };
    SourceTrack { title, artists, duration_ms, outcome }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_playlist_limit_accepts_all_three_selected_sources_without_sampling() {
        for total in [33, 63, 117, MAX_PLAYLIST_OCCURRENCES] {
            assert!(validate_total(total).is_ok());
        }
        assert!(validate_total(0).is_err());
        assert!(validate_total(MAX_PLAYLIST_OCCURRENCES + 1).is_err());
    }

    #[test]
    fn pagination_preserves_all_117_occurrences_across_three_pages_including_repeats() {
        let mut tracks = Vec::new();
        let mut total = None;
        for (offset, length, last) in [(0, 50, false), (50, 50, false), (100, 17, true)] {
            let repeated = serde_json::json!({
                "item": {"name": "Repeated song", "type": "track",
                    "artists": [{"name": "Artist"}], "duration_ms": 180000}
            });
            let page = serde_json::json!({
                "total":117, "offset":offset, "items":vec![repeated; length],
                "next": if last { serde_json::Value::Null } else { serde_json::json!("unused-marker") }
            });
            assert_eq!(append_page(&mut tracks, &mut total, &page).unwrap(), last);
        }
        assert_eq!(tracks.len(), 117);
        assert_eq!(tracks.iter().filter(|track| track.title == "Repeated song").count(), 117);
    }

    #[test]
    fn pagination_rejects_incomplete_and_inconsistent_source_reads() {
        let mut tracks = vec![source_track(&serde_json::json!({"item":null})); 50];
        let mut total = Some(117);
        for page in [
            serde_json::json!({"total":117,"offset":50,"items":[],"next":null}),
            serde_json::json!({"total":118,"offset":50,"items":[],"next":"marker"}),
            serde_json::json!({"total":117,"offset":0,"items":[],"next":"marker"}),
            serde_json::json!({"total":117,"offset":50,"items":[]}),
        ] {
            assert!(append_page(&mut tracks, &mut total, &page).is_err());
        }
    }

    #[test]
    fn playlist_urls_reject_arbitrary_hosts_and_paths() {
        assert!(
            playlist_id("https://open.spotify.com/playlist/3zlBonH1l7BOTkoKITprOt?si=test").is_ok()
        );
        for raw in [
            "http://open.spotify.com/playlist/3zlBonH1l7BOTkoKITprOt",
            "https://evil.test/playlist/3zlBonH1l7BOTkoKITprOt",
            "https://open.spotify.com@evil.test/playlist/3zlBonH1l7BOTkoKITprOt",
            "spotify:playlist:3zlBonH1l7BOTkoKITprOt",
        ] {
            assert!(playlist_id(raw).is_err());
        }
    }

    #[test]
    fn callback_requires_exact_state_unique_parameters_and_expected_path() {
        assert_eq!(callback_code("/login?code=abc&state=expected", "expected").unwrap(), "abc");
        for target in [
            "/other?code=abc&state=expected",
            "/login?code=abc&state=foreign",
            "/login?code=a&code=b&state=expected",
            "/login?error=access_denied&state=expected",
            "/login?state=expected",
        ] {
            assert!(callback_code(target, "expected").is_err());
        }
    }

    #[test]
    fn client_id_comes_from_configuration_and_rejects_malformed_values() {
        assert_eq!(client_id(Some(" abc123 ".into())).unwrap(), "abc123");
        for value in [None, Some(String::new()), Some("a&b".into()), Some("x".repeat(65))] {
            assert!(client_id(value).is_err());
        }
    }

    #[test]
    fn null_and_unsupported_entries_remain_distinct() {
        use crate::spotify_import::ImportOutcome;
        assert_eq!(
            source_track(&serde_json::json!({"item":null})).outcome,
            ImportOutcome::SourceError
        );
        assert_eq!(
            source_track(&serde_json::json!({"item":{"type":"episode"}})).outcome,
            ImportOutcome::Unsupported
        );
    }

    #[test]
    fn missing_title_or_partial_artist_metadata_requires_skipping() {
        use crate::spotify_import::ImportOutcome;
        let mut entry = serde_json::json!({
            "item": {"type": "track", "duration_ms": 180000, "artists": [{"name": "Artist"}]}
        });
        assert_eq!(source_track(&entry).outcome, ImportOutcome::SourceError);
        entry["item"]["name"] = serde_json::json!("Song");
        entry["item"]["artists"] = serde_json::json!([{"name": "Artist"}, {}]);
        assert_eq!(source_track(&entry).outcome, ImportOutcome::SourceError);
    }
}
