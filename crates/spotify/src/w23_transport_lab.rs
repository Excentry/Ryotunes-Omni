use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use librespot_core::{Session, SpotifyUri};
use librespot_playback::audio_backend::{Sink, SinkError, SinkResult};
use librespot_playback::config::{Bitrate, PlayerConfig};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;
use librespot_playback::mixer::NoOpVolume;
use librespot_playback::player::{Player, PlayerEvent};

use crate::stream::StreamEvent;

const TRACK_PREFIX: &str = "spotify:track:";
const POSITION_INTERVAL: Duration = Duration::from_millis(500);
const MAX_CHUNK_BYTES: usize = 16 * 1024;
const SEND_RETRY: Duration = Duration::from_millis(2);
const READ_RETRY: Duration = Duration::from_millis(10);

#[derive(Clone)]
struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Default)]
struct Metrics {
    produced: AtomicU64,
    consumed: AtomicU64,
    blocked_micros: AtomicU64,
    max_buffered: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MetricsSnapshot {
    pub produced_bytes: u64,
    pub consumed_bytes: u64,
    pub blocked_micros: u64,
    pub max_buffered_bytes: u64,
}

impl Metrics {
    fn produced(&self, bytes: usize) {
        let total = self.produced.fetch_add(bytes as u64, Ordering::Relaxed) + bytes as u64;
        let pending = total.saturating_sub(self.consumed.load(Ordering::Relaxed));
        self.max_buffered.fetch_max(pending, Ordering::Relaxed);
    }

