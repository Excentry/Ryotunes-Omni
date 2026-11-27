//! The D3 safety-net extractor: rustypipe whole-videoId resolution. desktop policy §rustypipe.
//!
//! rustypipe is all-or-nothing per videoId — it runs its
//! own `/player` + cipher + PoToken internally. We cannot hand it a `signatureCipher` from our
//! own response. So it slots in at the videoId level only: "our direct clients all failed →
//! ask rustypipe to resolve the whole id." It must be able to carry the queue SOLO.

use rustypipe::client::RustyPipe;
use rustypipe::error::{Error as RpError, ExtractionError, UnavailabilityReason};
use rustypipe::model::AudioStream;

/// A resolved stream from the fallback. Mirrors what the orchestrator needs.
#[derive(Debug, Clone)]
pub struct StreamCandidate {
    pub url: String,
    pub itag: u32,
    pub mime: String,
    pub bitrate: u32,
    pub expires_in_seconds: u32,
    /// rustypipe's loudness value (inverse ReplayGain) used for playback gain.
    pub loudness_db: Option<f32>,
    pub title: Option<String>,
    pub duration_secs: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum FallbackError {
    #[error("temporary network failure: {0}")]
    Transient(String),
    #[error("age-restricted (rustypipe)")]
    AgeRestricted,
    #[error("unavailable: {0}")]
    Unavailable(String),
    #[error("no audio stream in rustypipe result")]
    NoAudio,
    #[error("rustypipe: {0}")]
    RustyPipe(String),
}

/// Resolve a videoId to its best audio stream via rustypipe. `prefer_high`: pick the
/// highest-bitrate opus/mp4a stream (matches our HIGH preference); else lowest ≤128k.
pub async fn resolve(video_id: &str, prefer_high: bool) -> Result<StreamCandidate, FallbackError> {
    // Keep rustypipe's cache out of the app CWD (it defaults to ./rustypipe_cache.json +
    // ./rustypipe_reports/, and an installed app's CWD may not be writable).
    let storage = std::env::temp_dir().join("ryotunes-rustypipe");
    std::fs::create_dir_all(&storage).ok();
    let player = RustyPipe::builder()
        .storage_dir(storage)
        .build()
        .map_err(|e| FallbackError::RustyPipe(e.to_string()))?
        .query()
        .player(video_id)
        .await
        .map_err(map_err)?;

    let best = pick_audio(&player.audio_streams, prefer_high).ok_or(FallbackError::NoAudio)?;
    Ok(StreamCandidate {
        url: best.url.clone(),
        itag: best.itag,
        mime: best.mime.clone(),
        bitrate: best.bitrate,
        expires_in_seconds: player.expires_in_seconds,
        loudness_db: best.loudness_db,
        title: player.details.name.clone(),
        duration_secs: Some(player.details.duration),
    })
}

fn pick_audio(streams: &[AudioStream], prefer_high: bool) -> Option<&AudioStream> {
    fn codec_score(mime: &str) -> u8 {
        if mime.contains("opus") {
            2
        } else if mime.contains("mp4a") {
            1
        } else {
            0
        }
    }
    if prefer_high {
        streams.iter().max_by(|a, b| {
            codec_score(&a.mime).cmp(&codec_score(&b.mime)).then(a.bitrate.cmp(&b.bitrate))
        })
    } else {
        let capped: Vec<&AudioStream> = streams.iter().filter(|s| s.bitrate <= 128_000).collect();
        if capped.is_empty() {
            streams.iter().min_by_key(|s| s.bitrate)
        } else {
            capped.into_iter().max_by_key(|s| s.bitrate)
        }
    }
}

fn map_err(e: RpError) -> FallbackError {
    match e {
        RpError::Http(_) => FallbackError::Transient(e.to_string()),
        RpError::HttpStatus(status, _) if is_transient_http_status(status) => {
            FallbackError::Transient(e.to_string())
        }
        RpError::Extraction(ExtractionError::Unavailable { reason, msg }) => match reason {
            UnavailabilityReason::AgeRestricted => FallbackError::AgeRestricted,
            _ => FallbackError::Unavailable(msg),
        },
        other => FallbackError::RustyPipe(other.to_string()),
    }
}

fn is_transient_http_status(status: u16) -> bool {
    status == 408 || status == 429 || (500..600).contains(&status)
}

#[cfg(test)]
mod tests {
    use super::{is_transient_http_status, map_err, FallbackError};
    use rustypipe::error::Error;
    use std::borrow::Cow;

    #[test]
    fn fallback_transport_errors_are_transient() {
        let error = map_err(Error::Http(Cow::Borrowed("connection failed")));
        assert!(matches!(error, FallbackError::Transient(_)));
    }

    #[test]
    fn fallback_retryable_statuses_are_transient() {
        for status in [408, 429, 500, 503, 599] {
            assert!(is_transient_http_status(status));
        }
        for status in [400, 403, 404] {
            assert!(!is_transient_http_status(status));
        }
    }
}
