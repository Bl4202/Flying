/**
 * The snapshot channel: Sim Worker -> Render Worker, one way, latest wins.
 *
 * Two implementations behind one interface, chosen by whether the page is cross-origin isolated:
 *
 * * `shared`: a three-slot seqlock in a `SharedArrayBuffer` (platform-web `ipc/triple-buffer`).
 *   The writer never waits and the reader never blocks it.
 * * `message`: a pool of four transferable `ArrayBuffer`s over a `MessagePort`. The sink sends a
 *   buffer, the source keeps only the newest and sends older ones back to be reused. If all four
 *   are in flight (the reader is stalled) the sink holds the newest snapshot back, replacing it
 *   with each later one, and sends it the moment a buffer returns: no allocation, and the reader
 *   still ends up on the newest.
 *
 * Both give the reader the same thing: the newest snapshot, a publication number that only goes
 * up, and the same number again if nothing new was published.
 */
import {
  CONTENDED,
  initSeqlock,
  SeqlockReader,
  SeqlockWriter,
  seqlockBytes,
} from '@flying/platform-web/ipc/triple-buffer';
import { SNAPSHOT } from './layout.gen.ts';
import { SNAPSHOT_SAB_LAYOUT } from './layouts.ts';

export type SnapshotEndpoint =
  { kind: 'shared'; sab: SharedArrayBuffer } | { kind: 'message'; port: MessagePort };

export interface SnapshotSink {
  /**
   * Publishes `src` (exactly one snapshot). `prepare` runs on the published copy before anyone
   * can see it, for the host to stamp its own fields.
   */
  publish(src: Uint8Array, prepare?: (copy: Uint8Array) => void): void;
  /**
   * Publications that were replaced by a newer one before they could be sent (message channel
   * only, when the reader stalls; always 0 for shared).
   */
  dropped(): number;
  close(): void;
}

export interface SnapshotSource {
  /**
   * Copies the newest snapshot into `out`. Returns its publication number (more than 0), or 0 if
   * nothing has been published, or `CONTENDED` (shared channel under a hammering writer) if the
   * read gave up; in the last two cases `out` is untouched.
   */
  read(out: Uint8Array): number;
  close(): void;
}

export { CONTENDED };

/** Buffers circulating on the message channel. */
export const MESSAGE_POOL = 4;

/** Both ends of a new channel. Hand `sink` to the producer's thread and `source` to the consumer's. */
export function makeSnapshotChannel(kind: 'shared' | 'message'): {
  sink: SnapshotEndpoint;
  source: SnapshotEndpoint;
} {
  if (kind === 'shared') {
    const sab = new SharedArrayBuffer(seqlockBytes(SNAPSHOT_SAB_LAYOUT));
    initSeqlock(sab, SNAPSHOT_SAB_LAYOUT);
    return { sink: { kind: 'shared', sab }, source: { kind: 'shared', sab } };
  }
  const { port1, port2 } = new MessageChannel();
  return { sink: { kind: 'message', port: port1 }, source: { kind: 'message', port: port2 } };
}

/** What to list in `postMessage`'s transfer argument when sending an endpoint to a worker. */
export function snapshotTransferables(endpoint: SnapshotEndpoint): Transferable[] {
  return endpoint.kind === 'message' ? [endpoint.port] : [];
}

export function openSnapshotSink(endpoint: SnapshotEndpoint): SnapshotSink {
  if (endpoint.kind === 'shared') {
    const writer = new SeqlockWriter(endpoint.sab, SNAPSHOT_SAB_LAYOUT);
    return {
      publish: (src, prepare) => {
        writer.publish(src, prepare);
      },
      dropped: () => 0,
      close: () => undefined,
    };
  }
  return new MessageSink(endpoint.port);
}

export function openSnapshotSource(endpoint: SnapshotEndpoint): SnapshotSource {
  if (endpoint.kind === 'shared') {
    const reader = new SeqlockReader(endpoint.sab, SNAPSHOT_SAB_LAYOUT);
    return { read: (out) => reader.read(out), close: () => undefined };
  }
  return new MessageSource(endpoint.port);
}

interface Published {
  n: number;
  buf: ArrayBuffer;
}
interface Recycled {
  recycle: ArrayBuffer;
}

class MessageSink implements SnapshotSink {
  readonly #port: MessagePort;
  #free: ArrayBuffer[] = [];
  #n = 0;
  #dropped = 0;
  /** The newest snapshot that found no free buffer, waiting for one. */
  readonly #held = new Uint8Array(SNAPSHOT.SIZE);
  #hasHeld = false;
  #heldPrepare: ((copy: Uint8Array) => void) | undefined;

  constructor(port: MessagePort) {
    this.#port = port;
    for (let i = 0; i < MESSAGE_POOL; i++) {
      this.#free.push(new ArrayBuffer(SNAPSHOT.SIZE));
    }
    port.onmessage = (event: MessageEvent<Recycled>) => {
      const buf = event.data.recycle;
      if (buf.byteLength !== SNAPSHOT.SIZE) {
        return;
      }
      this.#free.push(buf);
      if (this.#hasHeld) {
        this.#hasHeld = false;
        this.#send(this.#held, this.#heldPrepare);
        this.#heldPrepare = undefined;
      }
    };
  }

  publish(src: Uint8Array, prepare?: (copy: Uint8Array) => void): void {
    if (src.byteLength !== SNAPSHOT.SIZE) {
      throw new RangeError(
        `snapshot is ${String(src.byteLength)} bytes, expected ${String(SNAPSHOT.SIZE)}`,
      );
    }
    if (this.#free.length === 0) {
      if (this.#hasHeld) {
        this.#dropped += 1; // the one being replaced was never sent
      }
      this.#held.set(src);
      this.#hasHeld = true;
      this.#heldPrepare = prepare;
      return;
    }
    this.#send(src, prepare);
  }

  #send(src: Uint8Array, prepare: ((copy: Uint8Array) => void) | undefined): void {
    const buf = this.#free.pop();
    if (buf === undefined) {
      return;
    }
    const view = new Uint8Array(buf);
    view.set(src);
    prepare?.(view);
    this.#n += 1;
    this.#port.postMessage({ n: this.#n, buf } satisfies Published, [buf]);
  }

  dropped(): number {
    return this.#dropped;
  }

  close(): void {
    this.#port.close();
  }
}

class MessageSource implements SnapshotSource {
  readonly #port: MessagePort;
  #latest: Published | undefined;

  constructor(port: MessagePort) {
    this.#port = port;
    port.onmessage = (event: MessageEvent<Published>) => {
      const previous = this.#latest;
      this.#latest = event.data;
      if (previous !== undefined) {
        // Latest wins: the older buffer goes back to the sink for reuse.
        port.postMessage({ recycle: previous.buf } satisfies Recycled, [previous.buf]);
      }
    };
  }

  read(out: Uint8Array): number {
    if (out.byteLength < SNAPSHOT.SIZE) {
      throw new RangeError('output buffer is smaller than a snapshot');
    }
    const latest = this.#latest;
    if (latest === undefined) {
      return 0;
    }
    out.set(new Uint8Array(latest.buf));
    return latest.n;
  }

  close(): void {
    this.#port.close();
  }
}
