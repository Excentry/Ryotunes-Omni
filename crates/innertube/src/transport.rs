//! HTTP transport. write API. Pure — no Tauri/webview/mpv.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Serialize;
use sha1::{Digest, Sha1};

use crate::clients::YouTubeClient;
use crate::models::context::Locale;

pub const BASE_URL: &str = "https://music.youtube.com/youtubei/v1/";
pub const ORIGIN: &str = "https://music.youtube.com";
pub const REFERER: &str = "https://music.youtube.com/";
pub const SW_JS_DATA_URL: &str = "https://music.youtube.com/sw.js_data";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http: {0}")]
    Http(#[source] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("visitorData not found in sw.js_data")]
    VisitorDataNotFound,
    #[error("Your YouTube Music session expired — open the account menu and sign in again.")]
    SessionExpired,
    #[error("This track is already in the playlist.")]
    AlreadyInPlaylist,
    #[error(
        "YouTube Music only allows custom playlist art on accounts with a verified phone number."
    )]
    CoverRefused,
    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.without_url())
    }
}

/// Session state, set once at startup / login. write API §mutable session state.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub locale: Locale,
    pub visitor_data: Option<String>,
    pub data_sync_id: Option<String>,
    /// Full cookie string. When present, authenticated requests are available.
    pub cookie: Option<String>,
}

impl Session {
    /// Pull the `SAPISID` value out of the cookie string, if present.
    fn sapisid(&self) -> Option<String> {
        self.cookie.as_deref().and_then(cookie_sapisid).map(str::to_owned)
    }
}

/// Extract the `SAPISID` (or its modern `__Secure-3PAPISID` alias) value from a Cookie header
/// string. Public so the login flow (authentication flow) can validate a pasted cookie before setting it.
pub fn cookie_sapisid(cookie: &str) -> Option<&str> {
    cookie.split(';').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        matches!(k.trim(), "SAPISID" | "__Secure-3PAPISID").then(|| v.trim())
    })
}

/// The transport client. One shared `reqwest::Client`; proxy must be set before the
/// first request or reqwest snapshots it as none (desktop policy, the App.kt gotcha).
///
/// `session` is behind a shared lock: the app clones `InnerTube` into the orchestrator, and a
/// runtime login (authentication flow) must be visible through every clone. Reads/writes are quick and
/// never held across an `.await`, so a std `RwLock` is right (no async lock needed).
#[derive(Clone)]
pub struct InnerTube {
    http: reqwest::Client,
    session: Arc<RwLock<Session>>,
}

impl InnerTube {
    pub fn new(session: Session, proxy: Option<&str>) -> Result<Self, Error> {
        let mut builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(4);
        if let Some(p) = proxy {
            builder = builder.proxy(reqwest::Proxy::all(p)?);
        }
        Ok(InnerTube { http: builder.build()?, session: Arc::new(RwLock::new(session)) })
    }

    // --- session accessors (authentication flow) -----------------------------------------------------

    /// True when a login cookie is set.
    pub fn is_logged_in(&self) -> bool {
        self.session.read().unwrap().cookie.is_some()
    }

    /// The current visitorData (read fresh per resolve — a login may have refreshed it).
    pub fn visitor_data(&self) -> Option<String> {
        self.session.read().unwrap().visitor_data.clone()
    }

    /// The current cookie header, if logged in (for the stream-validation HEAD request).
    pub fn cookie(&self) -> Option<String> {
        self.session.read().unwrap().cookie.clone()
    }

    pub fn set_cookie(&self, cookie: Option<String>) {
        self.session.write().unwrap().cookie = cookie;
    }

    pub fn set_data_sync_id(&self, id: Option<String>) {
        self.session.write().unwrap().data_sync_id = id;
    }

    pub fn data_sync_id(&self) -> Option<String> {
        self.session.read().unwrap().data_sync_id.clone()
    }

    pub fn set_visitor_data(&self, vd: Option<String>) {
        self.session.write().unwrap().visitor_data = vd;
    }

