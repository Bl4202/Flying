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
