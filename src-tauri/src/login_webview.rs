//! The visible Google sign-in window (authentication flow Path A). Implements the core's
//! [`LoginFlow`]: it opens a real browser surface with a spoofed desktop UA, watches for the
//! redirect back to music.youtube.com, harvests the youtube cookies, and hands them to the core,
//! which applies them through the same cookie path as before.
//!
//! Persistent (non-incognito) on purpose: the webview keeps its own Google session, so a later
//! re-login is one click with no password/paste — the real fix for KI-2 (cookie staleness), where
//! Google's short-lived `__Secure-*SIDTS` cookies rotate and a pasted cookie eventually stops
//! authenticating.

use std::future::Future;
use std::time::Duration;

use tauri::webview::cookie::Cookie;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use ryotunes_core::host::{LoginError, LoginFlow, LoginResult};
use ryotunes_core::session::{allowed_login_navigation, LOGIN_URL};

const LOGIN_LABEL: &str = "login";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Keep the WebKit-compatible UA for WebKit hosts. Windows WebView2 must use its native default UA.
#[cfg(not(windows))]
const LOGIN_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
                        (KHTML, like Gecko) Version/17.4.1 Safari/605.1.15";

fn login_user_agent_override() -> Option<&'static str> {
    #[cfg(windows)]
    {
        None
    }
    #[cfg(not(windows))]
    {
        Some(LOGIN_UA)
    }
}

fn guard_login_navigation(
    url: &tauri::Url,
    navigation_tx: &tokio::sync::mpsc::UnboundedSender<LoginEvent>,
) -> bool {
    let allowed = allowed_login_navigation(url);
    if !allowed {
        tracing::warn!(
            scheme = url.scheme(),
            host = url.host_str().unwrap_or_default(),
            "login navigation rejected"
        );
        let _ = navigation_tx.send(LoginEvent::NavigationRejected);
    }
    allowed
}

/// What the login window reports back to the async sign-in.
enum LoginEvent {
    Landed,
    NavigationRejected,
    BuildFailed(String),
}

#[derive(Debug, PartialEq, Eq)]
enum CookieReadError {
    Failed,
    TimedOut,
}

/// The host's [`LoginFlow`]: one visible Google sign-in window per call.
pub struct TauriLogin {
    pub app: AppHandle,
}

enum AttemptError {
    NavigationRejected,
    Failed(LoginError),
}

const MAX_NAVIGATION_RETRIES: u8 = 1;
const NAVIGATION_REJECTED_MESSAGE: &str =
    "Sign-in redirected outside Google or YouTube. Please try again.";

fn retry_after_rejection(retries_used: u8) -> bool {
    retries_used < MAX_NAVIGATION_RETRIES
}

#[async_trait::async_trait]
impl LoginFlow for TauriLogin {
    async fn sign_in(&self) -> Result<LoginResult, LoginError> {
        let deadline = tokio::time::Instant::now() + LOGIN_TIMEOUT;
        let mut retries_used = 0;
        loop {
            match self.run_attempt(deadline).await {
                Ok(result) => return Ok(result),
                Err(AttemptError::Failed(error)) => return Err(error),
                Err(AttemptError::NavigationRejected) if retry_after_rejection(retries_used) => {
                    retries_used += 1;
                }
                Err(AttemptError::NavigationRejected) => {
                    return Err(LoginError::Failed(NAVIGATION_REJECTED_MESSAGE.into()));
                }
            }
        }
    }
}

