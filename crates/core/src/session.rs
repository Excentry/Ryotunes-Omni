//! Login session logic shared across hosts: the navigation allow-list the interactive sign-in
//! window enforces, and the orchestration that applies the host's [`LoginResult`].
//!
//! The visible Google sign-in window itself lives in the host (`login_webview.rs`), because it
//! needs a real browser surface; it hands its harvested cookies back through
//! [`crate::host::LoginFlow`] and this module applies them through the same cookie path as before.

use std::sync::Arc;

use crate::host::{LoginError, LoginResult};
use crate::state::{AppState, SignInOutcome};

/// Google sign-in with `continue` back to YTM, so a successful login redirects to
/// music.youtube.com (the host's completion signal). Kept here so every host opens the same URL.
pub const LOGIN_URL: &str =
    "https://accounts.google.com/ServiceLogin?service=youtube&continue=https://music.youtube.com/";

const GOOGLE_REGIONAL_DOMAINS: &str = include_str!("google_login_domains.txt");

fn is_regional_google_login_host(host: &str) -> bool {
    let Some(domain) = host.strip_prefix("accounts.").or_else(|| host.strip_prefix("www.")) else {
        return false;
    };
    GOOGLE_REGIONAL_DOMAINS.lines().any(|supported| supported == domain)
}

/// The navigation allow-list the login window enforces: only https on Google/YouTube. Shared with
/// the host so a phishing redirect can never keep the sign-in window.
pub fn allowed_login_navigation(url: &url::Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else { return false };
    host == "google.com"
        || host.ends_with(".google.com")
        || is_regional_google_login_host(host)
        || host == "youtube.com"
        || host.ends_with(".youtube.com")
}

impl AppState {
    /// Run the host's interactive sign-in and apply the result exactly as the old login webview
    /// did: the host harvests the cookies, then they flow through the same cookie path, emitting
    /// `login-done` / `account-selection-required` / `login-error`. Sign-in completes
    /// asynchronously; the UI learns via those events.
    pub async fn sign_in(self: &Arc<Self>) {
        match self.login.sign_in().await {
            Ok(result) => self.apply_login(result).await,
            // A closed window is a silent no-op, matching the old fire-and-forget flow.
            Err(LoginError::Cancelled) => {}
            Err(e) => self.emit("login-error", e.to_string()),
        }
    }

    async fn apply_login(&self, result: LoginResult) {
        // The host hands back the `.youtube.com` jar as `name=value` pairs; rebuild the Cookie
        // header the cookie path expects (sorted, so it is byte-identical to the old flow).
        let header =
            result.cookies.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ");
        match self.apply_cookie(header).await {
            Ok(SignInOutcome::Complete) => self.emit("login-done", ()),
            // The authenticated cookie is saved, but the account stays deliberately unfinished
            // until the main-window picker selects a server-issued delegated identity.
            Ok(SignInOutcome::SelectionRequired) => {}
            Err(e) => self.emit("login-error", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_navigation_stays_on_google_and_youtube_https() {
        assert!(allowed_login_navigation(
            &url::Url::parse("https://accounts.google.com/ServiceLogin").unwrap()
        ));
        assert!(allowed_login_navigation(&url::Url::parse("https://music.youtube.com/").unwrap()));
        assert!(!allowed_login_navigation(
            &url::Url::parse("http://accounts.google.com/").unwrap()
        ));
        assert!(!allowed_login_navigation(&url::Url::parse("https://example.com/phish").unwrap()));
    }

    #[test]
    fn login_navigation_accepts_published_google_regional_hosts() {
        for value in [
            "https://accounts.google.com.co/accounts",
            "https://accounts.google.co.uk/accounts",
            "https://www.google.co.jp/",
        ] {
            assert!(allowed_login_navigation(&url::Url::parse(value).unwrap()), "{value}");
        }
    }

    #[test]
    fn login_navigation_rejects_google_regional_lookalikes() {
        for value in [
            "http://accounts.google.com.co/accounts",
            "https://google.com.co.evil.example/",
            "https://evilgoogle.com.co/",
            "https://accounts.google.com.co.example/",
            "https://evil.accounts.google.com.co/",
            "https://accounts.google.com.xx/",
        ] {
            assert!(!allowed_login_navigation(&url::Url::parse(value).unwrap()), "{value}");
        }
    }
}
