use std::sync::atomic::Ordering;

use innertube::SongItem;

use crate::media_ref::MediaRef;

use super::{
    parse_duration_ms, saved_volume, AppState, ScTrackMeta, LIKED_SONGS_ID, LIKED_SONGS_TITLE,
};

impl AppState {
    pub(super) async fn current_item(&self) -> Option<SongItem> {
        let q = self.queue.lock().await;
        q.items.get(q.current).cloned()
    }

    fn now_playing_json(&self, item: &SongItem, stream_client: &str) -> serde_json::Value {
        let mut now = serde_json::json!({
        "videoId": item.video_id,
        "title": item.title,
        "artists": item.artists,
        "artistId": item.artist_id,
        "artistRuns": item.artist_runs,
        "thumbnail": item.thumbnail,
        "duration": item.duration,
        "streamClient": stream_client,
        "rating": if item.rating.is_none() || !self.it.is_logged_in() || matches!(MediaRef::parse(&item.video_id), MediaRef::SoundCloudTrack(_) | MediaRef::SoundCloudUser(_) | MediaRef::SoundCloudPlaylist(_) | MediaRef::SoundCloudSystem(_) | MediaRef::SoundCloudOther(_)) {
        if self.liked_locally(&item.video_id) { Some(innertube::Rating::Like) } else { item.rating }
        } else {
        item.rating
        },
        });
        if matches!(
            MediaRef::parse(&item.video_id),
            MediaRef::SoundCloudTrack(_)
                | MediaRef::SoundCloudUser(_)
                | MediaRef::SoundCloudPlaylist(_)
                | MediaRef::SoundCloudSystem(_)
                | MediaRef::SoundCloudOther(_)
        ) {
            if let Some(meta) = self.sc_meta.lock().get(&item.video_id).cloned() {
                let obj = now.as_object_mut().expect("json object");
                if let Some(plays) = meta.plays {
                    obj.insert("plays".to_owned(), serde_json::Value::String(plays));
                }
                if let Some(likes) = meta.likes {
                    obj.insert("likes".to_owned(), serde_json::Value::String(likes));
                }
                if let Some(genre) = meta.genre {
                    obj.insert("genre".to_owned(), serde_json::Value::String(genre));
                }
            }
        }
        now
    }

    pub(super) fn remember_sc_meta(&self, video_id: &str, track: &ryotunes_soundcloud::Track) {
        let mut map = self.sc_meta.lock();
        if map.len() > 512 {
            map.clear();
        }
        map.insert(
            video_id.to_owned(),
            ScTrackMeta {
                plays: track.plays.map(crate::soundcloud_bridge::abbreviate),
                likes: track.likes.map(crate::soundcloud_bridge::abbreviate),
                genre: track.genre.clone(),
            },
        );
    }

    pub async fn playback_snapshot(&self) -> serde_json::Value {
        let (duration, item) = {
            let q = self.queue.lock().await;
            (q.duration, q.items.get(q.current).cloned())
        };
        let position = self.player.position().unwrap_or_else(|| self.current_position());
        self.note_position_sample(position);
        serde_json::json!({
        "now": item.as_ref().map(|i| self.now_playing_json(i, "current")),
        "paused": !self.is_playing.load(Ordering::Relaxed),
        "position": position,
        "duration": duration,
        "volume": saved_volume(&self.db),
        "stopAfterCurrent": self.stop_after_current.load(Ordering::Relaxed),
        })
    }

