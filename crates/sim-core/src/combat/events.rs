//! The combat event ring (ADR-48): the last [`EVENT_CAPACITY`] events, each with a sequence
//! number that starts at 1 and never repeats in a session, so the host can read every event once
//! from the snapshot even if it skips frames (up to the ring's length). An observation: never
//! hashed.

use super::EVENT_CAPACITY;

/// One event (layout `snapshot::EV_*`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CombatEvent {
    /// Sequence number (0: the entry is empty).
    pub seq: u32,
    /// The tick it happened, mod 2³².
    pub tick: u32,
    /// `combat_event`.
    pub kind: u8,
    /// Who did it (`combat_actor`).
    pub actor: u8,
    /// To whom (`combat_actor`).
    pub target: u8,
    /// The kind's detail (a `kill_weapon`, `crash_cause`, `miss_reason` or `launch_inhibit`).
    pub arg: u8,
    /// `MISSILE_HIT`: the miss distance, m; KILL: the range, m.
    pub value: f32,
}

/// The ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventRing {
    /// The entries; the newest is at `(seq − 1) % EVENT_CAPACITY`.
    pub entries: [CombatEvent; EVENT_CAPACITY],
    /// The newest event's sequence number (0: none yet).
    pub seq: u32,
}

impl Default for EventRing {
    fn default() -> Self {
        Self::new()
    }
}

impl EventRing {
    /// No events.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [CombatEvent {
                seq: 0,
                tick: 0,
                kind: 0,
                actor: 0,
                target: 0,
                arg: 0,
                value: 0.0,
            }; EVENT_CAPACITY],
            seq: 0,
        }
    }

    /// Adds an event, overwriting the oldest once the ring is full.
    pub fn push(&mut self, kind: u8, actor: u8, target: u8, arg: u8, value: f64, tick: u64) {
        self.seq = self.seq.wrapping_add(1).max(1);
        let at = (self.seq as usize - 1) % EVENT_CAPACITY;
        self.entries[at] = CombatEvent {
            seq: self.seq,
            tick: tick as u32,
            kind,
            actor,
            target,
            arg,
            value: value as f32,
        };
    }

    /// The events with sequence numbers above `after`, oldest first (at most the ring's length).
    pub fn since(&self, after: u32) -> impl Iterator<Item = &CombatEvent> {
        let mut out: [Option<&CombatEvent>; EVENT_CAPACITY] = [None; EVENT_CAPACITY];
        let mut n = 0;
        for e in &self.entries {
            if e.seq > after {
                out[n] = Some(e);
                n += 1;
            }
        }
        out[..n].sort_by_key(|e| e.map_or(0, |e| e.seq));
        out.into_iter().take(n).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_numbers_events_from_one_and_overwrites_the_oldest() {
        let mut r = EventRing::new();
        assert_eq!(r.seq, 0);
        assert!(r.entries.iter().all(|e| e.seq == 0));
        r.push(4, 0, 1, 1, 812.5, 70);
        assert_eq!(r.seq, 1);
        assert_eq!(
            r.entries[0],
            CombatEvent {
                seq: 1,
                tick: 70,
                kind: 4,
                actor: 0,
                target: 1,
                arg: 1,
                value: 812.5,
            }
        );
        for k in 0..40_u32 {
            r.push(1, 0, 1, 0, f64::from(k), 100 + u64::from(k));
        }
        assert_eq!(r.seq, 41);
        // The newest is at (41 − 1) % 32 = 8; the oldest kept is 41 − 31 = 10.
        assert_eq!(r.entries[8].seq, 41);
        let seqs: Vec<u32> = r.since(0).map(|e| e.seq).collect();
        assert_eq!(seqs, (10..=41).collect::<Vec<_>>());
        let late: Vec<u32> = r.since(39).map(|e| e.seq).collect();
        assert_eq!(late, [40, 41]);
        // The tick wraps at 2³².
        r.push(2, 1, 0, 0, 0.0, (1 << 32) + 5);
        assert_eq!(r.entries[9].tick, 5);
    }
}
