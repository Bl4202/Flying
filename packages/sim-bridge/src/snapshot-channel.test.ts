import { describe, expect, it } from 'vitest';
import { waitFor } from './channel-test-support.ts';
import { SNAPSHOT } from './layout.gen.ts';
import {
  MESSAGE_POOL,
  makeSnapshotChannel,
  openSnapshotSink,
  openSnapshotSource,
} from './snapshot-channel.ts';
import { snapshotChannelContract } from './snapshot-channel.contract.ts';

snapshotChannelContract('shared');
snapshotChannelContract('message');

describe('snapshot channel specifics', () => {
  it('the message channel holds back only the newest when every buffer is in flight, and sends it when one returns', async () => {
    const { sink, source } = makeSnapshotChannel('message');
    const s = openSnapshotSink(sink);
    const r = openSnapshotSource(source);
    try {
      const snap = (n: number): Uint8Array => {
        const b = new Uint8Array(SNAPSHOT.SIZE);
        new DataView(b.buffer).setUint32(0, n, true);
        return b;
      };
      // No yielding: the source cannot receive or recycle anything while we hold the thread.
      for (let i = 1; i <= MESSAGE_POOL + 3; i++) s.publish(snap(i));
      // Four were sent; the 5th, 6th and 7th competed for the held slot, so two were replaced.
      expect(s.dropped()).toBe(2);
      const out = new Uint8Array(SNAPSHOT.SIZE);
      const content = (): number => new DataView(out.buffer).getUint32(0, true);
      await waitFor(() => r.read(out) === MESSAGE_POOL + 1, 'the held snapshot to be sent');
      expect(content()).toBe(MESSAGE_POOL + 3);
      // Nothing is left held: the next publication goes straight out.
      s.publish(snap(100));
      await waitFor(() => r.read(out) === MESSAGE_POOL + 2, 'the next snapshot');
      expect(content()).toBe(100);
      expect(s.dropped()).toBe(2);
    } finally {
      s.close();
      r.close();
    }
  });

  it('the shared channel never drops', () => {
    const { sink } = makeSnapshotChannel('shared');
    const s = openSnapshotSink(sink);
    for (let i = 0; i < 1000; i++) s.publish(new Uint8Array(SNAPSHOT.SIZE));
    expect(s.dropped()).toBe(0);
  });

  it('endpoints say what to transfer', async () => {
    const { snapshotTransferables } = await import('./snapshot-channel.ts');
    const shared = makeSnapshotChannel('shared');
    const message = makeSnapshotChannel('message');
    expect(snapshotTransferables(shared.sink)).toEqual([]);
    expect(snapshotTransferables(message.sink)).toHaveLength(1);
    for (const end of [message.sink, message.source]) {
      if (end.kind === 'message') {
        end.port.close();
      }
    }
  });
});