    pub(super) fn refresh_rating(self: &std::sync::Arc<Self>, video_id: &str, gen: u64) {
        if !self.it.is_logged_in()
            || matches!(MediaRef::parse(video_id), MediaRef::Local(_) | MediaRef::Radio(_))
        {
            return;
        }
        let me = self.clone();
        let video_id = video_id.to_owned();
        tokio::spawn(async move {
            let Some(client) = me.clients.get(innertube::METADATA_CLIENT) else { return };
            let epoch = me.rate_epoch.load(Ordering::SeqCst);
            let rating = match me.it.next(client, Some(&video_id), None).await {
                Ok(next) => next.rating,
                Err(e) => {
                    tracing::debug!(error = %e, "rating refresh failed; keeping row snapshot");
                    return;
                }
            };
            let Some(rating) = rating else { return };
            if me.generation.load(Ordering::SeqCst) != gen
                || me.rate_epoch.load(Ordering::SeqCst) != epoch
            {
                return;
            }
            let changed = {
                let mut q = me.queue.lock().await;
                let mut changed = false;
                for item in q.items.iter_mut().filter(|i| i.video_id == video_id) {
                    changed |= item.rating != Some(rating);
                    item.rating = Some(rating);
                }
                changed
            };
            if !changed {
                return;
            }
            me.emit("rating", serde_json::json!({ "videoId": video_id, "rating": rating }));
        });
    }

    pub async fn queue_item(&self, video_id: &str) -> Option<SongItem> {
        let q = self.queue.lock().await;
        q.items.iter().find(|i| i.video_id == video_id).cloned()
    }

    pub fn liked_locally(&self, video_id: &str) -> bool {
        self.db
            .local_playlist_track_json(LIKED_SONGS_ID)
            .iter()
            .filter_map(|j| serde_json::from_str::<SongItem>(j).ok())
            .any(|s| s.video_id == video_id)
    }

    pub async fn rate_locally(&self, video_id: &str, like: bool) -> Result<(), String> {
        if like {
            let item = self
                .queue_item(video_id)
                .await
                .ok_or_else(|| "This track is not in the queue.".to_string())?;
            if self.db.local_playlist(LIKED_SONGS_ID).is_none() {
                self.db
                    .create_local_playlist(LIKED_SONGS_ID, LIKED_SONGS_TITLE)
                    .map_err(|e| format!("device playlist: {e}"))?;
            }
            let json = serde_json::to_string(&item).map_err(|e| e.to_string())?;
            self.db
                .add_local_playlist_track(LIKED_SONGS_ID, video_id, &json)
                .map_err(|e| format!("device playlist: {e}"))?;
        } else if self.db.local_playlist(LIKED_SONGS_ID).is_some() {
            self.db
                .remove_local_playlist_track(LIKED_SONGS_ID, video_id)
                .map_err(|e| format!("device playlist: {e}"))?;
        }
        let rating = if like { innertube::Rating::Like } else { innertube::Rating::Indifferent };
        {
            let mut q = self.queue.lock().await;
            for item in q.items.iter_mut().filter(|i| i.video_id == video_id) {
                item.rating = Some(rating);
            }
        }
        self.emit("rating", serde_json::json!({ "videoId": video_id, "rating": rating }));
        self.emit("library-changed", serde_json::json!({ "id": LIKED_SONGS_ID }));
        Ok(())
    }

    pub(super) fn emit_now_playing(&self, item: &SongItem, stream_client: &str) {
        self.emit("now-playing", self.now_playing_json(item, stream_client));
        self.emit("playback-state", "playing");
        if let Some(m) = &self.media {
            let cover = item.thumbnail.as_ref().map(|t| {
                if t.contains("://") {
                    t.clone()
                } else {
                    format!("file://{t}")
                }
            });
            m.set_metadata(&item.title, &item.artists, item.album.as_deref(), cover.as_deref());
            let secs = parse_duration_ms(item.duration.as_deref()) as f64 / 1000.0;
            if secs > 0.0 {
                m.set_duration(secs);
            }
        }
        if let Some(d) = &self.discord {
            d.set_track(item);
        }
        if !matches!(MediaRef::parse(&item.video_id), MediaRef::Radio(_)) {
            self.lastfm.set_track(item);
        }
        self.last_media_push.store(0, Ordering::Relaxed);
    }
}
