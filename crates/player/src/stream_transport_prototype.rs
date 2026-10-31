use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use libmpv2::events::{Event, EventContext};
use libmpv2::protocol::{Protocol, ProtocolContext};
use libmpv2::Mpv;

const READ_INTERVAL: Duration = Duration::from_millis(10);

struct PcmInput {
    receiver: Receiver<Vec<u8>>,
    cancelled: Arc<AtomicBool>,
}

struct PcmReader {
    receiver: Receiver<Vec<u8>>,
    cancelled: Arc<AtomicBool>,
    chunk: Vec<u8>,
    offset: usize,
}

type PcmRegistry = Arc<Mutex<HashMap<String, PcmInput>>>;

fn open(registry: &mut PcmRegistry, uri: &str) -> PcmReader {
    let id = uri.strip_prefix("ryotune-pcm://").unwrap();
    let input = registry.lock().unwrap().remove(id).unwrap();
    PcmReader { receiver: input.receiver, cancelled: input.cancelled, chunk: Vec::new(), offset: 0 }
}

fn close(_: Box<PcmReader>) {}

fn read(reader: &mut PcmReader, output: &mut [c_char]) -> i64 {
    if output.is_empty() || reader.cancelled.load(Ordering::Acquire) {
        return 0;
    }
    if reader.offset == reader.chunk.len() {
        reader.chunk = loop {
            if reader.cancelled.load(Ordering::Acquire) {
                return 0;
            }
            match reader.receiver.recv_timeout(READ_INTERVAL) {
                Ok(chunk) => break chunk,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return 0,
            }
        };
        reader.offset = 0;
    }
    let count = output.len().min(reader.chunk.len() - reader.offset);
    for (slot, byte) in output.iter_mut().zip(&reader.chunk[reader.offset..reader.offset + count]) {
        *slot = *byte as c_char;
    }
    reader.offset += count;
    count as i64
}

fn protocol_context(
    mpv: &Mpv,
    registry: PcmRegistry,
) -> ProtocolContext<'_, PcmReader, PcmRegistry> {
    let context = mpv.create_protocol_context();
    let protocol =
        unsafe { Protocol::new("ryotune-pcm".to_owned(), registry, open, close, read, None, None) };
    context.register(protocol).unwrap();
    context
}

fn wait_for_eof(events: &mut EventContext) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(Ok(Event::EndFile(reason))) = events.wait_event(0.1) {
            assert_eq!(reason as i32, 0);
            return;
        }
        assert!(std::time::Instant::now() < deadline, "mpv did not observe PCM EOF");
    }
}

fn load(mpv: &Mpv, id: &str) {
    mpv.command(
        "loadfile",
        &[
            &format!("ryotune-pcm://{id}"),
            "replace",
            "-1",
            "demuxer=rawaudio,demuxer-rawaudio-format=s16le,demuxer-rawaudio-rate=44100,demuxer-rawaudio-channels=2",
        ],
    )
    .unwrap();
}

fn finished_input() -> PcmInput {
    let (sender, receiver) = std::sync::mpsc::sync_channel(2);
    sender.send(vec![0; 17_640]).unwrap();
    drop(sender);
    PcmInput { receiver, cancelled: Arc::new(AtomicBool::new(false)) }
}

#[test]
fn mpv_custom_protocol_reads_pcm_channel_to_eof() {
    let registry = Arc::new(Mutex::new(HashMap::from([("track".to_owned(), finished_input())])));
    let mpv = Mpv::with_initializer(|initializer| {
        initializer.set_property("ao", "null")?;
        initializer.set_property("vid", "no")?;
        initializer.set_property("terminal", "no")?;
        Ok(())
    })
    .unwrap();
    let _protocols = protocol_context(&mpv, registry);
    let mut events = EventContext::new(mpv.ctx);

    load(&mpv, "track");
    wait_for_eof(&mut events);
}

#[test]
fn pcm_reader_wakes_when_stream_is_cancelled() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut reader =
        PcmReader { receiver, cancelled: cancelled.clone(), chunk: Vec::new(), offset: 0 };
    let reader_thread = std::thread::spawn(move || {
        let mut output = [0];
        read(&mut reader, &mut output)
    });
    std::thread::sleep(Duration::from_millis(25));
    cancelled.store(true, Ordering::Release);
    drop(sender);
    assert_eq!(reader_thread.join().unwrap(), 0);
}