impl TauriLogin {
    async fn run_attempt(
        &self,
        deadline: tokio::time::Instant,
    ) -> Result<LoginResult, AttemptError> {
        let app = self.app.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<LoginEvent>();
        let build_tx = tx.clone();

        // Window creation must happen on the main thread (GTK).
        let app2 = app.clone();
        let dispatched = app.run_on_main_thread(move || {
            // Reclaim the label if a prior login window is still around.
            if let Some(w) = app2.get_webview_window(LOGIN_LABEL) {
                let _ = w.destroy();
            }
            let Ok(url) = tauri::Url::parse(LOGIN_URL) else {
                let _ = build_tx.send(LoginEvent::BuildFailed("bad login url".into()));
                return;
            };
            let mut builder =
                WebviewWindowBuilder::new(&app2, LOGIN_LABEL, WebviewUrl::External(url))
                    .title("Sign in to YouTube Music")
                    .inner_size(480.0, 720.0);
            if let Some(user_agent) = login_user_agent_override() {
                builder = builder.user_agent(user_agent);
            }
            let navigation_tx = tx.clone();
            let res = builder
                .on_navigation(move |url| guard_login_navigation(url, &navigation_tx))
                .on_page_load(move |_w, payload| {
                    if matches!(payload.event(), PageLoadEvent::Finished)
                        && payload.url().host_str() == Some("music.youtube.com")
                    {
                        let _ = tx.send(LoginEvent::Landed);
                    }
                })
                .build();
            if let Err(e) = res {
                let _ = build_tx.send(LoginEvent::BuildFailed(format!(
                    "Couldn't open the sign-in window: {e}"
                )));
            }
        });
        if dispatched.is_err() {
            return Err(AttemptError::Failed(LoginError::Failed(
                "Couldn't open the sign-in window".into(),
            )));
        }

        loop {
            let ev = match recv_login_event(&mut rx, deadline).await {
                Ok(Some(ev)) => ev,
                Ok(None) => break,
                Err(()) => {
                    close_login(&app);
                    return Err(AttemptError::Failed(LoginError::Failed(
                        "Sign-in timed out. Please try again.".into(),
                    )));
                }
            };
            match ev {
                LoginEvent::BuildFailed(e) => {
                    close_login(&app);
                    return Err(AttemptError::Failed(LoginError::Failed(e)));
                }
                LoginEvent::NavigationRejected => {
                    close_login(&app);
                    return Err(AttemptError::NavigationRejected);
                }
                LoginEvent::Landed => {
                    // The redirect that lands us here sets the youtube cookies; they may appear a
                    // beat after the page finishes, so poll briefly.
                    for _ in 0..6 {
                        let cookies = match read_login_cookies_until(
                            deadline,
                            read_login_cookies(&app),
                        )
                        .await
                        {
                            Ok(cookies) => cookies,
                            Err(CookieReadError::Failed) => {
                                close_login(&app);
                                return Err(AttemptError::Failed(LoginError::Failed(
                                    "Couldn't read sign-in cookies. Please try again.".into(),
                                )));
                            }
                            Err(CookieReadError::TimedOut) => {
                                close_login(&app);
                                return Err(AttemptError::Failed(LoginError::Failed(
                                    "Sign-in timed out. Please try again.".into(),
                                )));
                            }
                        };
                        if innertube::cookie_sapisid(&cookie_header(&cookies)).is_some() {
                            close_login(&app);
                            return Ok(LoginResult { cookies, authuser: 0 });
                        }
                        if tokio::time::timeout_at(
                            deadline,
                            tokio::time::sleep(Duration::from_millis(500)),
                        )
                        .await
                        .is_err()
                        {
                            close_login(&app);
                            return Err(AttemptError::Failed(LoginError::Failed(
                                "Sign-in timed out. Please try again.".into(),
                            )));
                        }
                    }
                    // Landed on music.youtube.com but not authenticated yet — keep watching.
                }
            }
        }
        // The window closed before authentication completed.
        close_login(&app);
        Err(AttemptError::Failed(LoginError::Cancelled))
    }
}

async fn recv_login_event(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<LoginEvent>,
    deadline: tokio::time::Instant,
) -> Result<Option<LoginEvent>, ()> {
    tokio::time::timeout_at(deadline, rx.recv()).await.map_err(|_| ())
}

async fn read_login_cookies_until<F>(
    deadline: tokio::time::Instant,
    read: F,
) -> Result<Vec<(String, String)>, CookieReadError>
where
    F: Future<Output = Result<Vec<(String, String)>, ()>>,
{
    tokio::time::timeout_at(deadline, read)
        .await
        .map_err(|_| CookieReadError::TimedOut)?
        .map_err(|_| CookieReadError::Failed)
}

/// Read the youtube-domain cookies from the login window as `name=value` pairs. Reads the platform
/// cookie store (HttpOnly + secure included), matching what a browser sends to music.youtube.com.
///
/// Hops to the main thread: both backends drive their platform event loop while they wait for the
/// store (`gtk::main_iteration` on WebKitGTK, `NSRunLoop::mainRunLoop` on WKWebView), so they are
/// written to be called from the thread that owns it.
async fn read_login_cookies(app: &AppHandle) -> Result<Vec<(String, String)>, ()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let app2 = app.clone();
    if app
        .run_on_main_thread(move || {
            let _ = tx.send(youtube_cookies(&app2));
        })
        .is_err()
    {
        return Err(());
    }
    rx.await.map_err(|_| ())?
}

fn youtube_cookies(app: &AppHandle) -> Result<Vec<(String, String)>, ()> {
    let wv = app.get_webview_window(LOGIN_LABEL).ok_or(())?;
    let cookies = wv.cookies().map_err(|_| ())?;
    Ok(youtube_cookie_pairs(cookies))
}

