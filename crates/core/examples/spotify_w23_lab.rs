#[cfg(not(windows))]
fn main() {
    eprintln!("The W23 Spotify transport lab runs on Windows only.");
}

#[cfg(windows)]
mod windows_lab {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::io::{self, BufRead, Read, Write};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use anyhow::{bail, ensure, Context as _, Result};
    use player::{Player, PlayerEvent};
    use ryotunes_spotify::w23_transport_lab::{LabCancellation, SpotifyTransportLab};
    use ryotunes_spotify::SpotifyProvider;

    const NETWORK_TIMEOUT_SECONDS: f64 = 300.0;
    const QUEUE_CHUNKS: usize = 64;
    const MAX_REQUEST_BYTES: usize = 8192;
    const PIPE_BUFFER_BYTES: u32 = 65_536;
    const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x0000_0008;
    const PIPE_TYPE_BYTE: u32 = 0;
    const PIPE_READMODE_BYTE: u32 = 0;
    const PIPE_WAIT: u32 = 0;
    const PIPE_ACCESS_OUTBOUND: u32 = 0x0000_0002;
    const ERROR_PIPE_CONNECTED: u32 = 535;
    const ERROR_OPERATION_ABORTED: u32 = 995;
    const ERROR_OPERATION_ABORTED_CODE: i32 = 995;
    const ERROR_BROKEN_PIPE: i32 = 109;
    const WSAECONNABORTED: i32 = 10053;
    const WSAECONNRESET: i32 = 10054;
    const WSAENOTCONN: i32 = 10057;

    type Handle = *mut c_void;

    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;

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

    unsafe impl Send for PipeHandle {}

