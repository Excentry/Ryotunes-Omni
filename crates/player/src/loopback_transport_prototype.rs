use std::io::{self, Read};
use std::net::TcpListener;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use libmpv2::events::{Event, EventContext};
use libmpv2::Mpv;

struct QueueReader {
    receiver: Receiver<Vec<u8>>,
    chunk: Vec<u8>,
    offset: usize,
}

impl Read for QueueReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            if self.offset < self.chunk.len() {
                let count = output.len().min(self.chunk.len() - self.offset);
                output[..count].copy_from_slice(&self.chunk[self.offset..self.offset + count]);
                self.offset += count;
                return Ok(count);
            }
            match self.receiver.recv() {
                Ok(chunk) => {
                    self.chunk = chunk;
                    self.offset = 0;
                }
                Err(_) => return Ok(0),
            }
        }
    }
}

fn wait_for_eof(events: &mut EventContext) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match events.wait_event(0.1) {
            Some(Ok(Event::EndFile(reason))) => {
                assert_eq!(reason as i32, 0);
                return;
            }
            Some(Err(error)) => panic!("mpv failed to read loopback stream: {error}"),
            Some(Ok(_)) | None => {}
        }
        assert!(std::time::Instant::now() < deadline, "mpv did not observe loopback EOF");
    }
}

#[test]
fn mpv_reads_synthetic_raw_pcm_from_loopback_tcp_to_eof() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = std::sync::mpsc::sync_channel(2);
    sender.send(vec![0; 17_640]).unwrap();
    drop(sender);
    let writer = thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        let mut reader = QueueReader { receiver, chunk: Vec::new(), offset: 0 };
        io::copy(&mut reader, &mut connection).unwrap();
    });

    let mpv = Mpv::with_initializer(|initializer| {
        initializer.set_property("ao", "null")?;
        initializer.set_property("vid", "no")?;
        initializer.set_property("terminal", "no")?;
        Ok(())
    })
    .unwrap();
    let mut events = EventContext::new(mpv.ctx);
    let url = format!("tcp://{address}");
    mpv.command(
        "loadfile",
        &[
            &super::quoted(&url),
            "replace",
            "-1",
            "demuxer=rawaudio,demuxer-rawaudio-format=s16le,demuxer-rawaudio-rate=44100,demuxer-rawaudio-channels=2",
        ],
    )
    .unwrap();
    wait_for_eof(&mut events);
    writer.join().unwrap();
}