/// Domain-match by hand rather than with `cookies_for_url`: WKWebView's implementation compares the
/// cookie's domain to the URL's host with `==`, so YouTube's `.youtube.com` cookies never match
/// music.youtube.com and macOS got an empty jar (no SAPISID, so sign-in gave up silently).
/// WebKitGTK matches domains properly, which is why Linux never saw it.
///
/// Anything outside youtube.com is dropped, google.com cookies included: this becomes a `Cookie`
/// header sent to YouTube, and a cookie without a domain we recognise doesn't belong in it.
fn youtube_cookie_pairs(mut cookies: Vec<Cookie<'static>>) -> Vec<(String, String)> {
    // `Cookie::domain()` has already stripped the leading dot. Sorting means the most specific
    // domain is inserted last and so wins a name collision, the way a browser resolves one.
    cookies.sort_by_key(|c| c.domain().unwrap_or_default().len());
    let mut jar = std::collections::BTreeMap::new();
    for c in cookies {
        let domain = c.domain().unwrap_or_default();
        if domain == "youtube.com" || domain.ends_with(".youtube.com") {
            jar.insert(c.name().to_string(), c.value().to_string());
        }
    }
    jar.into_iter().collect()
}

/// The `Cookie` header a browser would send: `name=value` joined with `; `.
fn cookie_header(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ")
}

fn close_login(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = app2.get_webview_window(LOGIN_LABEL) {
            let _ = w.destroy();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_user_agent_override_matches_webview_platform() {
        #[cfg(windows)]
        assert_eq!(login_user_agent_override(), None);
        #[cfg(not(windows))]
        assert_eq!(login_user_agent_override(), Some(LOGIN_UA));
    }

    #[test]
    fn blocked_login_navigation_notifies_sign_in_flow() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let url = tauri::Url::parse("https://example.com/login").unwrap();

        assert!(!guard_login_navigation(&url, &tx));
        assert!(matches!(rx.try_recv(), Ok(LoginEvent::NavigationRejected)));
    }

    #[test]
    fn rejected_navigation_is_retried_once_then_reported() {
        assert!(retry_after_rejection(0));
        assert!(!retry_after_rejection(MAX_NAVIGATION_RETRIES));
    }

    fn cookie(s: &str) -> Cookie<'static> {
        Cookie::parse(s.to_string()).unwrap()
    }

    #[test]
    fn keeps_the_youtube_jar_and_drops_everything_else() {
        // `.youtube.com` is where the auth cookies actually live, and the domain WKWebView refuses
        // to match against music.youtube.com.
        let header = cookie_header(&youtube_cookie_pairs(vec![
            cookie("SAPISID=abc; Domain=.youtube.com"),
            cookie("SID=def; Domain=.youtube.com"),
            cookie("VISITOR_INFO1_LIVE=xyz; Domain=music.youtube.com"),
            cookie("SAPISID=notthisone; Domain=.google.com"),
            cookie("nodomain=1"),
        ]));
        assert_eq!(header, "SAPISID=abc; SID=def; VISITOR_INFO1_LIVE=xyz");
        // The check the harvest gates on: no SAPISID means sign-in silently gives up.
        assert_eq!(innertube::cookie_sapisid(&header), Some("abc"));
    }

    #[test]
    fn the_most_specific_domain_wins_a_name_collision() {
        let header = cookie_header(&youtube_cookie_pairs(vec![
            cookie("PREF=broad; Domain=.youtube.com"),
            cookie("PREF=specific; Domain=music.youtube.com"),
        ]));
        assert_eq!(header, "PREF=specific");
    }

    #[tokio::test]
    async fn login_event_wait_times_out() {
        let (_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(10);
        assert!(matches!(recv_login_event(&mut rx, deadline).await, Err(())));
    }

    #[tokio::test]
    async fn closing_login_window_ends_the_event_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        drop(tx);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        assert!(matches!(recv_login_event(&mut rx, deadline).await, Ok(None)));
    }

    #[tokio::test]
    async fn login_event_arrives_before_deadline() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        assert!(tx.send(LoginEvent::Landed).is_ok());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        assert!(matches!(recv_login_event(&mut rx, deadline).await, Ok(Some(LoginEvent::Landed))));
    }

    #[tokio::test]
    async fn cookie_read_completes_before_deadline() {
        let cookies = vec![("SAPISID".into(), "fixture".into())];
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        assert_eq!(
            read_login_cookies_until(deadline, async { Ok(cookies.clone()) }).await,
            Ok(cookies)
        );
    }

    #[tokio::test]
    async fn cookie_read_failure_is_recoverable() {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        assert_eq!(
            read_login_cookies_until(deadline, async { Err(()) }).await,
            Err(CookieReadError::Failed)
        );
    }

    #[tokio::test]
    async fn stalled_cookie_read_obeys_login_deadline() {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(10);
        assert_eq!(
            read_login_cookies_until(deadline, std::future::pending()).await,
            Err(CookieReadError::TimedOut)
        );
    }
}