    impl PipeHandle {
        fn create(name: &[u16]) -> Result<Self> {
            let handle = unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_OUTBOUND,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    1,
                    PIPE_BUFFER_BYTES,
                    0,
                    0,
                    std::ptr::null_mut(),
                )
            };
            ensure!(handle != INVALID_HANDLE_VALUE, "CreateNamedPipeW failed: {}", unsafe {
                GetLastError()
            });
            Ok(Self(handle))
        }

        fn connect(&self) -> Result<bool> {
            if unsafe { ConnectNamedPipe(self.0, std::ptr::null_mut()) } != 0 {
                return Ok(true);
            }
            let error = unsafe { GetLastError() };
            if error == ERROR_OPERATION_ABORTED {
                return Ok(false);
            }
            ensure!(error == ERROR_PIPE_CONNECTED, "ConnectNamedPipe failed: {error}");
            Ok(true)
        }
    }

    impl Write for PipeHandle {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.is_empty() {
                return Ok(0);
            }
            let length = bytes.len().min(u32::MAX as usize) as u32;
            let mut written = 0;
            let success = unsafe {
                WriteFile(self.0, bytes.as_ptr().cast(), length, &mut written, std::ptr::null_mut())
            };
            if success == 0 {
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

    struct Bridge {
        cancel: Arc<AtomicBool>,
        thread: Option<JoinHandle<Result<()>>>,
        pipe: bool,
        source_cancel: Option<LabCancellation>,
    }

    impl Bridge {
        fn stop(&mut self) -> Result<()> {
            self.cancel.store(true, Ordering::Release);
            if let Some(source_cancel) = &self.source_cancel {
                source_cancel.cancel();
            }
            if self.pipe {
                if let Some(thread) = &self.thread {
                    while !thread.is_finished() {
                        unsafe {
                            CancelSynchronousIo(thread.as_raw_handle() as Handle);
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            if let Some(thread) = self.thread.take() {
                let result =
                    thread.join().map_err(|_| anyhow::anyhow!("PCM bridge thread panicked"))?;
                if let Err(error) = result {
                    let cancelled = self.cancel.load(Ordering::Acquire);
                    let expected = error
                        .downcast_ref::<io::Error>()
                        .and_then(io::Error::raw_os_error)
                        .is_some_and(|code| {
                            code == ERROR_OPERATION_ABORTED_CODE
                                || code == ERROR_BROKEN_PIPE
                                || code == WSAECONNABORTED
                                || code == WSAECONNRESET
                                || code == WSAENOTCONN
                        });
                    if !cancelled || !expected {
                        return Err(error);
                    }
                }
            }
            Ok(())
        }
    }

    impl Drop for Bridge {
        fn drop(&mut self) {
            let _ = self.stop();
        }
    }

    use std::os::windows::io::AsRawHandle;

    static LAB_CLOCK: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

    fn token() -> String {
        let random = rand::random::<[u8; 32]>();
        random.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn authorized_request(request: &[u8], secret: &str) -> bool {
        let first_line = request
            .split(|byte| *byte == b'\n')
            .next()
            .map(|line| String::from_utf8_lossy(line).trim().to_owned())
            .unwrap_or_default();
        first_line == format!("GET /{secret} HTTP/1.1")
            || first_line == format!("GET /{secret} HTTP/1.0")
    }

    fn accept_cancellable(
        listener: &TcpListener,
        cancel: &AtomicBool,
    ) -> io::Result<Option<(TcpStream, SocketAddr)>> {
        while !cancel.load(Ordering::Acquire) {
            match listener.accept() {
                Ok(connection) => return Ok(Some(connection)),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }

    fn mpv_result<T>(result: std::result::Result<T, player::Error>) -> Result<T> {
        result.map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    fn tcp_bridge<R: Read + Send + 'static>(
        mut pcm: R,
        secret: String,
        source_cancel: Option<LabCancellation>,
    ) -> Result<(String, Bridge)> {
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let worker_secret = secret.clone();
        let thread = thread::Builder::new()
            .name("w23-tcp-pcm-bridge".to_owned())
            .spawn(move || {
                while !worker_cancel.load(Ordering::Acquire) {
                    match accept_cancellable(&listener, &worker_cancel)? {
                        None => return Ok(()),
                        Some((mut client, peer)) => {
                            client.set_nonblocking(false)?;
                            if !peer.ip().is_loopback() {
                                let _ = client.write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
                                continue;
                            }
                            client.set_read_timeout(Some(Duration::from_secs(5)))?;
                            let mut request = Vec::with_capacity(1024);
                            let mut byte = [0; 1];
                            while request.len() < MAX_REQUEST_BYTES && !request.ends_with(b"\r\n\r\n") {
                                match client.read(&mut byte) {
                                    Ok(0) => break,
                                    Ok(_) => request.push(byte[0]),
                                    Err(error) if error.kind() == io::ErrorKind::WouldBlock
                                        || error.kind() == io::ErrorKind::TimedOut => break,
                                    Err(error) => return Err(error.into()),
                                }
                            }
                            if !authorized_request(&request, &worker_secret) {
                                let _ = client.write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
                                continue;
                            }
                            client.write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                            )?;
                            io::copy(&mut pcm, &mut client)?;
                            return Ok(());
                        }
                    }
                }
                Ok(())
            })?;
        Ok((
            format!("http://{address}/{secret}"),
            Bridge { cancel, thread: Some(thread), pipe: false, source_cancel },
        ))
    }

    fn pipe_bridge<R: Read + Send + 'static>(
        pcm: R,
        source_cancel: Option<LabCancellation>,
    ) -> Result<(String, Bridge)> {
        let suffix = token();
        let name = format!("ryotune-w23-{}-{suffix}", std::process::id());
        let server_path = format!(r"\\.\pipe\{name}");
        let client_path = format!(r"\\?\pipe\{name}");
        let wide_name: Vec<u16> = server_path.encode_utf16().chain(std::iter::once(0)).collect();
        let pipe = PipeHandle::create(&wide_name)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let thread = thread::Builder::new().name("w23-named-pipe-pcm-bridge".to_owned()).spawn(
            move || {
                if !pipe.connect()? {
                    return Ok(());
                }
                let mut pipe = pipe;
                let mut pcm = pcm;
                io::copy(&mut pcm, &mut pipe)?;
                pipe.flush()?;
                if worker_cancel.load(Ordering::Acquire) {
                    return Ok(());
                }
                Ok(())
            },
        )?;
        Ok((client_path, Bridge { cancel, thread: Some(thread), pipe: true, source_cancel }))
    }

    fn usage() -> ! {
        eprintln!("Usage: cargo run --locked -p ryotunes-core --example spotify_w23_lab -- <tcp|named-pipe> <track-id-or-uri> <data-dir>");
        eprintln!("       cargo run --locked -p ryotunes-core --example spotify_w23_lab -- --dead-listener <port> <data-dir>");
        std::process::exit(2)
    }

    async fn probe_dead_listener(port: u16, data_dir: PathBuf) -> Result<()> {
        let cache_dir = data_dir.join("mpv-cache-dead-listener");
        std::fs::create_dir_all(&cache_dir)?;
        let cache_dir = cache_dir.to_string_lossy().into_owned();
        let mut player =
            mpv_result(Player::new_with_network_timeout(&cache_dir, NETWORK_TIMEOUT_SECONDS))?;
        let mut events = player.take_events().context("libmpv event stream unavailable")?;
        let url = format!("http://127.0.0.1:{port}/{}", token());
        let options = [
            "demuxer=rawaudio".to_owned(),
            "demuxer-rawaudio-format=s16le".to_owned(),
            "demuxer-rawaudio-rate=44100".to_owned(),
            "demuxer-rawaudio-channels=2".to_owned(),
        ];
        mpv_result(player.load(
            &url,
            &HashMap::new(),
            None,
            "W23 closed-listener probe",
            &options,
        ))?;
        println!("dead_listener_url={url} network_timeout_seconds={NETWORK_TIMEOUT_SECONDS}");
        let started = Instant::now();
        let deadline = tokio::time::sleep(Duration::from_secs(310));
        tokio::pin!(deadline);
        let mut report = tokio::time::interval(Duration::from_secs(10));
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Some(PlayerEvent::TrackFailed(error)) => {
                        println!("dead_listener_result=error elapsed_ms={} error={error}", started.elapsed().as_millis());
                        return Ok(());
                    }
                    Some(PlayerEvent::TrackEnded) => {
                        println!("dead_listener_result=unexpected_eof elapsed_ms={}", started.elapsed().as_millis());
                        return Ok(());
                    }
                    Some(event) => println!("dead_listener_event={event:?}"),
                    None => bail!("libmpv event stream closed before reporting the failed connection"),
                },
                _ = report.tick() => println!("dead_listener_wait_seconds={}", started.elapsed().as_secs()),
                _ = &mut deadline => bail!("no visible libmpv failure after {} seconds", started.elapsed().as_secs()),
            }
        }
    }

    #[tokio::main]
    async fn async_main() -> Result<()> {
        let _ = LAB_CLOCK.elapsed();
        let mut args = std::env::args().skip(1);
        let first = args.next().unwrap_or_else(|| usage());
        if first == "--dead-listener" {
            let port = args.next().unwrap_or_else(|| usage()).parse::<u16>()?;
            let data_dir = args.next().map(PathBuf::from).unwrap_or_else(|| usage());
            if args.next().is_some() {
                usage();
            }
            return probe_dead_listener(port, data_dir).await;
        }
        let transport = first;
        let track = args.next().unwrap_or_else(|| usage());
        let data_dir = args.next().map(PathBuf::from).unwrap_or_else(|| usage());
        if args.next().is_some() || !matches!(transport.as_str(), "tcp" | "named-pipe") {
            usage();
        }
        std::fs::create_dir_all(&data_dir)?;
        let provider = SpotifyProvider::new(data_dir.clone());
        let session = match provider.restore_session().await? {
            Some(session) => session,
            None => provider
                .sign_in_session(|url| println!("Open this Spotify authorization URL: {url}"))
                .await
                .context("Spotify Premium sign-in failed")?,
        };
        let lab = SpotifyTransportLab::new(session, QUEUE_CHUNKS)?;
        let stream = lab.stream(&track)?;
        let cancellation = stream.cancellation();
        let id = stream.id;
        let mut metrics = stream.metrics_handle();
        let mut event_rx = stream.events;
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                println!("elapsed_ms={} spotify-event={event:?}", LAB_CLOCK.elapsed().as_millis());
            }
        });
        let pcm = stream.pcm;
        let (url, mut bridge) = match transport.as_str() {
            "tcp" => tcp_bridge(pcm, token(), Some(cancellation.clone()))?,
            "named-pipe" => pipe_bridge(pcm, Some(cancellation.clone()))?,
            _ => unreachable!(),
        };
        let cache_dir = data_dir.join("mpv-cache");
        std::fs::create_dir_all(&cache_dir)?;
        let cache_dir = cache_dir.to_string_lossy().into_owned();
        let mut player =
            mpv_result(Player::new_with_network_timeout(&cache_dir, NETWORK_TIMEOUT_SECONDS))?;
        let events = player.take_events().context("libmpv event stream unavailable")?;
        tokio::spawn(async move {
            let mut events = events;
            while let Some(event) = events.recv().await {
                println!("elapsed_ms={} mpv-event={event:?}", LAB_CLOCK.elapsed().as_millis());
            }
        });
        let options = [
            "demuxer=rawaudio".to_owned(),
            "demuxer-rawaudio-format=s16le".to_owned(),
            "demuxer-rawaudio-rate=44100".to_owned(),
            "demuxer-rawaudio-channels=2".to_owned(),
            "demuxer-max-bytes=2097152".to_owned(),
            "demuxer-max-back-bytes=1048576".to_owned(),
        ];
        mpv_result(player.load(
            &url,
            &HashMap::new(),
            None,
            "Spotify W23 transport lab",
            &options,
        ))?;
        println!("transport={transport} track={track} stream_id={id} network_timeout_seconds={NETWORK_TIMEOUT_SECONDS} queue_chunks={QUEUE_CHUNKS} cache_bytes=2097152 cache_back_bytes=1048576");
        println!("Commands: load <track-id-or-uri> | pause | resume | seek <seconds> | metrics | stop | quit");
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = line?;
            let mut parts = line.split_whitespace();
            match parts.next().unwrap_or_default() {
                "pause" => mpv_result(player.pause())?,
                "resume" => mpv_result(player.play())?,
                "seek" => {
                    let seconds = parts.next().context("seek requires seconds")?.parse::<f64>()?;
                    mpv_result(player.seek(seconds))?;
                }
                "load" => {
                    let next_track =
                        parts.next().context("load requires a Spotify track id or URI")?;
                    mpv_result(player.stop())?;
                    bridge.stop()?;
                    let next_stream = lab.stream(next_track)?;
                    let next_cancel = next_stream.cancellation();
                    let next_id = next_stream.id;
                    metrics = next_stream.metrics_handle();
                    let mut next_events = next_stream.events;
                    tokio::spawn(async move {
                        while let Some(event) = next_events.recv().await {
                            println!(
                                "elapsed_ms={} spotify-event={event:?}",
                                LAB_CLOCK.elapsed().as_millis()
                            );
                        }
                    });
                    let (next_url, next_bridge) = match transport.as_str() {
                        "tcp" => tcp_bridge(next_stream.pcm, token(), Some(next_cancel.clone()))?,
                        "named-pipe" => pipe_bridge(next_stream.pcm, Some(next_cancel.clone()))?,
                        _ => unreachable!(),
                    };
                    bridge = next_bridge;
                    mpv_result(player.load(
                        &next_url,
                        &HashMap::new(),
                        None,
                        "Spotify W23 transport lab",
                        &options,
                    ))?;
                    println!(
                        "elapsed_ms={} loaded_track={next_track} stream_id={next_id}",
                        LAB_CLOCK.elapsed().as_millis()
                    );
                }
                "metrics" => println!("pcm_metrics={:?}", metrics.snapshot()),
                "stop" | "quit" => {
                    mpv_result(player.stop())?;
                    cancellation.cancel();
                    break;
                }
                "" => {}
                command => eprintln!("Unknown command: {command}"),
            }
        }
        let cleanup_started = Instant::now();
        cancellation.cancel();
        bridge.stop()?;
        println!(
            "elapsed_ms={} cleanup_ms={}",
            LAB_CLOCK.elapsed().as_millis(),
            cleanup_started.elapsed().as_millis()
        );
        Ok(())
    }

    pub fn run() {
        if let Err(error) = async_main() {
            eprintln!("W23 lab failed: {error:#}");
            std::process::exit(1);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{accept_cancellable, authorized_request, pipe_bridge, tcp_bridge};
        use std::io::{Cursor, Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::thread;
        use std::time::{Duration, Instant};

        #[test]
        fn tcp_bridge_rejects_unauthorized_requests() {
            assert!(authorized_request(b"GET /secret HTTP/1.1\r\n\r\n", "secret"));
            assert!(!authorized_request(b"GET /other HTTP/1.1\r\n\r\n", "secret"));
            assert!(!authorized_request(b"POST /secret HTTP/1.1\r\n\r\n", "secret"));
        }

        #[test]
        fn blocked_tcp_accept_is_cancellable() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            let worker = thread::spawn(move || accept_cancellable(&listener, &worker_cancel));
            thread::sleep(Duration::from_millis(30));
            let started = Instant::now();
            cancel.store(true, Ordering::Release);
            assert!(worker.join().unwrap().unwrap().is_none());
            assert!(started.elapsed() < Duration::from_millis(500));
        }

        #[test]
        fn tcp_http_bridge_rejects_then_streams_pcm() {
            let body = b"synthetic-pcm-payload".to_vec();
            let (url, mut bridge) =
                tcp_bridge(Cursor::new(body.clone()), "test-token".to_owned(), None).unwrap();
            let address = url.strip_prefix("http://").unwrap().split('/').next().unwrap();
            let mut client = TcpStream::connect(address).unwrap();
            client.write_all(b"GET /wrong HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut rejected = Vec::new();
            client.read_to_end(&mut rejected).unwrap();
            assert!(rejected.starts_with(b"HTTP/1.1 403 Forbidden\r\n"));

            let mut client = TcpStream::connect(address).unwrap();
            client.write_all(b"GET /test-token HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut response = Vec::new();
            client.read_to_end(&mut response).unwrap();
            let split = response.windows(4).position(|window| window == b"\r\n\r\n").unwrap() + 4;
            assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
            assert_eq!(&response[split..], body);
            bridge.stop().unwrap();
        }

        #[test]
        fn cancelling_pending_named_pipe_connect_retries_until_worker_exits() {
            for _ in 0..200 {
                let (_, mut bridge) = pipe_bridge(Cursor::new(Vec::<u8>::new()), None).unwrap();
                bridge.stop().unwrap();
            }
        }

        #[test]
        fn cancelling_blocked_named_pipe_write_is_expected_cleanup() {
            use std::fs::File;
            use std::thread;
            use std::time::Duration;

            let payload = vec![0_u8; 2 * 1024 * 1024];
            let (path, mut bridge) = pipe_bridge(Cursor::new(payload), None).unwrap();
            let reader = thread::spawn(move || File::open(path).unwrap());
            let _reader = reader.join().unwrap();
            thread::sleep(Duration::from_millis(50));
            bridge.stop().unwrap();
        }
    }
}

#[cfg(windows)]
fn main() {
    windows_lab::run();
}
