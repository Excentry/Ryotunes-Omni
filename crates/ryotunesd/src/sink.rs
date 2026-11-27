//! `EventSink` over the socket: every event goes to every connection that subscribed.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use ryotunes_core::host::EventSink;
use ryotunes_protocol::{Event, Outgoing};
use serde_json::Value;
use tokio::sync::{mpsc, watch};

#[derive(Clone)]
pub struct ClientWriter {
    tx: mpsc::Sender<String>,
    overloaded: watch::Sender<bool>,
}

impl ClientWriter {
    pub fn new(tx: mpsc::Sender<String>, overloaded: watch::Sender<bool>) -> Self {
        Self { tx, overloaded }
    }

    pub fn send(&self, line: String) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.tx.try_send(line) {
            self.overloaded.send_replace(true);
        }
    }

    pub async fn closed(&self) {
        self.tx.closed().await;
    }

    fn try_send_event(&self, line: String) -> bool {
        match self.tx.try_send(line) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.overloaded.send_replace(true);
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    fn is_closed(&self) -> bool {
        self.tx.is_closed() || *self.overloaded.borrow()
    }
}

#[derive(Default)]
pub struct SocketSink {
    subscribers: Mutex<Vec<ClientWriter>>,
    count: AtomicUsize,
}

impl SocketSink {
    pub fn subscribe(&self, tx: ClientWriter) {
        self.subscribers.lock().unwrap().push(tx);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Live subscribers, after dropping the ones whose connection has gone. A client killed
    /// outright leaves its sender open until the next emit would find it closed; `show` reads
    /// this to decide between raising a client and launching one, so it must not count ghosts
    /// (a stale count kept the daemon emitting `show` to nobody instead of spawning the client).
    pub fn subscriber_count(&self) -> usize {
        let mut subs = self.subscribers.lock().unwrap();
        self.prune(&mut subs);
        self.count.load(Ordering::Relaxed)
    }

    fn prune(&self, subs: &mut Vec<ClientWriter>) {
        let before = subs.len();
        subs.retain(|tx| !tx.is_closed());
        let dropped = before - subs.len();
        if dropped > 0 {
            self.count.fetch_sub(dropped, Ordering::Relaxed);
        }
    }
}

impl EventSink for SocketSink {
    fn emit(&self, event: &'static str, payload: Value) {
        let line = Outgoing::Event(Event { event: event.to_owned(), data: payload }).to_line();
        let mut subs = self.subscribers.lock().unwrap();
        self.prune(&mut subs);
        let before = subs.len();
        subs.retain(|tx| tx.try_send_event(line.clone()));
        let dropped = before - subs.len();
        if dropped > 0 {
            self.count.fetch_sub(dropped, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientWriter, SocketSink};
    use ryotunes_core::host::EventSink;

    #[test]
    fn full_subscriber_queue_is_bounded_and_marked_for_disconnect() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let (overloaded, overload_rx) = tokio::sync::watch::channel(false);
        let sink = SocketSink::default();
        sink.subscribe(ClientWriter::new(tx, overloaded));

        sink.emit("position", serde_json::json!({ "position": 1 }));
        assert_eq!(sink.subscriber_count(), 1);

        sink.emit("position", serde_json::json!({ "position": 2 }));
        assert_eq!(sink.subscriber_count(), 0);
        assert!(*overload_rx.borrow());
    }
}
