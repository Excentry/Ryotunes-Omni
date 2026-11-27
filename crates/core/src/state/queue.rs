use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering;

use innertube::SongItem;
use listen_protocol::Track;
use rand::seq::SliceRandom;

use super::{song_to_track, track_to_song, AppState, Fill, RepeatMode};
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

pub(super) fn queue_event_payload(q: &QueueState, unchanged: bool) -> serde_json::Value {
    if unchanged {
        serde_json::json!({
            "currentIndex": q.current,
            "playedFrom": q.played_from,
            "shuffle": q.shuffle_orig.is_some(),
            "repeat": q.repeat,
            "sourceName": &q.source_name,
            "current": q.items.get(q.current),
        })
    } else {
        serde_json::json!({
            "items": &q.items,
            "currentIndex": q.current,
            "playedFrom": q.played_from,
            "shuffle": q.shuffle_orig.is_some(),
            "repeat": q.repeat,
            "sourceName": &q.source_name,
        })
    }
}

pub(super) fn enqueue_at(q: &QueueState) -> usize {
    let mut at = (q.current + 1).min(q.items.len());
    while q.items.get(at).map(|i| i.queued || i.queued_end).unwrap_or(false) {
        at += 1;
    }
    at
}

pub(super) fn drop_duplicates(
    items: &mut Vec<SongItem>,
    current: &mut usize,
    played_from: &mut usize,
    ids: &HashSet<&str>,
) -> bool {
    let mut removed = false;
    for i in (0..items.len()).rev() {
        if i != *current && ids.contains(items[i].video_id.as_str()) {
            items.remove(i);
            if i < *current {
                *current -= 1;
                if i < *played_from {
                    *played_from -= 1;
                }
            }
            removed = true;
        }
    }
    removed
}

pub(super) fn guest_insert_index(items: &[SongItem], current: usize) -> usize {
    let mut at = (current + 1).min(items.len());
    while items.get(at).map(|i| i.queued).unwrap_or(false) {
        at += 1;
    }
    at
}

pub(super) fn radio_seed_for(source_id: Option<String>) -> Option<String> {
    source_id.map(|id| {
        let id = id.strip_prefix("VL").unwrap_or(&id);
        if id.starts_with("RD") {
            id.to_owned()
        } else {
            format!("RDAMPL{id}")
        }
    })
}

pub(super) fn splice_radio_into(
    q: &mut QueueState,
    items: Vec<SongItem>,
    seed: String,
    title: Option<String>,
) {
    let carried = upcoming_queued(&q.items, q.current);
    q.items.truncate(q.current + 1);
    q.items.extend(carried);
    let mut seen: HashSet<String> = q.items.iter().map(|i| i.video_id.clone()).collect();
    for item in items {
        if seen.insert(item.video_id.clone()) {
            q.items.push(item);
        }
    }
    q.radio_seed = Some(seed);
    q.source_name = title;
    q.radio = true;
    if q.shuffle_orig.is_some() {
        q.shuffle_orig = Some(q.items.clone());
        let cur = q.current;
        shuffle_upcoming(&mut q.items, cur);
    }
}

pub(super) fn autoplay_tail_near(len: usize, current: usize) -> bool {
    len.saturating_sub(current) <= 2
}

pub(super) fn merge_radio(
    items: &mut Vec<SongItem>,
    fresh: Vec<SongItem>,
    mut existing: HashSet<String>,
    cap: usize,
) -> usize {
    let mut added = 0;
    for mut item in fresh {
        if added >= cap {
            break;
        }
        if !existing.insert(item.video_id.clone()) {
            continue;
        }
        item.autoplay = true;
        items.push(item);
        added += 1;
    }
    added
}

pub(super) fn next_index(len: usize, current: usize, repeat: RepeatMode) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let next = current + 1;
    if next < len {
        Some(next)
    } else if repeat == RepeatMode::All {
        Some(0)
    } else {
        None
    }
}

pub(super) fn unshuffled(
    orig: Vec<SongItem>,
    heard: &HashSet<String>,
    playing_id: &str,
    fallback: usize,
) -> (Vec<SongItem>, usize) {
    let idx = orig
        .iter()
        .position(|i| i.video_id == playing_id)
        .unwrap_or_else(|| fallback.min(orig.len().saturating_sub(1)));
    let mut items = orig;
    let mut i = 0;
    items.retain(|it| {
        let keep = i <= idx || !heard.contains(&it.video_id);
        i += 1;
        keep
    });
    let tail = items.split_off((idx + 1).min(items.len()));
    let (queued, rest): (Vec<_>, Vec<_>) =
        tail.into_iter().partition(|it| it.queued || it.queued_end);
    items.extend(queued);
    items.extend(rest);
    (items, idx)
}

