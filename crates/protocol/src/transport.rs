//! The transport abstraction (GDD 33.3.1) and its in-process implementation.
//!
//! Single-player runs through the same trait as multiplayer: a listen server with no remote
//! clients, whose "network" is [`LoopbackTransport`]. It serialises nothing itself but it does
//! copy every message and enforce the payload limits of the real transports, so the protocol and
//! prediction code never meets a message a browser would have refused.
//!
//! A replacement (WebRTC data channels, WebTransport, WebSocket) must pass [`contract`].

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

/// Largest unreliable message, bytes: below the IPv6 minimum MTU of 1 280 less IP, UDP, DTLS and
/// SCTP overhead, and over VPN overhead.
pub const MAX_UNRELIABLE_BYTES: usize = 1_200;
/// Largest reliable message, bytes: the chunk size that fits every browser's SCTP limits. Larger
/// payloads are fragmented by the application.
pub const MAX_RELIABLE_BYTES: usize = 16 * 1024;

/// Identifies the other end of a link, as seen by the endpoint that owns this id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId(pub u32);

/// Which of the two channels a message travelled on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// State and inputs: may be dropped or reordered.
    Unreliable,
    /// Events, commands, chat: ordered and delivered.
    Reliable,
}

/// What [`Transport::poll`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportEvent {
    /// A peer is now reachable.
    Connected(PeerId),
    /// A peer is gone. Messages it sent before are delivered first.
    Disconnected(PeerId),
    /// A message arrived intact.
    Message {
        /// Who sent it.
        peer: PeerId,
        /// Which channel it used.
        channel: Channel,
        /// The payload.
        bytes: Vec<u8>,
    },
}

/// Counters for one link.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinkStats {
    /// Round-trip time, milliseconds (zero for loopback).
    pub rtt_ms: f32,
    /// Messages accepted for sending.
    pub messages_sent: u64,
    /// Messages handed to the application by `poll`.
    pub messages_received: u64,
    /// Payload bytes accepted for sending.
    pub bytes_sent: u64,
    /// Payload bytes handed to the application.
    pub bytes_received: u64,
    /// Messages refused because they exceeded the limit of their channel.
    pub oversize_drops: u64,
}

/// A bidirectional link to one or more peers.
pub trait Transport {
    /// Sends a message that may be lost or reordered; at most [`MAX_UNRELIABLE_BYTES`].
    fn send_unreliable(&mut self, peer: PeerId, data: &[u8]);
    /// Sends a message that is delivered, once and in order; at most [`MAX_RELIABLE_BYTES`].
    fn send_reliable(&mut self, peer: PeerId, data: &[u8]);
    /// The next event, or `None` if there is nothing to report.
    fn poll(&mut self) -> Option<TransportEvent>;
    /// Counters for the link to `peer` (all zero for an unknown peer).
    fn stats(&self, peer: PeerId) -> LinkStats;
}

// ---------------------------------------------------------------------------------------------
// Loopback.

#[derive(Default)]
struct Shared {
    /// Events waiting for endpoint 0 and endpoint 1.
    queues: [VecDeque<TransportEvent>; 2],
    connected: bool,
}

/// One end of an in-process link made by [`LoopbackTransport::pair`].
pub struct LoopbackTransport {
    shared: Arc<Mutex<Shared>>,
    /// 0 or 1: which queue this end reads.
    side: usize,
    remote: PeerId,
    stats: LinkStats,
    undeliverable: u64,
}

impl LoopbackTransport {
    /// Two connected endpoints. The first addresses the second as `PeerId(1)` and the second
    /// addresses the first as `PeerId(0)`; each starts with a `Connected` event queued.
    #[must_use]
    pub fn pair() -> (Self, Self) {
        let mut shared = Shared {
            connected: true,
            ..Shared::default()
        };
        shared.queues[0].push_back(TransportEvent::Connected(PeerId(1)));
        shared.queues[1].push_back(TransportEvent::Connected(PeerId(0)));
        let shared = Arc::new(Mutex::new(shared));
        let end = |side: usize| Self {
            shared: Arc::clone(&shared),
            side,
            remote: PeerId(1 - side as u32),
            stats: LinkStats::default(),
            undeliverable: 0,
        };
        (end(0), end(1))
    }

    /// The id by which this endpoint addresses the other.
    #[must_use]
    pub fn peer(&self) -> PeerId {
        self.remote
    }

    /// Cuts the link. Both ends see `Disconnected` after whatever was already in flight, and
    /// later sends are dropped.
    pub fn disconnect(&mut self) {
        let mut s = self.lock();
        if !s.connected {
            return;
        }
        s.connected = false;
        s.queues[0].push_back(TransportEvent::Disconnected(PeerId(1)));
        s.queues[1].push_back(TransportEvent::Disconnected(PeerId(0)));
    }

