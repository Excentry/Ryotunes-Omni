#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TidalCredentials {
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub device_secret: Option<String>,
    #[serde(default)]
    pub pkce_id: Option<String>,
    #[serde(default)]
    pub pkce_secret: Option<String>,
    #[serde(default)]
    pub pkce_redirect: Option<String>,
}

impl From<TidalCredentials> for ryotunes_tidal::Credentials {
    fn from(c: TidalCredentials) -> Self {
        fn pick(set: Option<String>, shipped: Option<String>) -> Option<String> {
            match set {
                Some(v) if !v.trim().is_empty() => Some(v),
                _ => shipped,
            }
        }
        let shipped = ryotunes_tidal::Credentials::default();
        ryotunes_tidal::Credentials {
            device_id: pick(c.device_id, shipped.device_id),
            device_secret: pick(c.device_secret, shipped.device_secret),
            pkce_id: pick(c.pkce_id, shipped.pkce_id),
            pkce_secret: pick(c.pkce_secret, shipped.pkce_secret),
            pkce_redirect: pick(c.pkce_redirect, shipped.pkce_redirect),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TidalCredentials;

    #[test]
    fn an_empty_stored_blob_keeps_the_shipped_pair() {
        let shipped = ryotunes_tidal::Credentials::default();
        let merged: ryotunes_tidal::Credentials = TidalCredentials::default().into();
        assert_eq!(merged, shipped, "a blank row must not disarm sign-in");
        let blanks = TidalCredentials {
            device_id: Some("   ".into()),
            device_secret: Some(String::new()),
            ..Default::default()
        };
        let merged: ryotunes_tidal::Credentials = blanks.into();
        assert_eq!(merged, shipped);
    }

    #[test]
    fn one_set_field_overrides_only_itself() {
        let merged: ryotunes_tidal::Credentials =
            TidalCredentials { device_id: Some("mine".into()), ..Default::default() }.into();
        assert_eq!(merged.device_id.as_deref(), Some("mine"));
        assert_eq!(merged.device_secret, ryotunes_tidal::Credentials::default().device_secret);
    }
}
