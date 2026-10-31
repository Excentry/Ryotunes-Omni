use std::sync::mpsc::{self, Receiver, SyncSender};

use librespot_playback::audio_backend::{Sink, SinkError, SinkResult};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;

const MAX_CHUNK_BYTES: usize = 16 * 1024;

struct ChannelSink {
    sender: Option<SyncSender<Vec<u8>>>,
}

fn channel_sink(capacity: usize) -> (ChannelSink, Receiver<Vec<u8>>) {
    let (sender, receiver) = mpsc::sync_channel(capacity);
    (ChannelSink { sender: Some(sender) }, receiver)
}

impl Sink for ChannelSink {
    fn stop(&mut self) -> SinkResult<()> {
        self.sender.take();
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
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| SinkError::OnWrite("PCM channel is closed".to_owned()))?;
        for chunk in bytes.chunks(MAX_CHUNK_BYTES) {
            sender
                .send(chunk.to_vec())
                .map_err(|_| SinkError::OnWrite("PCM reader disconnected".to_owned()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use librespot_playback::audio_backend::Sink;
    use librespot_playback::convert::Converter;
    use librespot_playback::decoder::AudioPacket;

    use super::{MAX_CHUNK_BYTES, channel_sink};

    #[test]
    fn bounded_channel_backpressures_and_closes_at_stop() {
        let (mut sink, reader) = channel_sink(1);
        let (finished, completion) = mpsc::channel();
        let writer = thread::spawn(move || {
            let mut converter = Converter::new(None);
            sink.write(AudioPacket::Raw(vec![7; MAX_CHUNK_BYTES * 2]), &mut converter).unwrap();
            finished.send(()).unwrap();
            sink.stop().unwrap();
        });

        assert!(completion.recv_timeout(Duration::from_millis(50)).is_err());
        assert_eq!(reader.recv().unwrap(), vec![7; MAX_CHUNK_BYTES]);
        assert_eq!(reader.recv().unwrap(), vec![7; MAX_CHUNK_BYTES]);
        completion.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(reader.recv().is_err());
        writer.join().unwrap();
    }

    #[test]
    fn sample_packets_are_encoded_as_little_endian_s16() {
        let (mut sink, reader) = channel_sink(1);
        let mut converter = Converter::new(None);
        sink.write(AudioPacket::Samples(vec![-0.5, 0.0, 0.5]), &mut converter).unwrap();
        let bytes = reader.recv().unwrap();
        assert_eq!(bytes, vec![0x00, 0xc0, 0x00, 0x00, 0x00, 0x40]);
    }
}