    /// Messages that could not be delivered because they were addressed to nobody or the link
    /// was cut.
    #[must_use]
    pub fn undeliverable(&self) -> u64 {
        self.undeliverable
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn send(&mut self, peer: PeerId, channel: Channel, data: &[u8]) {
        let limit = match channel {
            Channel::Unreliable => MAX_UNRELIABLE_BYTES,
            Channel::Reliable => MAX_RELIABLE_BYTES,
        };
        if data.len() > limit {
            self.stats.oversize_drops += 1;
            return;
        }
        let mut s = self.lock();
        if peer != self.remote || !s.connected {
            drop(s);
            self.undeliverable += 1;
            return;
        }
        // The sender is the peer the receiver knows it by.
        let from = PeerId(self.side as u32);
        s.queues[1 - self.side].push_back(TransportEvent::Message {
            peer: from,
            channel,
            bytes: data.to_vec(),
        });
        drop(s);
        self.stats.messages_sent += 1;
        self.stats.bytes_sent += data.len() as u64;
    }
}

impl Transport for LoopbackTransport {
    fn send_unreliable(&mut self, peer: PeerId, data: &[u8]) {
        self.send(peer, Channel::Unreliable, data);
    }

    fn send_reliable(&mut self, peer: PeerId, data: &[u8]) {
        self.send(peer, Channel::Reliable, data);
    }

    fn poll(&mut self) -> Option<TransportEvent> {
        let event = self.lock().queues[self.side].pop_front()?;
        if let TransportEvent::Message { bytes, .. } = &event {
            self.stats.messages_received += 1;
            self.stats.bytes_received += bytes.len() as u64;
        }
        Some(event)
    }

