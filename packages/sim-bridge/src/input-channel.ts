/**
 * The input channel: main thread -> Sim Worker, one way, in order.
 *
 * * `shared`: the SPSC ring in a `SharedArrayBuffer` (platform-web `ipc/spsc-ring`).
 * * `message`: one `postMessage` per frame (the pump sends at about 30 Hz in this mode), queued on
 *   the Sim Worker's side in a ring of the same capacity.
 *
 * Both drop the *newest* frame when 64 are already waiting and count it, so a stalled sim shows up
 * as a number rather than as unbounded memory.
 */
import {
  initRing,
  ringBytes,
  SpscRingConsumer,
  SpscRingProducer,
} from '@flying/platform-web/ipc/spsc-ring';
import { INPUT_FRAME, INPUT_RING } from './layout.gen.ts';
import { INPUT_RING_LAYOUT } from './layouts.ts';

export type InputEndpoint =
  { kind: 'shared'; sab: SharedArrayBuffer } | { kind: 'message'; port: MessagePort };

export interface InputSink {
  /** Sends one 96-byte frame. False if it was refused (shared ring full). */
  send(frame: Uint8Array): boolean;
  close(): void;
}

export interface InputSource {
  /** Copies up to `maxFrames` waiting frames into `out`, oldest first. Returns how many. */
  drain(out: Uint8Array, maxFrames: number): number;
  /** Frames waiting. */
  pending(): number;
  /** Frames dropped because the queue was full. */
  dropped(): number;
  close(): void;
}

export function makeInputChannel(kind: 'shared' | 'message'): {
  sink: InputEndpoint;
  source: InputEndpoint;
} {
  if (kind === 'shared') {
    const sab = new SharedArrayBuffer(ringBytes(INPUT_RING_LAYOUT));
    initRing(sab, INPUT_RING_LAYOUT);
    return { sink: { kind: 'shared', sab }, source: { kind: 'shared', sab } };
  }
  const { port1, port2 } = new MessageChannel();
  return { sink: { kind: 'message', port: port1 }, source: { kind: 'message', port: port2 } };
}

export function inputTransferables(endpoint: InputEndpoint): Transferable[] {
  return endpoint.kind === 'message' ? [endpoint.port] : [];
}

export function openInputSink(endpoint: InputEndpoint): InputSink {
  if (endpoint.kind === 'shared') {
    const producer = new SpscRingProducer(endpoint.sab, INPUT_RING_LAYOUT);
    return { send: (frame) => producer.push(frame), close: () => undefined };
  }
  const { port } = endpoint;
  return {
    send(frame) {
      if (frame.byteLength !== INPUT_FRAME.SIZE) {
        throw new RangeError(
          `a frame is ${String(INPUT_FRAME.SIZE)} bytes, got ${String(frame.byteLength)}`,
        );
      }
      const copy = frame.slice().buffer;
      port.postMessage(copy, [copy]);
      return true;
    },
    close: () => {
      port.close();
    },
  };
}

export function openInputSource(endpoint: InputEndpoint): InputSource {
  if (endpoint.kind === 'shared') {
    const consumer = new SpscRingConsumer(endpoint.sab, INPUT_RING_LAYOUT);
    return {
      drain: (out, maxFrames) => consumer.drain(out, maxFrames),
      pending: () => consumer.pending(),
      dropped: () => consumer.dropped(),
      close: () => undefined,
    };
  }
  return new MessageInputSource(endpoint.port);
}

class MessageInputSource implements InputSource {
  readonly #port: MessagePort;
  readonly #queue = new Uint8Array(INPUT_RING.CAPACITY * INPUT_FRAME.SIZE);
  #head = 0;
  #count = 0;
  #dropped = 0;

  constructor(port: MessagePort) {
    this.#port = port;
    port.onmessage = (event: MessageEvent<ArrayBuffer>) => {
      const frame = event.data;
      if (frame.byteLength !== INPUT_FRAME.SIZE) {
        return;
      }
      if (this.#count >= INPUT_RING.CAPACITY) {
        this.#dropped += 1;
        return;
      }
      const slot = (this.#head + this.#count) % INPUT_RING.CAPACITY;
      this.#queue.set(new Uint8Array(frame), slot * INPUT_FRAME.SIZE);
      this.#count += 1;
    };
  }

  drain(out: Uint8Array, maxFrames: number): number {
    if (out.byteLength < maxFrames * INPUT_FRAME.SIZE) {
      throw new RangeError('output buffer is smaller than maxFrames frames');
    }
    const n = Math.min(this.#count, maxFrames);
    for (let i = 0; i < n; i++) {
      const at = ((this.#head + i) % INPUT_RING.CAPACITY) * INPUT_FRAME.SIZE;
      out.set(this.#queue.subarray(at, at + INPUT_FRAME.SIZE), i * INPUT_FRAME.SIZE);
    }
    this.#head = (this.#head + n) % INPUT_RING.CAPACITY;
    this.#count -= n;
    return n;
  }

  pending(): number {
    return this.#count;
  }

  dropped(): number {
    return this.#dropped;
  }

  close(): void {
    this.#port.close();
  }
}
