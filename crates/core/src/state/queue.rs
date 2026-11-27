use innertube::SongItem;

use super::RepeatMode;
use crate::orchestrator::PlaybackPing;

#[derive(Default)]
pub(super) struct QueueState {
    pub(super) items: Vec<SongItem>,
    pub(super) current: usize,
    pub(super) played_from: usize,
    pub(super) shuffle_orig: Option<Vec<SongItem>>,
    pub(super) repeat: RepeatMode,
    pub(super) radio_seed: Option<String>,
    pub(super) source_name: Option<String>,
    pub(super) radio: bool,
    pub(super) lookahead_loaded: Option<usize>,
    pub(super) current_client: Option<String>,
    pub(super) lookahead_client: Option<String>,
    pub(super) lookahead_gain: Option<Option<f64>>,
    pub(super) playback_ping: Option<PlaybackPing>,
    pub(super) lookahead_playback_ping: Option<PlaybackPing>,
    pub(super) cpn: String,
    pub(super) history_pinged: bool,
    pub(super) duration: f64,
    pub(super) retried: Option<String>,
}

impl QueueState {
    pub(super) fn seek_to(&mut self, index: usize) {
        self.current = index;
        self.played_from = self.played_from.min(index);
    }
}