    fn stats(&self, peer: PeerId) -> LinkStats {
        if peer == self.remote {
            self.stats
        } else {
            LinkStats::default()
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Contract.

/// Behaviour every `Transport` must have (`AC-61.5`, `AC-66.2`), checked on two freshly made,
/// connected endpoints. `a_to_b` is how `a` addresses `b`; `b_to_a` the reverse.
///
/// It checks what a lossy real link may still promise: reliable messages arrive complete, once
/// and in order; unreliable ones, if they arrive, are intact and not invented; the payload
/// limits hold; counters add up; bad addresses and empty queues are harmless.
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(a: &mut dyn Transport, b: &mut dyn Transport, a_to_b: PeerId, b_to_a: PeerId) {
    assert_eq!(
        a.poll(),
        Some(TransportEvent::Connected(a_to_b)),
        "a fresh endpoint reports its peer as connected first"
    );
    assert_eq!(b.poll(), Some(TransportEvent::Connected(b_to_a)));
    assert_eq!(a.poll(), None, "nothing else is pending");

    // Reliable: sizes from empty to the limit, in order, byte-exact, in both directions.
    let sizes = [0, 1, 2, 63, 1_199, 1_200, 1_201, 4_096, MAX_RELIABLE_BYTES];
    let payload = |i: usize, len: usize| -> Vec<u8> {
        (0..len).map(|k| (k.wrapping_mul(31) ^ i) as u8).collect()
    };
    for (i, &len) in sizes.iter().enumerate() {
        a.send_reliable(a_to_b, &payload(i, len));
        b.send_reliable(b_to_a, &payload(i + 100, len));
    }
    for (i, &len) in sizes.iter().enumerate() {
        assert_eq!(
            b.poll(),
            Some(TransportEvent::Message {
                peer: b_to_a,
                channel: Channel::Reliable,
                bytes: payload(i, len),
            }),
            "reliable message {i} ({len} bytes) arrives complete and in order"
        );
        assert_eq!(
            a.poll(),
            Some(TransportEvent::Message {
                peer: a_to_b,
                channel: Channel::Reliable,
                bytes: payload(i + 100, len),
            }),
            "and in the other direction"
        );
    }

    // Unreliable: whatever arrives was sent, intact, and no more arrives than was sent.
    let sent: Vec<Vec<u8>> = (0..50).map(|i| payload(i, 1 + i * 23)).collect();
    for m in &sent {
        a.send_unreliable(a_to_b, m);
    }
    let mut remaining = sent.clone();
    let mut got = 0;
    while let Some(event) = b.poll() {
        let TransportEvent::Message { channel, bytes, .. } = event else {
            panic!("only messages are expected here, got {event:?}");
        };
        assert_eq!(channel, Channel::Unreliable);
        let at = remaining
            .iter()
            .position(|m| *m == bytes)
            .expect("an unreliable message that arrives was sent, and arrives at most once");
        remaining.swap_remove(at);
        got += 1;
    }
    assert!(got <= sent.len());

    // Limits: a too-large message is refused, not truncated and not delivered; the edge passes.
    let before = a.stats(a_to_b);
    a.send_unreliable(a_to_b, &vec![7; MAX_UNRELIABLE_BYTES + 1]);
    a.send_reliable(a_to_b, &vec![7; MAX_RELIABLE_BYTES + 1]);
    assert_eq!(b.poll(), None, "oversize messages are not delivered");
    let after = a.stats(a_to_b);
    assert_eq!(after.oversize_drops, before.oversize_drops + 2);
    assert_eq!(
        after.bytes_sent, before.bytes_sent,
        "refused bytes are not counted as sent"
    );
    a.send_unreliable(a_to_b, &vec![7; MAX_UNRELIABLE_BYTES]);
    assert!(
        matches!(b.poll(), None | Some(TransportEvent::Message { .. })),
        "a message at the limit is acceptable"
    );

    // Counters add up.
    let sa = a.stats(a_to_b);
    let sb = b.stats(b_to_a);
    assert!(
        sa.messages_sent >= sizes.len() as u64,
        "reliable sends are counted"
    );
    assert!(sa.bytes_sent > 0 && sb.bytes_received > 0);
    assert!(
        sb.messages_received <= sa.messages_sent,
        "nothing is received that was not sent"
    );
    assert_eq!(
        a.stats(PeerId(9_999)),
        LinkStats::default(),
        "an unknown peer has no stats"
    );

    // Bad addresses and empty queues are harmless.
    a.send_reliable(PeerId(9_999), b"nobody");
    a.send_unreliable(PeerId(9_999), b"nobody");
    assert_eq!(b.poll(), None);
    assert_eq!(a.poll(), None);
    assert_eq!(a.poll(), None, "polling an empty queue stays empty");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_passes_the_contract() {
        let (mut a, mut b) = LoopbackTransport::pair();
        let (to_b, to_a) = (a.peer(), b.peer());
        contract(&mut a, &mut b, to_b, to_a);
    }

    #[test]
    fn the_two_ends_know_each_other_by_different_ids() {
        let (a, b) = LoopbackTransport::pair();
        assert_eq!((a.peer(), b.peer()), (PeerId(1), PeerId(0)));
    }

    #[test]
    fn messages_are_copied_and_attributed_to_the_sender() {
        let (mut a, mut b) = LoopbackTransport::pair();
        let mut buf = *b"first";
        a.send_reliable(PeerId(1), &buf);
        buf.copy_from_slice(b"XXXXX");
        assert_eq!(b.poll(), Some(TransportEvent::Connected(PeerId(0))));
        assert_eq!(
            b.poll(),
            Some(TransportEvent::Message {
                peer: PeerId(0),
                channel: Channel::Reliable,
                bytes: b"first".to_vec()
            })
        );
    }

    #[test]
    fn limits_are_the_ones_in_the_gdd() {
        assert_eq!(MAX_UNRELIABLE_BYTES, 1_200);
        assert_eq!(MAX_RELIABLE_BYTES, 16_384);
        let (mut a, mut b) = LoopbackTransport::pair();
        a.send_unreliable(PeerId(1), &[0; 1_200]);
        a.send_unreliable(PeerId(1), &[0; 1_201]);
        a.send_reliable(PeerId(1), &vec![0; 16_384]);
        a.send_reliable(PeerId(1), &vec![0; 16_385]);
        let s = a.stats(PeerId(1));
        assert_eq!((s.messages_sent, s.oversize_drops), (2, 2));
        assert_eq!(s.bytes_sent, 1_200 + 16_384);
        let mut delivered = 0;
        while let Some(e) = b.poll() {
            delivered += usize::from(matches!(e, TransportEvent::Message { .. }));
        }
        assert_eq!(delivered, 2);
        let r = b.stats(PeerId(0));
        assert_eq!((r.messages_received, r.bytes_received), (2, 1_200 + 16_384));
    }

    #[test]
    fn disconnect_delivers_what_was_in_flight_then_reports_it() {
        let (mut a, mut b) = LoopbackTransport::pair();
        assert!(matches!(b.poll(), Some(TransportEvent::Connected(_))));
        a.send_reliable(PeerId(1), b"last words");
        a.disconnect();
        a.send_reliable(PeerId(1), b"too late");
        assert_eq!(a.undeliverable(), 1);
        assert!(matches!(b.poll(), Some(TransportEvent::Message { .. })));
        assert_eq!(b.poll(), Some(TransportEvent::Disconnected(PeerId(0))));
        assert_eq!(b.poll(), None);
        // Both ends see it, and cutting twice reports once.
        b.disconnect();
        assert!(matches!(a.poll(), Some(TransportEvent::Connected(_))));
        assert_eq!(a.poll(), Some(TransportEvent::Disconnected(PeerId(1))));
        assert_eq!(a.poll(), None);
    }

    #[test]
    fn a_misaddressed_message_goes_nowhere_and_is_counted() {
        let (mut a, mut b) = LoopbackTransport::pair();
        let _ = (a.poll(), b.poll());
        a.send_reliable(PeerId(0), b"to myself");
        a.send_unreliable(PeerId(2), b"to a stranger");
        assert_eq!((a.poll(), b.poll()), (None, None));
        assert_eq!(a.undeliverable(), 2);
        assert_eq!(a.stats(PeerId(1)).messages_sent, 0);
    }

    #[test]
    fn endpoints_can_move_to_another_thread() {
        fn assert_send<T: Send>() {}
        assert_send::<LoopbackTransport>();
        let (mut a, mut b) = LoopbackTransport::pair();
        let t = std::thread::spawn(move || {
            a.send_reliable(PeerId(1), b"across threads");
        });
        t.join().unwrap();
        let _ = b.poll();
        assert!(matches!(b.poll(), Some(TransportEvent::Message { .. })));
    }
}
