use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::time::Duration;

use librespot_playback::audio_backend::{Sink, SinkError, SinkResult};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;

const MAX_CHUNK_BYTES: usize = 16 * 1024;
const RETRY_INTERVAL: Duration = Duration::from_millis(2);
const READ_INTERVAL: Duration = Duration::from_millis(10);

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

struct PcmReader {
    receiver: Receiver<Vec<u8>>,
    cancel: CancelToken,
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
                return Ok(count);
            }
            match self.receiver.recv_timeout(READ_INTERVAL) {
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

struct PcmSource {
    id: u64,
    reader: PcmReader,
    cancel: CancelToken,
}

struct ActiveSink {
    sender: SyncSender<Vec<u8>>,
    cancel: CancelToken,
}

struct ChannelSink {
    capacity: usize,
    sources: mpsc::Sender<PcmSource>,
    active: Option<ActiveSink>,
    next_id: u64,
}

fn channel_sink(capacity: usize) -> (ChannelSink, Receiver<PcmSource>) {
    let (sources, receiver) = mpsc::channel();
    (ChannelSink { capacity, sources, active: None, next_id: 0 }, receiver)
}

impl Sink for ChannelSink {
    fn start(&mut self) -> SinkResult<()> {
        if self.active.is_some() {
            return Err(SinkError::StateChange("PCM sink is already started".to_owned()));
        }
        let (sender, receiver) = mpsc::sync_channel(self.capacity);
        let cancel = CancelToken::new();
        let id = self.next_id;
        self.next_id += 1;
        self.sources
            .send(PcmSource {
                id,
                reader: PcmReader {
                    receiver,
                    cancel: cancel.clone(),
                    chunk: Vec::new(),
                    offset: 0,
                },
                cancel: cancel.clone(),
            })
            .map_err(|_| {
                SinkError::ConnectionRefused("PCM source registry disconnected".to_owned())
            })?;
        self.active = Some(ActiveSink { sender, cancel });
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
            loop {
                if active.cancel.is_cancelled() {
                    return Err(SinkError::OnWrite("PCM stream cancelled".to_owned()));
                }
                match active.sender.try_send(pending) {
                    Ok(()) => break,
                    Err(TrySendError::Full(bytes)) => {
                        pending = bytes;
                        std::thread::park_timeout(RETRY_INTERVAL);
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

impl Drop for ChannelSink {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            active.cancel.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use librespot_playback::audio_backend::Sink;
    use librespot_playback::convert::Converter;
    use librespot_playback::decoder::AudioPacket;

    use super::{MAX_CHUNK_BYTES, channel_sink};

    #[test]
    fn tracks_have_independent_sources_and_eof() {
        let (mut sink, sources) = channel_sink(1);
        let mut converter = Converter::new(None);

        sink.start().unwrap();
        let first = sources.recv().unwrap();
        sink.write(AudioPacket::Raw(vec![3; 32]), &mut converter).unwrap();
        sink.stop().unwrap();
        let mut first_bytes = Vec::new();
        let mut first_reader = first.reader;
        first_reader.read_to_end(&mut first_bytes).unwrap();
        assert_eq!(first.id, 0);
        assert_eq!(first_bytes, vec![3; 32]);

        sink.start().unwrap();
        let second = sources.recv().unwrap();
        sink.write(AudioPacket::Raw(vec![5; 48]), &mut converter).unwrap();
        sink.stop().unwrap();
        let mut second_bytes = Vec::new();
        let mut second_reader = second.reader;
        second_reader.read_to_end(&mut second_bytes).unwrap();
        assert_eq!(second.id, 1);
        assert_eq!(second_bytes, vec![5; 48]);
    }

    #[test]
    fn bounded_writer_wakes_when_cancelled_while_full() {
        let (mut sink, sources) = channel_sink(1);
        sink.start().unwrap();
        let source = sources.recv().unwrap();
        let cancel = source.cancel.clone();
        let (started, started_rx) = mpsc::channel();
        let writer = thread::spawn(move || {
            started.send(()).unwrap();
            let mut converter = Converter::new(None);
            sink.write(AudioPacket::Raw(vec![7; MAX_CHUNK_BYTES * 2]), &mut converter)
        });

        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        thread::sleep(Duration::from_millis(25));
        cancel.cancel();
        assert!(writer.join().unwrap().is_err());
    }

    #[test]
    fn reader_wakes_when_cancelled_while_empty() {
        let (mut sink, sources) = channel_sink(1);
        sink.start().unwrap();
        let source = sources.recv().unwrap();
        let cancel = source.cancel.clone();
        let mut reader = source.reader;
        let reader_thread = thread::spawn(move || {
            let mut byte = [0];
            reader.read(&mut byte)
        });

        thread::sleep(Duration::from_millis(25));
        cancel.cancel();
        assert_eq!(reader_thread.join().unwrap().unwrap(), 0);
    }

    #[test]
    fn sample_packets_are_encoded_as_little_endian_s16() {
        let (mut sink, sources) = channel_sink(1);
        sink.start().unwrap();
        let source = sources.recv().unwrap();
        let mut converter = Converter::new(None);
        sink.write(AudioPacket::Samples(vec![-0.5, 0.0, 0.5]), &mut converter).unwrap();
        let mut bytes = Vec::new();
        let mut reader = source.reader;
        sink.stop().unwrap();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![0x00, 0xc0, 0x00, 0x00, 0x00, 0x40]);
    }
}
