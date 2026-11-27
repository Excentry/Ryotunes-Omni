use std::sync::atomic::Ordering;

use innertube::SongItem;

use crate::db::now_secs;
use crate::media_ref::MediaRef;

use super::{parse_duration_ms, AppState, ON_REPEAT_WINDOW_SECS};

impl AppState {
    pub fn note_position_sample(&self, pos: f64) {
        if pos.is_finite() {
            self.latest_position.store(pos.to_bits(), Ordering::Relaxed);
        }
    }

    pub async fn on_position(&self, pos: f64) {
        self.record_position(pos);
        let crossed = {
            let mut q = self.queue.lock().await;
            if q.history_pinged {
                None
            } else {
                let threshold = if q.duration > 1.0 { (q.duration / 2.0).min(30.0) } else { 30.0 };
                if pos >= threshold {
                    q.history_pinged = true;
                    Some((q.playback_ping.clone(), q.cpn.clone(), q.items.get(q.current).cloned()))
                } else {
                    None
                }
            }
        };
        let Some((ping, cpn, played)) = crossed else { return };
        if let Some(item) = played.filter(|i| {
            !matches!(
                MediaRef::parse(&i.video_id),
                MediaRef::Local(_)
                    | MediaRef::Radio(_)
                    | MediaRef::SpotifyTrack(_)
                    | MediaRef::SpotifyOther(_)
                    | MediaRef::SpotifyArtist(_)
                    | MediaRef::SpotifyAlbum(_)
                    | MediaRef::SpotifyPlaylist(_)
                    | MediaRef::SoundCloudTrack(_)
                    | MediaRef::SoundCloudUser(_)
                    | MediaRef::SoundCloudPlaylist(_)
                    | MediaRef::SoundCloudSystem(_)
                    | MediaRef::SoundCloudOther(_)
            )
        }) {
            if let Ok(json) = serde_json::to_string(&item) {
                self.db.record_play(&item.video_id, &json, now_secs(), ON_REPEAT_WINDOW_SECS);
            }
        }
        if !self.history_enabled() || !self.it.is_logged_in() {
            return;
        }
        let Some(ping) = ping else {
            tracing::info!(
                "no watch-history ping for this play (no client returned a tracking URL)"
            );
            return;
        };
        let Some(client) = self.clients.get(&ping.client).cloned() else {
            tracing::warn!(client = %ping.client, "watch-history ping: unknown client");
            return;
        };
        let it = self.it.clone();
        tokio::spawn(async move {
            match it.register_playback(&client, &ping.url, &cpn, None).await {
                Ok(()) => tracing::debug!(client = %ping.client, "watch-history ping sent"),
                Err(e) => tracing::warn!(error = %e, "watch-history ping failed"),
            }
        });
    }

    pub async fn on_duration(&self, secs: f64) {
        if !(secs.is_finite() && secs > 0.0) {
            return;
        }
        {
            let mut q = self.queue.lock().await;
            let spotify = q
                .items
                .get(q.current)
                .is_some_and(|i| crate::spotify::streams_via_fifo(&i.video_id));
            if spotify {
                return;
            }
            q.duration = secs;
        }
        if let Some(m) = &self.media {
            m.set_duration(secs);
        }
        if let Some(d) = &self.discord {
            d.set_duration(secs);
        }
        self.lastfm.set_duration(secs);
    }
    pub(super) async fn seed_spotify_duration(&self, item: &SongItem) {
        let secs = parse_duration_ms(item.duration.as_deref()) as f64 / 1000.0;
        if secs <= 0.0 {
            return;
        }
        self.queue.lock().await.duration = secs;
        if let Some(m) = &self.media {
            m.set_duration(secs);
        }
        if let Some(d) = &self.discord {
            d.set_duration(secs);
        }
        self.lastfm.set_duration(secs);
    }

    fn history_enabled(&self) -> bool {
        self.db.get_setting("enable_history").map(|v| v != "false").unwrap_or(true)
    }

    fn record_position(&self, pos: f64) {
        self.latest_position.store(pos.to_bits(), Ordering::Relaxed);
        let now = now_secs() as u64;
        if now.saturating_sub(self.last_pos_persist.load(Ordering::Relaxed)) >= 5 {
            self.last_pos_persist.store(now, Ordering::Relaxed);
            self.db.set_setting("queue_position", &pos.to_string());
        }
        if now.saturating_sub(self.last_media_push.load(Ordering::Relaxed)) >= 1 {
            self.last_media_push.store(now, Ordering::Relaxed);
            let playing = self.is_playing.load(Ordering::Relaxed);
            if let Some(m) = &self.media {
                m.set_playback(playing, pos);
            }
            if let Some(d) = &self.discord {
                d.set_position(pos);
            }
            self.lastfm.set_position(pos);
        }
    }

    pub fn flush_position(&self) {
        let pos = f64::from_bits(self.latest_position.load(Ordering::Relaxed));
        self.db.set_setting("queue_position", &pos.to_string());
    }
}
