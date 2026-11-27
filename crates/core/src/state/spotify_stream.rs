use super::AppState;
use ryotunes_spotify::{StreamEvent, StreamHandle};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
impl AppState {
    pub(super) fn drop_spotify_stream(&self) {
        if let Some(prev) = self.spotify_stream.lock().take() {
            prev.watcher.abort();
        }
    }
    pub(super) async fn watch_spotify_stream(
        self: std::sync::Arc<Self>,
        video_id: String,
        mut handle: StreamHandle,
    ) {
        while let Some(event) = handle.next_event().await {
            if event == StreamEvent::Unavailable {
                self.skip_unavailable_spotify(&video_id).await;
                return;
            }
        }
    }
    fn skip_unavailable_spotify<'a>(
        self: &'a Arc<Self>,
        video_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let has_next = {
                let q = self.queue.lock().await;
                if q.items.get(q.current).map(|i| i.video_id != video_id).unwrap_or(true) {
                    return;
                }
                q.current + 1 < q.items.len()
            };
            self.emit_error(video_id, "This track isn't available on Spotify.");
            if has_next {
                self.on_track_ended().await;
            } else {
                let _ = self.player.stop();
                self.drop_spotify_stream();
                self.emit("playback-state", "paused");
                self.media_set_playing(false);
                self.persist_queue().await;
            }
        })
    }
}