    fn consumed(&self, bytes: usize) {
        self.consumed.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    fn blocked(&self, duration: Duration) {
        self.blocked_micros.fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            produced_bytes: self.produced.load(Ordering::Relaxed),
            consumed_bytes: self.consumed.load(Ordering::Relaxed),
            blocked_micros: self.blocked_micros.load(Ordering::Relaxed),
            max_buffered_bytes: self.max_buffered.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone)]
pub struct LabMetrics(Arc<Metrics>);

impl LabMetrics {
    pub fn snapshot(&self) -> MetricsSnapshot {
        self.0.snapshot()
    }
}

struct ActiveSink {
    sender: SyncSender<Vec<u8>>,
    cancel: CancelToken,
    metrics: Arc<Metrics>,
}

struct ChannelSink {
    pending: Receiver<ActiveSink>,
    active: Option<ActiveSink>,
}

pub struct PcmReader {
    receiver: Receiver<Vec<u8>>,
    cancel: CancelToken,
    metrics: Arc<Metrics>,
    chunk: Vec<u8>,
    offset: usize,
}

impl Read for PcmReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            if self.cancel.is_cancelled() {
                return Ok(0);
            }
            if self.offset < self.chunk.len() {
                let count = output.len().min(self.chunk.len() - self.offset);
                output[..count].copy_from_slice(&self.chunk[self.offset..self.offset + count]);
                self.offset += count;
                self.metrics.consumed(count);
                return Ok(count);
            }
            match self.receiver.recv_timeout(READ_RETRY) {
                Ok(chunk) => {
                    self.chunk = chunk;
                    self.offset = 0;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
    }
}

impl Sink for ChannelSink {
    fn start(&mut self) -> SinkResult<()> {
        if self.active.is_some() {
            return Err(SinkError::StateChange("PCM sink is already started".to_owned()));
        }
        self.active = Some(self.pending.recv().map_err(|_| {
            SinkError::ConnectionRefused("PCM source queue disconnected".to_owned())
        })?);
        Ok(())
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.active.take();
        Ok(())
    }

    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        let bytes = match packet {
            AudioPacket::Samples(samples) => converter
                .f64_to_s16(&samples)
                .into_iter()
                .flat_map(i16::to_le_bytes)
                .collect::<Vec<_>>(),
            AudioPacket::Raw(bytes) => bytes,
        };
        let active = self
            .active
            .as_ref()
            .ok_or_else(|| SinkError::NotConnected("PCM sink is not started".to_owned()))?;
        for chunk in bytes.chunks(MAX_CHUNK_BYTES) {
            let mut pending = chunk.to_vec();
            let blocked_at = Instant::now();
            let mut was_blocked = false;
            loop {
                if active.cancel.is_cancelled() {
                    return Err(SinkError::OnWrite("PCM stream cancelled".to_owned()));
                }
                match active.sender.try_send(pending) {
                    Ok(()) => {
                        active.metrics.produced(chunk.len());
                        if was_blocked {
                            active.metrics.blocked(blocked_at.elapsed());
                        }
                        break;
                    }
                    Err(TrySendError::Full(bytes)) => {
                        pending = bytes;
                        was_blocked = true;
                        std::thread::park_timeout(SEND_RETRY);
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        return Err(SinkError::OnWrite("PCM reader disconnected".to_owned()));
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct LabStream {
    pub id: u64,
    pub pcm: PcmReader,
    pub events: tokio::sync::mpsc::UnboundedReceiver<StreamEvent>,
    player: Arc<Player>,
    cancel: CancelToken,
    metrics: Arc<Metrics>,
}

#[derive(Clone)]
pub struct LabCancellation {
    player: Arc<Player>,
    cancel: CancelToken,
}

impl LabCancellation {
    pub fn cancel(&self) {
        self.cancel.cancel();
        self.player.stop();
    }
}

impl LabStream {
    pub fn cancel(&self) {
        self.cancellation().cancel();
    }

    pub fn cancellation(&self) -> LabCancellation {
        LabCancellation { player: self.player.clone(), cancel: self.cancel.clone() }
    }

    pub fn metrics(&self) -> MetricsSnapshot {
        self.metrics.snapshot()
    }

    pub fn metrics_handle(&self) -> LabMetrics {
        LabMetrics(self.metrics.clone())
    }
}

pub struct SpotifyTransportLab {
    player: Arc<Player>,
    pending: SyncSender<ActiveSink>,
    queue_chunks: usize,
    next_id: AtomicU64,
}

impl SpotifyTransportLab {
    pub fn new(session: Session, queue_chunks: usize) -> Result<Self> {
        anyhow::ensure!(queue_chunks > 0, "PCM queue capacity must be non-zero");
        let config = PlayerConfig {
            bitrate: Bitrate::Bitrate320,
            gapless: false,
            position_update_interval: Some(POSITION_INTERVAL),
            ..Default::default()
        };
        let (pending, receiver) = mpsc::sync_channel(1);
        let player = Player::new(config, session, Box::new(NoOpVolume), move || {
            Box::new(ChannelSink { pending: receiver, active: None })
        });
        Ok(Self { player, pending, queue_chunks, next_id: AtomicU64::new(0) })
    }

    pub fn stream(&self, track_id: &str) -> Result<LabStream> {
        let track_id = if track_id.starts_with(TRACK_PREFIX) {
            track_id.to_owned()
        } else {
            format!("{TRACK_PREFIX}{track_id}")
        };
        let uri = SpotifyUri::from_uri(&track_id)
            .with_context(|| format!("{track_id} is not a Spotify track id"))?;
        let (sender, receiver) = mpsc::sync_channel(self.queue_chunks);
        let cancel = CancelToken::new();
        let metrics = Arc::new(Metrics::default());
        self.pending.send(ActiveSink {
            sender,
            cancel: cancel.clone(),
            metrics: metrics.clone(),
        })?;
        let mut events = self.player.get_player_event_channel();
        self.player.load(uri, true, 0);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let player = self.player.clone();
        let event_player = player.clone();
        let event_cancel = cancel.clone();
        let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut request = None;
            while let Some(event) = events.recv().await {
                if let PlayerEvent::PlayRequestIdChanged { play_request_id } = event {
                    request.get_or_insert(play_request_id);
                    continue;
                }
                if event.get_play_request_id().is_some_and(|id| Some(id) != request) {
                    continue;
                }
                let mapped = match event {
                    PlayerEvent::Playing { position_ms, .. } => {
                        StreamEvent::Playing(Duration::from_millis(position_ms as u64))
                    }
                    PlayerEvent::Paused { position_ms, .. } => {
                        StreamEvent::Paused(Duration::from_millis(position_ms as u64))
                    }
                    PlayerEvent::PositionChanged { position_ms, .. }
                    | PlayerEvent::PositionCorrection { position_ms, .. } => {
                        StreamEvent::Position(Duration::from_millis(position_ms as u64))
                    }
                    PlayerEvent::EndOfTrack { .. } => StreamEvent::EndOfTrack,
                    PlayerEvent::Unavailable { .. } => StreamEvent::Unavailable,
                    _ => continue,
                };
                if mapped == StreamEvent::EndOfTrack {
                    event_player.stop();
                    event_cancel.cancel();
                }
                if event_sender.send(mapped).is_err() {
                    break;
                }
            }
        });
        Ok(LabStream {
            id,
            pcm: PcmReader {
                receiver,
                cancel: cancel.clone(),
                metrics: metrics.clone(),
                chunk: Vec::new(),
                offset: 0,
            },
            events: event_receiver,
            player,
            cancel,
            metrics,
        })
    }
}
