/**
 * `InputFrame`: one sample of the player's controls, 96 bytes, little-endian (GDD 58.2).
 * Mirrors `crates/sim-core/src/input.rs`; tested against the vectors that file produces.
 */
import { AXIS, INPUT_FRAME } from './layout.gen.ts';

export interface InputFrame {
  /** Increases by one per frame the host produces; the core ignores stale ones. */
  seq: number;
  /** `INPUT_FLAG` bits. */
  flags: number;
  /**
   * When the source event happened (key press, pointer move, gamepad sample), milliseconds on the
   * shared clock. An observation: it never feeds simulation state (ADR-33).
   */
  tMs: number;
  /** When the frame was written, milliseconds on the shared clock. Observation only. */
  tSentMs: number;
  /** `AXIS`-indexed values: sticks and throttles in -1..1 or 0..1. */
  axes: Float32Array;
  /** One bit per button (`BUTTON`). */
  buttons: bigint;
  hats: [number, number];
  /** Requested assist level (`ASSIST_LEVEL`); 0 keeps the current one. */
  assistReq: number;
  /** The `spawn_seq` of the snapshot the host has synchronised its throttle lever to. */
  spawnAck: number;
  /** Spawn for the next reset (`SPAWN_REQUEST`); 0 uses the session's. */
  spawnReq: number;
}

export function neutralFrame(seq = 0): InputFrame {
  return {
    seq,
    flags: 0,
    tMs: 0,
    tSentMs: 0,
    axes: new Float32Array(INPUT_FRAME.AXIS_COUNT),
    buttons: 0n,
    hats: [0, 0],
    assistReq: 0,
    spawnAck: 0,
    spawnReq: 0,
  };
}

/** Writes `frame` into the first 96 bytes of `out`; unused bytes are zeroed. */
export function encodeInputFrame(frame: InputFrame, out: Uint8Array): void {
  if (out.byteLength < INPUT_FRAME.SIZE) {
    throw new RangeError(`need ${String(INPUT_FRAME.SIZE)} bytes, got ${String(out.byteLength)}`);
  }
  out.fill(0, 0, INPUT_FRAME.SIZE);
  const dv = new DataView(out.buffer, out.byteOffset, INPUT_FRAME.SIZE);
  dv.setUint32(INPUT_FRAME.SEQ, frame.seq >>> 0, true);
  dv.setUint32(INPUT_FRAME.FLAGS, frame.flags >>> 0, true);
  dv.setFloat64(INPUT_FRAME.T_MS, frame.tMs, true);
  for (let i = 0; i < INPUT_FRAME.AXIS_COUNT; i++) {
    dv.setFloat32(INPUT_FRAME.AXES + 4 * i, frame.axes[i] ?? 0, true);
  }
  dv.setBigUint64(INPUT_FRAME.BUTTONS, BigInt.asUintN(64, frame.buttons), true);
  dv.setUint8(INPUT_FRAME.HATS, frame.hats[0]);
  dv.setUint8(INPUT_FRAME.HATS + 1, frame.hats[1]);
  dv.setUint8(INPUT_FRAME.ASSIST_REQ, frame.assistReq);
  dv.setUint8(INPUT_FRAME.SPAWN_ACK, frame.spawnAck);
  dv.setUint8(INPUT_FRAME.SPAWN_REQ, frame.spawnReq);
  dv.setFloat64(INPUT_FRAME.T_SENT_MS, frame.tSentMs, true);
}

/**
 * Encodes frames into one fixed buffer without allocating. `encodeInputFrame` takes a `bigint`
 * button mask and builds a `DataView` per call, which at the pump's 250 Hz is garbage the page
 * never needs. Set the public fields, call `encode()`, hand `bytes` to the sink.
 *
 * Buttons are a plain number: the 32 low button bits. `BUTTON` uses 11 of them; a button 32 or
 * above would need the bigint path.
 */
export class InputFrameWriter {
  /** The encoded frame; rewritten by every `encode()`. */
  readonly bytes = new Uint8Array(INPUT_FRAME.SIZE);
  readonly axes = new Float32Array(INPUT_FRAME.AXIS_COUNT);
  /**
   * `[tMs, tSentMs]`. A Float64Array because V8 allocates for every store of a fractional number
   * into an object field, and a frame is written every few milliseconds.
   */
  readonly times = new Float64Array(2);
  seq = 0;
  flags = 0;
  /** The low 32 button bits. */
  buttons = 0;
  assistReq = 0;
  spawnAck = 0;
  spawnReq = 0;
  readonly #dv = new DataView(this.bytes.buffer);

  /** Writes the fields into `bytes` (unused bytes stay zero) and returns it. */
  encode(): Uint8Array {
    const dv = this.#dv;
    dv.setUint32(INPUT_FRAME.SEQ, this.seq >>> 0, true);
    dv.setUint32(INPUT_FRAME.FLAGS, this.flags >>> 0, true);
    dv.setFloat64(INPUT_FRAME.T_MS, this.times[0] ?? 0, true);
    for (let i = 0; i < INPUT_FRAME.AXIS_COUNT; i++) {
      dv.setFloat32(INPUT_FRAME.AXES + 4 * i, this.axes[i] ?? 0, true);
    }
    dv.setUint32(INPUT_FRAME.BUTTONS, this.buttons >>> 0, true);
    dv.setUint32(INPUT_FRAME.BUTTONS + 4, 0, true);
    dv.setUint8(INPUT_FRAME.ASSIST_REQ, this.assistReq);
    dv.setUint8(INPUT_FRAME.SPAWN_ACK, this.spawnAck);
    dv.setUint8(INPUT_FRAME.SPAWN_REQ, this.spawnReq);
    dv.setFloat64(INPUT_FRAME.T_SENT_MS, this.times[1] ?? 0, true);
    return this.bytes;
  }
}

export function decodeInputFrame(bytes: Uint8Array): InputFrame {
  if (bytes.byteLength < INPUT_FRAME.SIZE) {
    throw new RangeError(`need ${String(INPUT_FRAME.SIZE)} bytes, got ${String(bytes.byteLength)}`);
  }
  const dv = new DataView(bytes.buffer, bytes.byteOffset, INPUT_FRAME.SIZE);
  const axes = new Float32Array(INPUT_FRAME.AXIS_COUNT);
  for (let i = 0; i < axes.length; i++) {
    axes[i] = dv.getFloat32(INPUT_FRAME.AXES + 4 * i, true);
  }
  return {
    seq: dv.getUint32(INPUT_FRAME.SEQ, true),
    flags: dv.getUint32(INPUT_FRAME.FLAGS, true),
    tMs: dv.getFloat64(INPUT_FRAME.T_MS, true),
    tSentMs: dv.getFloat64(INPUT_FRAME.T_SENT_MS, true),
    axes,
    buttons: dv.getBigUint64(INPUT_FRAME.BUTTONS, true),
    hats: [dv.getUint8(INPUT_FRAME.HATS), dv.getUint8(INPUT_FRAME.HATS + 1)],
    assistReq: dv.getUint8(INPUT_FRAME.ASSIST_REQ),
    spawnAck: dv.getUint8(INPUT_FRAME.SPAWN_ACK),
    spawnReq: dv.getUint8(INPUT_FRAME.SPAWN_REQ),
  };
}

export { AXIS };
