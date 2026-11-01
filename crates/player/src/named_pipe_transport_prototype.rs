use std::ffi::c_void;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use libmpv2::events::{Event, EventContext};
use libmpv2::Mpv;

type Handle = *mut c_void;

const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const PIPE_ACCESS_OUTBOUND: u32 = 0x00000002;
const PIPE_TYPE_BYTE: u32 = 0x00000000;
const PIPE_READMODE_BYTE: u32 = 0x00000000;
const PIPE_WAIT: u32 = 0x00000000;
const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x00000008;
const ERROR_PIPE_CONNECTED: u32 = 535;
const ERROR_OPERATION_ABORTED: i32 = 995;
const ERROR_NOT_FOUND: u32 = 1168;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateNamedPipeW(
        name: *const u16,
        open_mode: u32,
        pipe_mode: u32,
        max_instances: u32,
        out_buffer_size: u32,
        in_buffer_size: u32,
        default_timeout: u32,
        security_attributes: *mut c_void,
    ) -> Handle;
    fn ConnectNamedPipe(pipe: Handle, overlapped: *mut c_void) -> i32;
    fn WriteFile(
        file: Handle,
        buffer: *const c_void,
        bytes_to_write: u32,
        bytes_written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn FlushFileBuffers(file: Handle) -> i32;
    fn DisconnectNamedPipe(pipe: Handle) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn CancelSynchronousIo(thread: Handle) -> i32;
    fn GetLastError() -> u32;
}

struct PipeHandle(Handle);

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

impl PipeHandle {
    fn create(name: &[u16]) -> Result<Self, String> {
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_OUTBOUND,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                65_536,
                0,
                50,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("CreateNamedPipeW failed: {}", unsafe { GetLastError() }));
        }
        Ok(Self(handle))
    }

    fn connect(&self) -> Result<(), String> {
        let result = unsafe { ConnectNamedPipe(self.0, std::ptr::null_mut()) };
        if result != 0 {
            return Ok(());
        }
        let error = unsafe { GetLastError() };
        if error == ERROR_PIPE_CONNECTED {
            Ok(())
        } else {
            Err(format!("ConnectNamedPipe failed: {error}"))
        }
    }
}

impl Write for PipeHandle {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let length = bytes.len().min(u32::MAX as usize) as u32;
        let mut written = 0;
        if unsafe {
            WriteFile(self.0, bytes.as_ptr().cast(), length, &mut written, std::ptr::null_mut())
        } == 0
        {
            return Err(io::Error::from_raw_os_error(unsafe { GetLastError() } as i32));
        }
        Ok(written as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        if unsafe { FlushFileBuffers(self.0) } == 0 {
            return Err(io::Error::from_raw_os_error(unsafe { GetLastError() } as i32));
        }
        Ok(())
    }
}

impl Drop for PipeHandle {
    fn drop(&mut self) {
        unsafe {
            DisconnectNamedPipe(self.0);
            CloseHandle(self.0);
        }
    }
}

fn wait_for_eof(events: &mut EventContext, log_path: &Path) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match events.wait_event(0.1) {
            Some(Ok(Event::EndFile(reason))) => {
                assert_eq!(reason as i32, 0);
                return;
            }
            Some(Err(error)) => panic!(
                "mpv failed to read named-pipe stream: {error}; log: {}",
                std::fs::read_to_string(log_path).unwrap_or_default()
            ),
            Some(Ok(event)) => eprintln!("mpv event: {event:?}"),
            None => {}
        }
        assert!(
            std::time::Instant::now() < deadline,
            "mpv did not observe named-pipe EOF; log: {}",
            std::fs::read_to_string(log_path).unwrap_or_default()
        );
    }
}

#[test]
fn mpv_reads_raw_pcm_from_a_windows_named_pipe_to_eof() {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = format!(r"\\.\pipe\ryotune-omni-w23-{}-{id}", std::process::id());
    let name: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let (pcm_sender, pcm_receiver) = std::sync::mpsc::sync_channel(2);
    pcm_sender.send(vec![0; 17_640]).unwrap();
    drop(pcm_sender);
    let (ready, ready_rx) = std::sync::mpsc::channel();
    let (connected, connected_rx) = std::sync::mpsc::channel();
    let writer = thread::spawn(move || {
        let pipe = PipeHandle::create(&name)?;
        ready.send(()).unwrap();
        pipe.connect()?;
        connected.send(()).unwrap();
        let mut pipe = pipe;
        let mut reader = QueueReader { receiver: pcm_receiver, chunk: Vec::new(), offset: 0 };
        io::copy(&mut reader, &mut pipe).map_err(|error| error.to_string())?;
        pipe.flush().map_err(|error| error.to_string())
    });
    ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    let log_path = std::env::temp_dir().join(format!("ryotune-w23-mpv-{}.log", std::process::id()));
    let log_path_string = log_path.to_string_lossy().into_owned();
    let mpv = Mpv::with_initializer(|initializer| {
        initializer.set_property("ao", "null")?;
        initializer.set_property("vid", "no")?;
        initializer.set_property("terminal", "no")?;
        initializer.set_option("log-file", log_path_string.as_str())?;
        initializer.set_option("msg-level", "all=debug")?;
        Ok(())
    })
    .unwrap();
    let mut events = EventContext::new(mpv.ctx);
    mpv.command(
        "loadfile",
        &[
            &super::quoted(&path),
            "replace",
            "-1",
            "demuxer=rawaudio,demuxer-rawaudio-format=s16le,demuxer-rawaudio-rate=44100,demuxer-rawaudio-channels=2",
        ],
    )
    .unwrap();
    connected_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("libmpv did not connect to the named pipe");
    wait_for_eof(&mut events, &log_path);
    writer.join().unwrap().unwrap();
}

#[test]
fn blocked_named_pipe_connect_can_be_cancelled() {
    use std::os::windows::io::AsRawHandle;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = format!(r"\\.\pipe\ryotune-omni-w23-cancel-{}-{id}", std::process::id());
    let name: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let (created, created_rx) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let pipe = PipeHandle::create(&name)?;
        created.send(()).unwrap();
        pipe.connect()
    });
    created_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        if unsafe { CancelSynchronousIo(server.as_raw_handle() as Handle) } != 0 {
            break;
        }
        let error = unsafe { GetLastError() };
        assert_eq!(error, ERROR_NOT_FOUND);
        assert!(std::time::Instant::now() < deadline, "ConnectNamedPipe was not cancellable");
        thread::sleep(Duration::from_millis(2));
    }

    let result = server.join().unwrap();
    assert!(result.unwrap_err().contains(&ERROR_OPERATION_ABORTED.to_string()));
}
