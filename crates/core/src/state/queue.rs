use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use innertube::SongItem;
use rand::seq::SliceRandom;

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
