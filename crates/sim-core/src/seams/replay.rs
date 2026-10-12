//! `ReplaySink`: where each tick's published snapshot goes for later playback (GDD 61.5). The
//! first implementations drop it or count it; the keyframe-and-delta recorder replaces them in
//! M2.

/// Receives every published snapshot, in tick order. Called inside the tick: it must not block
/// and should not allocate.
pub trait ReplaySink: Send {
    /// Records the snapshot published at the end of tick `tick`.
    fn record(&mut self, tick: u64, snapshot: &[u8]);
}

/// Drops everything.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullReplaySink;

impl ReplaySink for NullReplaySink {
    fn record(&mut self, _tick: u64, _snapshot: &[u8]) {}
}

/// Counts what it is given and keeps nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CountingReplaySink {
    /// Snapshots recorded.
    pub frames: u64,
    /// Total bytes offered.
    pub bytes: u64,
    /// Tick of the newest snapshot.
    pub last_tick: Option<u64>,
    /// Snapshots that arrived with a tick not greater than the one before.
    pub out_of_order: u64,
}

impl ReplaySink for CountingReplaySink {
    fn record(&mut self, tick: u64, snapshot: &[u8]) {
        if self.last_tick.is_some_and(|last| tick <= last) {
            self.out_of_order += 1;
        }
        self.frames += 1;
        self.bytes += snapshot.len() as u64;
        self.last_tick = Some(tick);
    }
}

/// Behaviour every `ReplaySink` must have (`AC-61.5`): it accepts any sequence without
/// panicking, including empty, large and repeated records.
///
/// # Panics
/// Panics if the implementation does.
pub fn contract(s: &mut dyn ReplaySink) {
    let big = vec![0xA5_u8; crate::layout::snapshot::SIZE];
    s.record(0, &[]);
    for tick in 1..=600 {
        s.record(tick, &big);
    }
    s.record(600, &big);
    s.record(5, &big[..7]);
    s.record(u64::MAX, &big);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sinks_pass_the_contract() {
        contract(&mut NullReplaySink);
        contract(&mut CountingReplaySink::default());
    }

    #[test]
    fn the_counting_sink_counts() {
        let mut s = CountingReplaySink::default();
        s.record(1, &[0; 10]);
        s.record(2, &[0; 5]);
        s.record(2, &[]);
        assert_eq!(
            s,
            CountingReplaySink {
                frames: 3,
                bytes: 15,
                last_tick: Some(2),
                out_of_order: 1,
            }
        );
    }
}
