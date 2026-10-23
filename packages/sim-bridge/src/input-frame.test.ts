import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  decodeInputFrame,
  encodeInputFrame,
  neutralFrame,
  type InputFrame,
} from './input-frame.ts';
import { AXIS, INPUT_FRAME, INPUT_FLAG } from './layout.gen.ts';

interface Vector {
  axes: number[];
  buttons: string;
  flags: number;
  hats: [number, number];
  hex: string;
  seq: number;
  t_ms: number;
  t_sent_ms: number;
  assist_req: number;
  spawn_ack: number;
  spawn_req: number;
}
const vectors = JSON.parse(
  readFileSync(new URL('../../../tests/vectors/input_frames.json', import.meta.url), 'utf8'),
) as { record_bytes: number; frames: Vector[] };

const bytesOf = (hex: string): Uint8Array =>
  Uint8Array.from(hex.match(/../g) ?? [], (h) => Number.parseInt(h, 16));
const hexOf = (b: Uint8Array): string =>
  [...b].map((x) => x.toString(16).padStart(2, '0')).join('');

function frameOf(v: Vector): InputFrame {
  return {
    seq: v.seq,
    flags: v.flags,
    tMs: v.t_ms,
    tSentMs: v.t_sent_ms,
    axes: Float32Array.from(v.axes),
    buttons: BigInt(v.buttons),
    hats: v.hats,
    assistReq: v.assist_req,
    spawnAck: v.spawn_ack,
    spawnReq: v.spawn_req,
  };
}

describe('InputFrame against the Rust reference vectors', () => {
  it('has vectors to check, of the right size', () => {
    expect(vectors.record_bytes).toBe(INPUT_FRAME.SIZE);
    expect(vectors.frames.length).toBeGreaterThanOrEqual(9);
  });

  it('includes a frame that uses every v3 field', () => {
    const v3 = vectors.frames.find((v) => v.assist_req !== 0);
    expect(v3).toBeDefined();
    expect(v3?.spawn_ack).toBe(200);
    expect(v3?.spawn_req).toBe(2);
    expect(v3?.t_sent_ms).toBeGreaterThan(v3?.t_ms ?? Infinity);
    expect((v3?.flags ?? 0) & INPUT_FLAG.CHANGED).toBe(INPUT_FLAG.CHANGED);
  });

  it('encodes to exactly the bytes Rust encodes', () => {
    for (const v of vectors.frames) {
      const out = new Uint8Array(INPUT_FRAME.SIZE).fill(0xaa);
      encodeInputFrame(frameOf(v), out);
      expect(hexOf(out), `frame seq ${String(v.seq)}`).toBe(v.hex);
    }
  });

  it('decodes the bytes Rust encodes', () => {
    for (const v of vectors.frames) {
      const f = decodeInputFrame(bytesOf(v.hex));
      expect(f.seq).toBe(v.seq);
      expect(f.flags).toBe(v.flags);
      expect(f.tMs).toBe(v.t_ms);
      expect(f.tSentMs).toBe(v.t_sent_ms);
      expect([f.assistReq, f.spawnAck, f.spawnReq]).toEqual([
        v.assist_req,
        v.spawn_ack,
        v.spawn_req,
      ]);
      expect(f.buttons).toBe(BigInt(v.buttons));
      expect(f.hats).toEqual(v.hats);
      expect([...f.axes]).toEqual(v.axes.map(Math.fround));
    }
  });
});

describe('InputFrame', () => {
  it('has a neutral frame that encodes to zeros apart from the sequence number', () => {
    const out = new Uint8Array(INPUT_FRAME.SIZE).fill(0xff);
    encodeInputFrame(neutralFrame(7), out);
    expect(out[0]).toBe(7);
    expect(out.subarray(1).every((b) => b === 0)).toBe(true);
  });

  it('round-trips, including through a view that does not start at zero', () => {
    const f = neutralFrame(123);
    f.flags = INPUT_FLAG.NEUTRAL;
    f.tMs = 98765.4321;
    f.axes[AXIS.PITCH] = -0.75;
    f.axes[AXIS.THROTTLE] = 0.5;
    f.buttons = (1n << 63n) | 1n;
    f.hats = [3, 255];
    f.tSentMs = 98768.5;
    f.assistReq = 3;
    f.spawnAck = 255;
    f.spawnReq = 1;
    const backing = new Uint8Array(INPUT_FRAME.SIZE + 13);
    const view = backing.subarray(13);
    encodeInputFrame(f, view);
    expect(decodeInputFrame(view)).toEqual(f);
    expect(backing.subarray(0, 13).every((b) => b === 0)).toBe(true);
  });

  it('rejects buffers that are too small', () => {
    expect(() => {
      encodeInputFrame(neutralFrame(), new Uint8Array(95));
    }).toThrow(RangeError);
    expect(() => decodeInputFrame(new Uint8Array(95))).toThrow(RangeError);
  });
});