pub(super) fn upcoming_queued(items: &[SongItem], current: usize) -> Vec<SongItem> {
    items.iter().skip(current + 1).filter(|i| i.queued || i.queued_end).cloned().collect()
}

pub(super) fn shuffle_upcoming(items: &mut [SongItem], current: usize) {
    let mut start = current + 1;
    while items.get(start).map(|i| i.queued || i.queued_end).unwrap_or(false) {
        start += 1;
    }
    let mut i = current + 1;
    while i < start {
        let Some(from) = items[i].queued_from.clone() else {
            i += 1;
            continue;
        };
        let mut end = i + 1;
        while end < start && items[end].queued_from.as_deref() == Some(from.as_str()) {
            end += 1;
        }
        items[i..end].shuffle(&mut rand::thread_rng());
        i = end;
    }
    if start < items.len() {
        items[start..].shuffle(&mut rand::thread_rng());
        items[start..].sort_by_key(|i| i.autoplay);
    }
}

pub(super) fn append_page(q: &mut QueueState, page: Vec<SongItem>, playing: bool) {
    if let Some(orig) = q.shuffle_orig.as_mut() {
        orig.extend(page.iter().cloned());
    }
    if playing {
        q.items.extend(page);
        if q.shuffle_orig.is_some() {
            let pivot = q.lookahead_loaded.filter(|&i| i > q.current).unwrap_or(q.current);
            shuffle_upcoming(&mut q.items, pivot);
        }
    } else {
        let at = enqueue_at(q);
        q.items.splice(at..at, page);
    }
}

pub(super) fn is_mix(source_id: Option<&str>) -> bool {
    source_id.is_some_and(|id| id.strip_prefix("VL").unwrap_or(id).starts_with("RD"))
}

pub(super) fn shuffle_new_queue(items: &mut [SongItem], start: usize) -> usize {
    if !items.is_empty() {
        items.swap(0, start.min(items.len() - 1));
        shuffle_upcoming(items, 0);
    }
    0
}

pub(super) fn queue_fingerprint(items: &[SongItem]) -> u64 {
    let mut hasher = DefaultHasher::new();
    items.len().hash(&mut hasher);
    for item in items {
        item.video_id.hash(&mut hasher);
        item.set_video_id.hash(&mut hasher);
        item.queued.hash(&mut hasher);
        item.queued_end.hash(&mut hasher);
        item.queued_from.hash(&mut hasher);
        item.queued_by.hash(&mut hasher);
        item.autoplay.hash(&mut hasher);
    }
    hasher.finish()
}

pub(super) fn persist_fingerprint(q: &QueueState) -> u64 {
    let mut hasher = DefaultHasher::new();
    queue_fingerprint(&q.items).hash(&mut hasher);
    q.shuffle_orig.is_some().hash(&mut hasher);
    q.radio_seed.hash(&mut hasher);
    q.source_name.hash(&mut hasher);
    q.radio.hash(&mut hasher);
    hasher.finish()
}

impl AppState {
    pub async fn toggle_shuffle(self: &std::sync::Arc<Self>) {
        if self.lt.is_guest().await {
            self.emit_guest_hint();
            return;
        }
        {
            let mut q = self.queue.lock().await;
            if q.items.is_empty() {
                return;
            }
            if let Some(orig) = q.shuffle_orig.take() {
                let playing = q.items[q.current].video_id.clone();
                let fallback = q.current;
                let heard: HashSet<String> =
                    q.items[..=q.current].iter().map(|i| i.video_id.clone()).collect();
                let (items, idx) = unshuffled(orig, &heard, &playing, fallback);
                q.items = items;
                q.current = idx;
                q.played_from = idx;
            } else {
                q.shuffle_orig = Some(q.items.clone());
                let current = q.current;
                shuffle_upcoming(&mut q.items, current);
            }
            if q.lookahead_loaded.take().is_some() {
                let _ = self.player.clear_playlist();
            }
        }
        self.emit_queue().await;
        self.persist_queue().await;
        let gen = self.generation.load(Ordering::SeqCst);
        self.prime_lookahead(gen).await;
        self.extend_queue_radio(gen).await;
        self.lt_broadcast_queue().await;
    }

