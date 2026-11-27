use std::sync::atomic::Ordering;

use listen_protocol::{Playback, PlaybackKind, Track};

use super::{loudness_gain, media_title, song_to_track, track_to_song, AppState};
use crate::listentogether::SyncCommand;
impl AppState {
    pub async fn apply_sync(self: &std::sync::Arc<Self>, cmd: SyncCommand) {
        match cmd {
            SyncCommand::HostSeed => self.lt_host_seed().await,
            SyncCommand::Release => {}
            SyncCommand::ApplyState(state) => self.lt_apply_state(state).await,
            SyncCommand::ChangeTrack { track, position_ms, playing, queue } => {
                self.lt_apply_change_track(track, position_ms, playing, queue).await
            }
            SyncCommand::Play { position_ms, server_time_ms } => {
                self.lt_apply_play(position_ms, server_time_ms).await
            }
            SyncCommand::Pause { position_ms } => self.lt_apply_pause(position_ms).await,
            SyncCommand::Seek { position_ms } => {
                let _ = self.player.seek(position_ms as f64 / 1000.0);
            }
            SyncCommand::SyncQueue { queue } => self.lt_mirror_queue(queue).await,
            SyncCommand::GuestAdd { track } => self.lt_enqueue_track(track).await,
        }
    }

    async fn lt_apply_state(self: &std::sync::Arc<Self>, state: listen_protocol::RoomState) {
        let Some(track) = state.current_track else { return };
        let already_loaded = {
            let q = self.queue.lock().await;
            q.items.get(q.current).map(|i| i.video_id == track.id).unwrap_or(false)
        };
        if already_loaded && !self.player.is_idle() {
            let target = state.position_ms as f64 / 1000.0;
            if state.is_playing {
                if (self.current_position() - target).abs() > 0.35 {
                    let _ = self.player.seek(target);
                }
                let _ = self.player.play();
            } else {
                if target > 0.5 {
                    let _ = self.player.seek(target);
                }
                let _ = self.player.pause();
            }
            self.lt_mirror_queue(state.queue).await;
        } else {
            self.lt_apply_change_track(track, state.position_ms, state.is_playing, state.queue)
                .await;
        }
    }

    async fn lt_apply_change_track(
        self: &std::sync::Arc<Self>,
        track: Track,
        position_ms: i64,
        playing: bool,
        upcoming: Vec<Track>,
    ) {
        let t0 = std::time::Instant::now();
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut q = self.queue.lock().await;
            let mut items = vec![track_to_song(&track)];
            items.extend(upcoming.iter().map(track_to_song));
            q.items = items;
            q.current = 0;
            q.played_from = 0;
            q.lookahead_loaded = None;
            q.shuffle_orig = None;
            q.radio_seed = None;
            q.source_name = None;
        }
        let data = match self.resolve(&track.id, false).await {
            Ok(d) => d,
            Err(e) => {
                self.emit_error(&track.id, &e.to_string());
                return;
            }
        };
        if self.generation.load(Ordering::SeqCst) != gen {
            return;
        }
        if let Err(e) = self.player.load(
            &data.stream_url,
            &data.headers,
            loudness_gain(data.loudness_db),
            &media_title(&track.title, &track.artist),
            &data.mpv_options,
        ) {
            self.emit_error(&track.id, &e.to_string());
            return;
        }
        let target_ms =
            if playing { position_ms + t0.elapsed().as_millis() as i64 } else { position_ms };
        let pos = target_ms as f64 / 1000.0;
        if pos > 0.5 {
            let _ = self.player.seek(pos);
        }
        let _ = if playing { self.player.play() } else { self.player.pause() };
        if let Some(item) = self.current_item().await {
            self.emit_now_playing(&item, "listen-together");
        }
        if !playing {
            self.emit("playback-state", "paused");
        }
        self.emit_queue().await;
        if playing {
            let lt = self.lt.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                lt.request_sync().await;
            });
        }
    }

    async fn lt_apply_play(&self, position_ms: i64, server_time_ms: i64) {
        let target = if server_time_ms > 0 {
            position_ms + (now_ms() - server_time_ms).max(0)
        } else {
            position_ms
        };
        let cur_ms = (self.current_position() * 1000.0) as i64;
        if (cur_ms - target).abs() > 2000 {
            let _ = self.player.seek(target as f64 / 1000.0);
        }
        let _ = self.player.play();
    }

    async fn lt_apply_pause(&self, position_ms: i64) {
        let cur_ms = (self.current_position() * 1000.0) as i64;
        if (cur_ms - position_ms).abs() > 2000 {
            let _ = self.player.seek(position_ms as f64 / 1000.0);
        }
        let _ = self.player.pause();
    }

    pub(super) async fn lt_broadcast_current_track(&self, position_ms: i64, playing: bool) {
        if !self.lt.is_host().await {
            return;
        }
        let (track, queue) = {
            let q = self.queue.lock().await;
            let Some(cur) = q.items.get(q.current) else { return };
            let track = song_to_track(cur);
            let queue: Vec<Track> =
                q.items.iter().skip(q.current + 1).take(50).map(song_to_track).collect();
            (track, queue)
        };
        let mut p = Playback::new(PlaybackKind::ChangeTrack);
        p.track = Some(track);
        p.position_ms = position_ms;
        p.playing = playing;
        p.queue = Some(queue);
        self.lt.broadcast_playback(p).await;
    }

    async fn lt_host_seed(&self) {
        let position_ms = (self.current_position() * 1000.0) as i64;
        let playing = self.is_playing.load(Ordering::Relaxed);
        self.lt_broadcast_current_track(position_ms, playing).await;
    }

    pub async fn lt_on_play_state(&self, playing: bool) {
        if !self.lt.is_host().await {
            return;
        }
        let pos_ms = (self.current_position() * 1000.0) as i64;
        let p = if playing {
            let mut p = Playback::at(PlaybackKind::Play, pos_ms);
            p.playing = true;
            p
        } else {
            Playback::at(PlaybackKind::Pause, pos_ms)
        };
        self.lt.broadcast_playback(p).await;
    }

    pub async fn user_seek(&self, position: f64) -> Result<(), String> {
        if self.lt.is_guest().await {
            return Ok(());
        }
        let position = position.max(0.0);
        self.player.seek(position).map_err(|e| e.to_string())?;
        if self.lt.is_host().await {
            self.lt
                .broadcast_playback(Playback::at(PlaybackKind::Seek, (position * 1000.0) as i64))
                .await;
        }
        Ok(())
    }

    pub(super) async fn lt_broadcast_queue(&self) {
        if !self.lt.is_host().await {
            return;
        }
        let queue: Vec<Track> = {
            let q = self.queue.lock().await;
            q.items.iter().skip(q.current + 1).take(50).map(song_to_track).collect()
        };
        let mut p = Playback::new(PlaybackKind::SyncQueue);
        p.queue = Some(queue);
        self.lt.broadcast_playback(p).await;
    }

    async fn lt_mirror_queue(&self, upcoming: Vec<Track>) {
        {
            let mut q = self.queue.lock().await;
            let keep = q.current + 1;
            q.items.truncate(keep);
            q.items.extend(upcoming.iter().map(track_to_song));
        }
        self.emit_queue().await;
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
