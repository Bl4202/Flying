import { describe, expect, it } from 'vitest';
import { COUNTER, PROFILE, STAGE } from './layout.gen.ts';
import { COUNTER_NAMES, decodeProfile, STAGE_NAMES } from './profile.ts';

describe('profile block', () => {
  it('names line up with the generated indices', () => {
    expect(STAGE_NAMES).toHaveLength(PROFILE.STAGE_COUNT);
    expect(STAGE_NAMES[STAGE.INGEST]).toBe('ingest');
    expect(STAGE_NAMES[STAGE.DYNAMICS]).toBe('dynamics');
    expect(STAGE_NAMES[STAGE.PUBLISH]).toBe('publish');
    expect(COUNTER_NAMES).toHaveLength(PROFILE.COUNTER_COUNT);
    expect(COUNTER_NAMES[COUNTER.INPUT_APPLIED]).toBe('input_applied');
    expect(COUNTER_NAMES[COUNTER.FDM_FAULTS]).toBe('fdm_faults');
    expect(COUNTER_NAMES[COUNTER.TRIM_FAILURES]).toBe('trim_failures');
    expect(COUNTER_NAMES[COUNTER.SNAPSHOTS]).toBe('snapshots');
    expect(COUNTER_NAMES[COUNTER.TOUCHDOWNS]).toBe('touchdowns');
    expect(COUNTER_NAMES[COUNTER.HARD_LANDINGS]).toBe('hard_landings');
    expect(COUNTER_NAMES[COUNTER.GLOC_EVENTS]).toBe('gloc_events');
    expect(COUNTER_NAMES[COUNTER.ASSIST_CHANGES]).toBe('assist_changes');
  });

  it('decodes every field from its offset', () => {
    const b = new Uint8Array(PROFILE.SIZE);
    const dv = new DataView(b.buffer);
    dv.setBigUint64(PROFILE.TICKS, 123_456n, true);
    dv.setFloat32(PROFILE.TICK_MS_LAST, 1.5, true);
    dv.setFloat32(PROFILE.TICK_MS_EWMA, 1.25, true);
    for (let i = 0; i < PROFILE.STAGE_COUNT; i++) {
      dv.setFloat32(PROFILE.STAGE_MS_LAST + 4 * i, i + 0.5, true);
      dv.setFloat32(PROFILE.STAGE_MS_EWMA + 4 * i, (i + 0.5) / 2, true);
    }
    for (let i = 0; i < PROFILE.COUNTER_COUNT; i++) {
      dv.setUint32(PROFILE.COUNTERS + 4 * i, 1000 + i, true);
    }
    const p = decodeProfile(b);
    expect(p.ticks).toBe(123_456);
    expect([p.tickMsLast, p.tickMsEwma]).toEqual([1.5, 1.25]);
    expect(p.stageMsLast[STAGE.PUBLISH]).toBe(10.5);
    expect(p.stageMsEwma[STAGE.AI]).toBe(1.75);
    expect(p.counters[COUNTER.SNAPSHOTS]).toBe(1007);
    expect(p.stageMsLast).toHaveLength(PROFILE.STAGE_COUNT);
  });

  it('copies, so the block can be reused', () => {
    const b = new Uint8Array(PROFILE.SIZE);
    const p = decodeProfile(b);
    new DataView(b.buffer).setFloat32(PROFILE.STAGE_MS_LAST, 9, true);
    expect(p.stageMsLast[0]).toBe(0);
  });

  it('refuses a short block', () => {
    expect(() => decodeProfile(new Uint8Array(PROFILE.SIZE - 1))).toThrow(RangeError);
  });
});
