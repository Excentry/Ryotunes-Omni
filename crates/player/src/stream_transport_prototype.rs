use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use libmpv2::events::{Event, EventContext};
use libmpv2::protocol::{Protocol, ProtocolContext};
use libmpv2::Mpv;

struct PcmReader {
    receiver: Receiver<Vec<u8>>,
    chunk: Vec<u8>,
    offset: usize,
}

type PcmRegistry = Arc<Mutex<HashMap<String, Receiver<Vec<u8>>>>>;

fn open(registry: &mut PcmRegistry, uri: &str) -> PcmReader {
    let id = uri.strip_prefix("ryotune-pcm://").unwrap();
    PcmReader {
        receiver: registry.lock().unwrap().remove(id).unwrap(),
        chunk: Vec::new(),
        offset: 0,
    }
}

fn close(_: Box<PcmReader>) {}

fn read(reader: &mut PcmReader, output: &mut [c_char]) -> i64 {
    if reader.offset == reader.chunk.len() {
        reader.chunk = match reader.receiver.recv() {
            Ok(chunk) => chunk,
            Err(_) => return 0,
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

#[test]
fn mpv_custom_protocol_reads_bounded_pcm_channel_to_eof() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(2);
    sender.send(vec![0; 17_640]).unwrap();
    drop(sender);
    let registry = Arc::new(Mutex::new(HashMap::from([("track".to_owned(), receiver)])));

    let mpv = Mpv::with_initializer(|initializer| {
        initializer.set_property("ao", "null")?;
        initializer.set_property("vid", "no")?;
        initializer.set_property("terminal", "no")?;
        Ok(())
    })
    .unwrap();
    let _protocols = protocol_context(&mpv, registry);
    let mut events = EventContext::new(mpv.ctx);
    mpv.command(
        "loadfile",
        &[
            "ryotune-pcm://track",
            "replace",
            "-1",
            "demuxer=rawaudio,demuxer-rawaudio-format=s16le,demuxer-rawaudio-rate=44100,demuxer-rawaudio-channels=2",
        ],
    )
    .unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(Ok(Event::EndFile(reason))) = events.wait_event(0.1) {
            assert_eq!(reason as i32, 0);
            break;
        }
        assert!(std::time::Instant::now() < deadline, "mpv did not observe PCM EOF");
    }
}
