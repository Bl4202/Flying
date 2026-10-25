import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { PumpObservation } from '@flying/input';
import {
  type AircraftStatus,
  CONTENDED,
  newAircraftStatus,
  SNAPSHOT,
  type SnapshotSource,
} from '@flying/sim-bridge';
import { StatusFeed } from './status-feed.ts';

const vector = JSON.parse(
  readFileSync(new URL('../../../tests/vectors/snapshot_v3.json', import.meta.url), 'utf8'),
) as { hex: string; decoded: { ownship: Record<string, number> } };
const SNAPSHOT_BYTES = Uint8Array.from(vector.hex.match(/../g) ?? [], (h) =>
  Number.parseInt(h, 16),
);

/** A source that hands out `bytes` under a publication number the test sets. */
function fakeSource(bytes: Uint8Array = SNAPSHOT_BYTES) {
  const state = { publication: 0, reads: 0 };
  const source: SnapshotSource = {
    read: (out) => {
      state.reads += 1;
      if (state.publication > 0 && state.publication !== CONTENDED) out.set(bytes);
      return state.publication;
    },
    close: () => undefined,
  };
  return { state, source };
}

function rig(source: SnapshotSource | undefined, reported?: () => AircraftStatus | undefined) {
  const seen: { status: AircraftStatus; observation: PumpObservation }[] = [];
  const frames: (() => void)[] = [];
  const feed = new StatusFeed({
    source,
    reported: reported ?? (() => undefined),
    requestFrame: (cb) => frames.push(cb),
    cancelFrame: () => undefined,
    onStatus: (status, observation) => {
      seen.push({ status, observation });
    },
  });
  return { feed, seen, frames };
}

describe('StatusFeed (shared channel)', () => {
  it('has no status, and tells no one, before the first snapshot', () => {
    const { source } = fakeSource();
    const { feed, seen } = rig(source);
    expect(feed.poll()).toBe(false);
    expect(feed.status).toBeUndefined();
    expect(seen).toEqual([]);
  });

  it('reads the aircraft from the newest snapshot and hands the pump what it needs', () => {
    const { source, state } = fakeSource();
    const { feed, seen } = rig(source);
    state.publication = 1;
    expect(feed.poll()).toBe(true);
    const o = vector.decoded.ownship;
    expect(feed.status?.spawnSeq).toBe(o['spawn_seq']);
    expect(feed.status?.assist).toBe(o['assist']);
    const last = seen.at(-1);
    expect(last?.observation.spawnSeq).toBe(feed.status?.spawnSeq);
    expect(last?.observation.throttle).toBe(feed.status?.throttle);
    expect(last?.observation.assist).toBe(feed.status?.assist);
    expect(last?.observation.noseAzRad).toBe(feed.status?.noseAz);
    expect(last?.observation.noseElRad).toBe(feed.status?.noseEl);
  });

  it('reuses one status and one observation, however often it polls', () => {
    const { source, state } = fakeSource();
    const { feed, seen } = rig(source);
    state.publication = 1;
    feed.poll();
    state.publication = 2;
    feed.poll();
    expect(seen).toHaveLength(2);
    expect(seen[1]?.status).toBe(seen[0]?.status);
    expect(seen[1]?.observation).toBe(seen[0]?.observation);
  });

  it('does not read the status again for a publication it has seen', () => {
    const bytes = SNAPSHOT_BYTES.slice();
    const { source, state } = fakeSource(bytes);
    const { feed } = rig(source);
    state.publication = 5;
    feed.poll();
    // The bytes change under the same publication number: the number says nothing is new.
    new DataView(bytes.buffer).setFloat32(SNAPSHOT.O_NZ, 7.5, true);
    feed.poll();
    expect(feed.status?.nz).not.toBe(7.5);
    state.publication = 6;
    feed.poll();
    expect(feed.status?.nz).toBe(7.5);
  });

  it('keeps the last status when the read is contended or the snapshot is not valid', () => {
    const { source, state } = fakeSource();
    const { feed } = rig(source);
    state.publication = 1;
    feed.poll();
    const first = feed.status?.spawnSeq;
    state.publication = CONTENDED;
    expect(feed.poll()).toBe(true);
    expect(feed.status?.spawnSeq).toBe(first);

    const bad = fakeSource(new Uint8Array(SNAPSHOT.SIZE));
    bad.state.publication = 1;
    const empty = rig(bad.source);
    expect(empty.feed.poll()).toBe(false);
    expect(empty.feed.status).toBeUndefined();
  });

  it('polls once per animation frame between start and stop', () => {
    const { source, state } = fakeSource();
    const { feed, seen, frames } = rig(source);
    state.publication = 1;
    feed.start();
    feed.start();
    expect(frames).toHaveLength(1);
    frames.shift()?.();
    frames.shift()?.();
    expect(seen).toHaveLength(2);
    feed.stop();
  });
});

describe('StatusFeed (message channel)', () => {
  it("passes on the status the sim reported, which is the renderer's channel to read", () => {
    const reported = newAircraftStatus();
    reported.spawnSeq = 7;
    reported.assist = 3;
    reported.throttle = 0.4;
    const latest: { status?: AircraftStatus } = {};
    const { feed, seen } = rig(undefined, () => latest.status);
    expect(feed.poll()).toBe(false);
    expect(feed.status).toBeUndefined();
    latest.status = reported;
    expect(feed.poll()).toBe(true);
    expect(feed.status).toBe(reported);
    expect(seen[0]?.observation).toMatchObject({ spawnSeq: 7, assist: 3, throttle: 0.4 });
  });
});