    /// Build the request `context` for a client from the current session. Crate-internal — the
    /// endpoints facade calls it. Reads and drops the lock synchronously (no `.await` inside).
    pub(crate) fn context_for(&self, client: &YouTubeClient) -> crate::models::context::Context {
        let s = self.session.read().unwrap();
        // `onBehalfOfUser` makes Google *require* a credential: with no cookie it turns a request
        // that would have worked anonymously into a hard 401. Only send it when we can authenticate.
        let dsid = s.cookie.as_ref().and(s.data_sync_id.as_deref());
        client.to_context(&s.locale, s.visitor_data.as_deref(), dsid)
    }

    /// Build a one-off authenticated context for identity validation without changing the shared
    /// session seen by concurrent browse/playback requests. The caller commits the id only after
    /// the validation response succeeds.
    pub(crate) fn context_for_identity(
        &self,
        client: &YouTubeClient,
        data_sync_id: &str,
    ) -> crate::models::context::Context {
        let s = self.session.read().unwrap();
        let dsid = s.cookie.as_ref().map(|_| data_sync_id);
        client.to_context(&s.locale, s.visitor_data.as_deref(), dsid)
    }

    /// POST a JSON body to an InnerTube endpoint with this client's headers, retrying
    /// transient network errors (3 attempts, 500ms × 2 backoff). write API §retry.
    pub async fn post<B: Serialize>(
        &self,
        path: &str,
        client: &YouTubeClient,
        body: &B,
        set_login: bool,
    ) -> Result<serde_json::Value, Error> {
        // `path` may already carry query params (e.g. browse continuations); chain accordingly.
        let sep = if path.contains('?') { '&' } else { '?' };
        let url = format!("{BASE_URL}{path}{sep}prettyPrint=false");
        let headers = self.headers(client, set_login);
        let body = serde_json::to_vec(body)?;

        let mut delay = Duration::from_millis(500);
        let mut attempt = 0;
        loop {
            attempt += 1;
            let res = self
                .http
                .post(&url)
                .headers(headers.clone())
                .body(body.clone())
                .send()
                .await
                .and_then(|r| r.error_for_status());
            match res {
                Ok(resp) => return Ok(resp.json().await?),
                // Retry only on connect/timeout (transient), matching Metrolist's IOException filter.
                Err(e) if attempt < 3 && (e.is_timeout() || e.is_connect() || e.is_request()) => {
                    let safe_error = e.without_url();
                    tracing::warn!(attempt, error = %safe_error, "retrying InnerTube POST");
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
                // Signed in and Google says "no credential" (401) or "not for you" (403): the
                // stored cookie has gone stale. Raw reqwest text here reads as a broken app and
                // hands the user a URL instead of the one thing that fixes it.
                Err(e)
                    if self.is_logged_in() && e.status().is_some_and(|s| s == 401 || s == 403) =>
                {
                    tracing::warn!(status = ?e.status(), "InnerTube request rejected the session");
                    return Err(Error::SessionExpired);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// POST raw bytes to a path on the same origin that is *not* under `/youtubei`, with this
    /// client's headers plus `extra`, and hand back the response headers along with the body.
    ///
    /// Google's resumable uploader ("Scotty") lives on its own path and answers the first step in
    /// a header, so neither `post`'s URL shape nor its JSON-only return works here. The
    /// `content-type: application/json` the client headers carry stays put even when the body is
    /// an image: the uploader ignores it, and that is the shape known to work.
    pub(crate) async fn post_upload(
        &self,
        path: &str,
        client: &YouTubeClient,
        extra: &[(&'static str, String)],
        body: Vec<u8>,
    ) -> Result<(HeaderMap, Vec<u8>), Error> {
        let mut headers = self.headers(client, true);
        for (name, value) in extra {
            if let Ok(v) = HeaderValue::from_str(value) {
                headers.insert(HeaderName::from_static(name), v);
            }
        }
        // Explicitly, from the body we are about to send: reqwest omits `content-length` entirely
        // when the body is empty, and the uploader answers the empty "start" call with a bare
        // 411 Length Required. Sending it ourselves costs nothing on the calls that carry bytes.
        if let Ok(v) = HeaderValue::from_str(&body.len().to_string()) {
            headers.insert(reqwest::header::CONTENT_LENGTH, v);
        }
        let resp = self
            .http
            .post(format!("{ORIGIN}/{path}"))
            .headers(headers)
            .body(body)
            .send()
            .await?
            .error_for_status()?;
        let headers = resp.headers().clone();
        Ok((headers, resp.bytes().await?.to_vec()))
    }

    /// Per-request headers. write API §ytClient. Note `X-YouTube-Client-Name` carries the
    /// numeric client **id**, not the name string — intentional and required.
    fn headers(&self, client: &YouTubeClient, set_login: bool) -> HeaderMap {
        let mut h = HeaderMap::new();
        let set = |h: &mut HeaderMap, k: &'static str, v: &str| {
            if let Ok(val) = HeaderValue::from_str(v) {
                h.insert(HeaderName::from_static(k), val);
            }
        };
        set(&mut h, "content-type", "application/json");
        set(&mut h, "accept", "application/json");
        set(&mut h, "accept-language", "en-US,en;q=0.9");
        set(&mut h, "x-goog-api-format-version", "1");
        set(&mut h, "x-youtube-client-name", &client.client_id);
        set(&mut h, "x-youtube-client-version", &client.client_version);
        set(&mut h, "x-origin", ORIGIN);
        set(&mut h, "referer", REFERER);
        set(&mut h, "user-agent", &client.user_agent);

        let s = self.session.read().unwrap();
        if let Some(vd) = &s.visitor_data {
            set(&mut h, "x-goog-visitor-id", vd);
        }

        // SAPISIDHASH cookie auth is used only when logged in and supported by the client.
        if set_login && client.login_supported {
            if let Some(cookie) = &s.cookie {
                set(&mut h, "cookie", cookie);
                if let Some(sapisid) = s.sapisid() {
                    if let Ok(val) = HeaderValue::from_str(&sapisid_hash(&sapisid, ORIGIN)) {
                        h.insert(HeaderName::from_static("authorization"), val);
                    }
                }
            }
        }
        h
    }

    /// Bootstrap `visitorData` anonymously by scraping `sw.js_data`. PoToken flow §A.
    pub async fn fetch_visitor_data(&self) -> Result<String, Error> {
        let text = self.http.get(SW_JS_DATA_URL).send().await?.error_for_status()?.text().await?;
        parse_visitor_data(&text)
    }

    /// Register a play in watch history: GET the response's
    /// `playbackTracking.videostatsPlaybackUrl.baseUrl` with `c`/`cpn`/`ver` (+ `list`/`referrer`
    /// in a playlist) and the authed client headers. write API §registerPlayback. Best-effort —
    /// the caller logs-and-ignores errors.
    pub async fn register_playback(
        &self,
        client: &YouTubeClient,
        base_url: &str,
        cpn: &str,
        playlist_id: Option<&str>,
    ) -> Result<(), Error> {
        let url = build_playback_url(base_url, &client.client_name, cpn, playlist_id);
        let headers = self.headers(client, true);
        self.http.get(&url).headers(headers).send().await?.error_for_status()?;
        Ok(())
    }

    #[cfg(any(test, feature = "integration-tests"))]
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }
}

/// Build the playback-tracking GET URL. write API §registerPlayback. Pure — unit-tested. The
/// `base_url` already carries YouTube's own query params, so we chain onto it.
fn build_playback_url(
    base_url: &str,
    client_name: &str,
    cpn: &str,
    playlist_id: Option<&str>,
) -> String {
    let sep = if base_url.contains('?') { '&' } else { '?' };
    let mut url = format!(
        "{base_url}{sep}c={}&cpn={}&ver=2",
        urlencoding::encode(client_name),
        urlencoding::encode(cpn),
    );
    if let Some(list) = playlist_id {
        let enc = urlencoding::encode(list);
        url.push_str(&format!("&list={enc}&referrer={enc}"));
    }
    url
}

/// CPN alphabet — 64 URL-safe chars, exactly 6 bits each. write API.
const CPN_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// A fresh 16-char Content Playback Nonce for one playback. write API §registerPlayback.
// Note: time+counter-seeded xorshift, not crypto-rand — a CPN only needs to be unique per
// playback, not unpredictable; keeps the `rand` crate out of the tree.
pub fn generate_cpn() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let bump = COUNTER.fetch_add(1, Ordering::Relaxed).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut state = (nanos ^ bump).wrapping_add(0x1234_567);
    if state == 0 {
        state = 0xDEAD_BEEF;
    }
    let mut out = String::with_capacity(16);
    for _ in 0..16 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push(CPN_CHARS[(state & 63) as usize] as char);
    }
    out
}

/// `Authorization: SAPISIDHASH <epoch>_<sha1(epoch SAPISID origin)>`. write API.
pub fn sapisid_hash(sapisid: &str, origin: &str) -> String {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("SAPISIDHASH {epoch}_{}", sha1_hex(&format!("{epoch} {sapisid} {origin}")))
}

fn sha1_hex(input: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// The `sw.js_data` body starts with a 4–5 char junk prefix (`)]}'`); strip it, parse JSON,
/// and find the element matching `^Cg[ts]` in `[0][2]`. PoToken flow §A.
fn parse_visitor_data(body: &str) -> Result<String, Error> {
    // Drop everything up to and including the first newline or the `)]}'` guard.
    let json_start = body.find('[').ok_or(Error::VisitorDataNotFound)?;
    let value: serde_json::Value = serde_json::from_str(&body[json_start..])?;
    let arr = value
        .get(0)
        .and_then(|v| v.get(2))
        .and_then(|v| v.as_array())
        .ok_or(Error::VisitorDataNotFound)?;
    arr.iter()
        .filter_map(|v| v.as_str())
        .find(|s| s.starts_with("Cgt") || s.starts_with("Cgs"))
        .map(str::to_owned)
        .ok_or(Error::VisitorDataNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Read, Write};
    use std::sync::Mutex;

    #[derive(Clone)]
    struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedLogs {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLogs {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn sha1_known_vector() {
        // SHA1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
        assert_eq!(sha1_hex("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn sapisid_hash_shape() {
        let h = sapisid_hash("MYSAPISID", ORIGIN);
        assert!(h.starts_with("SAPISIDHASH "));
        let rest = &h["SAPISIDHASH ".len()..];
        let (epoch, hash) = rest.split_once('_').unwrap();
        assert!(epoch.parse::<u64>().is_ok());
        assert_eq!(hash.len(), 40); // sha1 hex
    }

    #[test]
    fn parse_visitor_data_from_blob() {
        // Shape of sw.js_data: outer array; [0][2] holds the visitorData among other strings.
        let blob = r#")]}'
[["wrs","x",["junk","CgtABCDEFG1234567%3D%3D","more"]],null]"#;
        assert_eq!(parse_visitor_data(blob).unwrap(), "CgtABCDEFG1234567%3D%3D");
    }

    #[test]
    fn playback_url_appends_params() {
        // Base URL already has query params → chained with `&`; playlist adds list+referrer.
        let u = build_playback_url(
            "https://s.youtube.com/api/stats/playback?cl=1&docid=abc",
            "WEB_REMIX",
            "CPN1234567890AB",
            Some("RDAMVMxyz"),
        );
        assert!(u.contains("?cl=1&docid=abc&c=WEB_REMIX&cpn=CPN1234567890AB&ver=2"));
        assert!(u.contains("&list=RDAMVMxyz&referrer=RDAMVMxyz"));
        // No existing query → first param uses `?`, no playlist params.
        let u2 = build_playback_url("https://s.youtube.com/x", "IOS", "abc", None);
        assert_eq!(u2, "https://s.youtube.com/x?c=IOS&cpn=abc&ver=2");
    }

    #[test]
    fn cpn_is_16_url_safe_chars() {
        let cpn = generate_cpn();
        assert_eq!(cpn.len(), 16);
        assert!(cpn.bytes().all(|b| CPN_CHARS.contains(&b)));
        // Two calls in quick succession must differ (counter salt).
        assert_ne!(generate_cpn(), generate_cpn());
    }

    #[test]
    fn on_behalf_of_user_needs_a_cookie() {
        let clients = crate::clients::Clients::bundled();
        let web = clients.get(crate::clients::METADATA_CLIENT).unwrap();
        let session = Session { data_sync_id: Some("abc123".into()), ..Default::default() };

        let it = InnerTube::new(session, None).unwrap();
        assert_eq!(it.context_for(web).user.on_behalf_of_user, None, "no cookie ⇒ no obo (401)");

        it.set_cookie(Some("SAPISID=secret".into()));
        assert_eq!(it.context_for(web).user.on_behalf_of_user.as_deref(), Some("abc123"));
    }

    #[test]
    fn identity_validation_context_does_not_mutate_the_committed_session() {
        let clients = crate::clients::Clients::bundled();
        let web = clients.get(crate::clients::METADATA_CLIENT).unwrap();
        let session = Session {
            cookie: Some("SAPISID=secret".into()),
            data_sync_id: Some("committed-id".into()),
            ..Default::default()
        };
        let it = InnerTube::new(session, None).unwrap();

        assert_eq!(
            it.context_for_identity(web, "candidate-id").user.on_behalf_of_user.as_deref(),
            Some("candidate-id")
        );
        assert_eq!(it.context_for(web).user.on_behalf_of_user.as_deref(), Some("committed-id"));
    }

    #[test]
    fn sapisid_extracted_from_cookie() {
        let s = Session {
            cookie: Some("FOO=bar; SAPISID=secret123; OTHER=x".into()),
            ..Default::default()
        };
        assert_eq!(s.sapisid().as_deref(), Some("secret123"));
    }

    #[tokio::test]
    async fn http_error_display_redacts_request_urls_and_queries() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .unwrap();
        });
        let request_url = format!(
            "http://{address}/youtubei/v1/browse?key=sensitive-key&continuation=sensitive-token"
        );
        let response = reqwest::Client::new().get(&request_url).send().await.unwrap();
        let error = response.error_for_status().unwrap_err();
        server.join().unwrap();

        let error = Error::from(error);
        let Error::Http(source) = &error else { panic!("expected an HTTP error") };
        assert_eq!(source.status().unwrap().as_u16(), 503);
        let display = error.to_string();
        assert!(display.contains("503"));
        for secret in ["http://", "/youtubei/v1/browse", "sensitive-key", "sensitive-token"] {
            assert!(!display.contains(secret), "leaked {secret}: {display}");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn retry_logs_do_not_include_continuation_tokens() {
        const TOKEN: &str = "continuation-token-canary";

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(8);
            let mut requests = 0;
            while requests < 3 && std::time::Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        requests += 1;
                        stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                        let mut request = [0; 2048];
                        let _ = stream.read(&mut request);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("proxy accept failed: {error}"),
                }
            }
            requests
        });
        let log_bytes = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(CapturedLogs(log_bytes.clone()))
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        let proxy = format!("http://{address}");
        let inner_tube = InnerTube::new(Session::default(), Some(&proxy)).unwrap();
        let clients = crate::clients::Clients::bundled();
        let client = clients.get(crate::clients::METADATA_CLIENT).unwrap();
        let path = format!("browse?ctoken={TOKEN}&continuation={TOKEN}");
        let result = inner_tube.post(&path, client, &serde_json::json!({}), false).await;
        let requests = server.join().unwrap();
        drop(_guard);

        assert!(result.is_err());
        assert!(requests > 0);
        let logs = String::from_utf8(log_bytes.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("retrying InnerTube POST"), "missing retry log: {logs}");
        assert!(!logs.contains(TOKEN), "continuation token leaked into logs: {logs}");
    }
}