    pub async fn set_stop_after_current(self: &std::sync::Arc<Self>, enabled: bool) {
        self.stop_after_current.store(enabled, Ordering::SeqCst);
        if enabled {
            let repeat_was_one = {
                let mut q = self.queue.lock().await;
                let was = q.repeat == RepeatMode::One;
                if was {
                    q.repeat = RepeatMode::Off;
                }
                q.lookahead_loaded = None;
                q.lookahead_client = None;
                q.lookahead_gain = None;
                q.lookahead_playback_ping = None;
                was
            };
            if repeat_was_one {
                let _ = self.player.set_loop_file(false);
                self.emit_queue().await;
                self.persist_queue().await;
            }
            if let Err(error) = self.player.clear_playlist() {
                tracing::debug!(%error, "clearing primed lookahead for stop-after-current failed");
            }
        } else {
            let gen = self.generation.load(Ordering::SeqCst);
            self.prime_lookahead(gen).await;
        }
        self.emit("stop-after-current", enabled);
    }

    pub async fn set_repeat(self: &std::sync::Arc<Self>, mode: RepeatMode) {
        if self.lt.is_guest().await {
            self.emit_guest_hint();
            return;
        }
        if mode == RepeatMode::One && self.stop_after_current.swap(false, Ordering::SeqCst) {
            self.emit("stop-after-current", false);
        }
        {
            let mut q = self.queue.lock().await;
            q.repeat = mode;
        }
        let _ = self.player.set_loop_file(mode == RepeatMode::One);
        self.emit_queue().await;
        self.persist_queue().await;
        if !self.stop_after_current.load(Ordering::SeqCst) {
            self.prime_lookahead(self.generation.load(Ordering::SeqCst)).await;
        }
    }
}

impl AppState {
    pub async fn play_next(
        self: &std::sync::Arc<Self>,
        items: Vec<SongItem>,
        from: Option<String>,
    ) {
        self.enqueue(items, true, from, None).await;
    }
    pub async fn add_to_queue(
        self: &std::sync::Arc<Self>,
        items: Vec<SongItem>,
        from: Option<String>,
        continuation: Option<String>,
    ) {
        self.enqueue(items, false, from, continuation).await;
    }
    async fn enqueue(
        self: &std::sync::Arc<Self>,
        items: Vec<SongItem>,
        next: bool,
        from: Option<String>,
        continuation: Option<String>,
    ) {
        if items.is_empty() {
            return;
        }
        if self.lt.is_guest().await {
            for item in items {
                self.lt.suggest(song_to_track(&item)).await;
            }
            return;
        }
        let items = match self.lt.is_host().await {
            false => items,
            true => {
                let by = self.lt.my_username().await.unwrap_or_else(|| "Host".into());
                items
                    .into_iter()
                    .map(|mut i| {
                        i.queued_by = Some(by.clone());
                        i
                    })
                    .collect()
            }
        };
        self.insert_queued(items, next, from.clone()).await;
        if let Some(token) = continuation {
            let gen = self.generation.load(Ordering::SeqCst);
            let me = self.clone();
            tokio::spawn(async move { me.fill_playlist(gen, token, Fill::Queued(from)).await });
        }
    }
    pub async fn lt_enqueue_track(self: &std::sync::Arc<Self>, track: Track) {
        self.insert_queued(vec![track_to_song(&track)], true, None).await;
    }
    async fn insert_queued(
        self: &std::sync::Arc<Self>,
        mut items: Vec<SongItem>,
        next: bool,
        from: Option<String>,
    ) {
        if items.is_empty() {
            return;
        }
        for item in &mut items {
            item.queued = next;
            item.queued_end = !next;
            item.queued_from = from.clone();
        }
        let dedupe = self.db.get_setting("prevent_duplicates").as_deref() == Some("true");
        let was_empty = {
            let mut q = self.queue.lock().await;
            let was_empty = q.items.is_empty();
            let before = q.items.get(q.current + 1).map(|i| i.video_id.clone());
            let ids: HashSet<&str> = items.iter().map(|i| i.video_id.as_str()).collect();
            let qm = &mut *q;
            let mut removed = dedupe
                && drop_duplicates(&mut qm.items, &mut qm.current, &mut qm.played_from, &ids);
            let cur = qm.current;
            let n = qm.items.len();
            let mut i = 0;
            qm.items.retain(|it| {
                let keep = i <= cur
                    || !((it.queued || it.queued_end) && ids.contains(it.video_id.as_str()));
                i += 1;
                keep
            });
            removed |= qm.items.len() != n;
            if removed {
                if let Some(orig) = qm.shuffle_orig.as_mut() {
                    let kept: HashSet<String> =
                        qm.items.iter().map(|i| i.video_id.clone()).collect();
                    orig.retain(|i| kept.contains(&i.video_id));
                }
            }
            let at = if next { guest_insert_index(&q.items, q.current) } else { enqueue_at(&q) };
            if let Some(orig) = q.shuffle_orig.as_mut() {
                orig.extend(items.iter().cloned());
                if from.is_some() {
                    use rand::seq::SliceRandom;
                    items.shuffle(&mut rand::thread_rng());
                }
            }
            q.items.splice(at..at, items);
            let expected = next_index(q.items.len(), q.current, q.repeat);
            let after = q.items.get(q.current + 1).map(|i| i.video_id.clone());
            if q.lookahead_loaded.is_some()
                && (removed || q.lookahead_loaded != expected || before != after)
            {
                q.lookahead_loaded = None;
                let _ = self.player.clear_playlist();
            }
            was_empty
        };
        if was_empty {
            self.play_index(0).await;
            self.lt_broadcast_queue().await;
            return;
        }
        self.emit_queue().await;
        self.persist_queue().await;
        self.prime_lookahead(self.generation.load(Ordering::SeqCst)).await;
        self.lt_broadcast_queue().await;
    }
    pub async fn remove_from_queue(self: &std::sync::Arc<Self>, index: usize) {
        if self.lt.is_guest().await {
            return;
        }
        let stale_lookahead = {
            let mut q = self.queue.lock().await;
            if index >= q.items.len() || index == q.current {
                return;
            }
            q.items.remove(index);
            if index < q.current {
                q.current -= 1;
                if index < q.played_from {
                    q.played_from -= 1;
                }
            }
            match q.lookahead_loaded {
                Some(i) if i == index => {
                    q.lookahead_loaded = None;
                    let _ = self.player.clear_playlist();
                    true
                }
                Some(i) if i > index => {
                    q.lookahead_loaded = Some(i - 1);
                    false
                }
                _ => false,
            }
        };
        self.emit_queue().await;
        self.persist_queue().await;
        if stale_lookahead {
            self.prime_lookahead(self.generation.load(Ordering::SeqCst)).await;
        }
        self.lt_broadcast_queue().await;
    }
    pub async fn move_in_queue(self: &std::sync::Arc<Self>, from: usize, to: usize) {
        if self.lt.is_guest().await {
            return;
        }
        let stale_lookahead = {
            let mut q = self.queue.lock().await;
            let first = q.current + 1;
            if from < first || from >= q.items.len() || to < first || to >= q.items.len() {
                return;
            }
            if from == to {
                return;
            }
            let before = q.items.get(first).map(|i| i.video_id.clone());
            let item = q.items.remove(from);
            q.items.insert(to, item);
            let expected = next_index(q.items.len(), q.current, q.repeat);
            let stale = q.lookahead_loaded.is_some()
                && (q.lookahead_loaded != expected
                    || before != q.items.get(first).map(|i| i.video_id.clone()));
            if stale {
                q.lookahead_loaded = None;
                let _ = self.player.clear_playlist();
            }
            stale
        };
        self.emit_queue().await;
        self.persist_queue().await;
        if stale_lookahead {
            self.prime_lookahead(self.generation.load(Ordering::SeqCst)).await;
        }
        self.lt_broadcast_queue().await;
    }
    pub async fn clear_queued(self: &std::sync::Arc<Self>) {
        if self.lt.is_guest().await {
            return;
        }
        {
            let mut q = self.queue.lock().await;
            let cur = q.current;
            let before = q.items.len();
            let mut i = 0;
            q.items.retain(|item| {
                let keep = i <= cur || !(item.queued || item.queued_end);
                i += 1;
                keep
            });
            if q.items.len() == before {
                return;
            }
            if q.lookahead_loaded.take().is_some() {
                let _ = self.player.clear_playlist();
            }
        }
        self.emit_queue().await;
        self.persist_queue().await;
        self.prime_lookahead(self.generation.load(Ordering::SeqCst)).await;
        self.lt_broadcast_queue().await;
    }
}

impl AppState {
    pub async fn next_in_queue(self: &std::sync::Arc<Self>) {
        let i = {
            let q = self.queue.lock().await;
            let repeat = if q.repeat == RepeatMode::One { RepeatMode::All } else { q.repeat };
            next_index(q.items.len(), q.current, repeat)
        };
        match i {
            Some(i) => self.play_index(i).await,
            None => {
                let gen = self.generation.load(Ordering::SeqCst);
                if self.extend_queue_radio(gen).await > 0 {
                    let next = self.queue.lock().await.current + 1;
                    self.play_index(next).await;
                }
            }
        }
    }
    pub async fn prev_in_queue(self: &std::sync::Arc<Self>) {
        let i = self.queue.lock().await.current.saturating_sub(1);
        self.play_index(i).await;
    }
    pub(super) async fn emit_queue(&self) {
        let q = self.queue.lock().await;
        let fingerprint = queue_fingerprint(&q.items);
        let unchanged =
            self.last_queue_fingerprint.swap(fingerprint, Ordering::Relaxed) == fingerprint;
        let payload = queue_event_payload(&q, unchanged);
        self.emit(if unchanged { "queue-index" } else { "queue-changed" }, payload);
    }
    pub async fn queue_snapshot(&self) -> serde_json::Value {
        let q = self.queue.lock().await;
        serde_json::json!({
        "items": &q.items,
        "currentIndex": q.current,
        "playedFrom": q.played_from,
        "shuffle": q.shuffle_orig.is_some(),
        "repeat": q.repeat,
        "sourceName": &q.source_name,
        })
    }
    pub(super) async fn persist_queue(&self) {
        let (index_json, blob) = {
            let q = self.queue.lock().await;
            let fingerprint = queue_fingerprint(&q.items);
            let index_json = serde_json::json!({
            "current": q.current,
            "playedFrom": q.played_from,
            "repeat": q.repeat,
            "fp": fingerprint.to_string(),
            })
            .to_string();
            let persist_fp = persist_fingerprint(&q);
            let changed =
                self.last_persisted_fingerprint.swap(persist_fp, Ordering::Relaxed) != persist_fp;
            let blob = changed.then(|| {
                serde_json::json!({
                "items": &q.items,
                "current": q.current,
                "playedFrom": q.played_from,
                "repeat": q.repeat,
                "shuffleOrig": &q.shuffle_orig,
                "radioSeed": &q.radio_seed,
                "sourceName": &q.source_name,
                "radio": q.radio,
                })
                .to_string()
            });
            (index_json, blob)
        };
        if let Some(blob) = blob {
            self.db.set_setting("queue_json", &blob);
        }
        self.db.set_setting("queue_index", &index_json);
    }
    pub async fn restore_queue(self: &std::sync::Arc<Self>) {
        let Some(json) = self.db.get_setting("queue_json") else { return };
        let Ok(saved) = serde_json::from_str::<serde_json::Value>(&json) else { return };
        let items: Vec<SongItem> = saved
            .get("items")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        if items.is_empty() {
            return;
        }
        let mut current = (saved.get("current").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
            .min(items.len() - 1);
        let mut played_from =
            (saved.get("playedFrom").and_then(|v| v.as_u64()).unwrap_or(current as u64) as usize)
                .min(current);
        let mut repeat: RepeatMode = saved
            .get("repeat")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let shuffle_orig: Option<Vec<SongItem>> =
            saved.get("shuffleOrig").and_then(|v| serde_json::from_value(v.clone()).ok()).flatten();
        let radio_seed: Option<String> =
            saved.get("radioSeed").and_then(|v| v.as_str()).map(str::to_owned);
        let source_name: Option<String> =
            saved.get("sourceName").and_then(|v| v.as_str()).map(str::to_owned);
        let radio = saved.get("radio").and_then(|v| v.as_bool()).unwrap_or(false);
        let items_fp = queue_fingerprint(&items).to_string();
        if let Some(idx) = self
            .db
            .get_setting("queue_index")
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(|idx| idx.get("fp").and_then(|v| v.as_str()) == Some(items_fp.as_str()))
        {
            if let Some(c) = idx.get("current").and_then(|v| v.as_u64()) {
                current = (c as usize).min(items.len() - 1);
            }
            if let Some(p) = idx.get("playedFrom").and_then(|v| v.as_u64()) {
                played_from = (p as usize).min(current);
            }
            if let Some(r) = idx.get("repeat").and_then(|v| serde_json::from_value(v.clone()).ok())
            {
                repeat = r;
            }
        }
        let pos = self.db.get_setting("queue_position").and_then(|s| s.parse::<f64>().ok());
        if let Some(p) = pos.filter(|p| *p > 0.0) {
            *self.pending_seek.lock().unwrap() = Some((items[current].video_id.clone(), p));
        }
        {
            let mut q = self.queue.lock().await;
            q.current = current;
            q.played_from = played_from;
            q.items = items;
            q.repeat = repeat;
            q.shuffle_orig = shuffle_orig;
            q.radio_seed = radio_seed;
            q.source_name = source_name;
            q.radio = radio;
        }
        if repeat == RepeatMode::One {
            let _ = self.player.set_loop_file(true);
        }
        if let Some(item) = self.current_item().await {
            self.media_set_playing(false);
            self.emit_now_playing(&item, "restored");
            self.refresh_rating(&item.video_id, self.generation.load(Ordering::SeqCst));
            self.emit("playback-state", "paused");
        }
        self.emit_queue().await;
    }
}
