import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { parsePresentBundle, PresentError, Slice } from '@flying/sim-bridge/present';
import { audioProfileFor, parseAudioProfile } from './profile.ts';

type Json = Record<string, unknown>;
const fixture = (): Json =>
  JSON.parse(
    readFileSync(new URL('../../../tests/vectors/present_bundle.json', import.meta.url), 'utf8'),
  ) as Json;
const kestrel = (): Json =>
  structuredClone(
    (fixture()['audio_profiles'] as Record<string, Json>)['core:aud/kestrel'] as Json,
  );

function pointerOf(f: () => unknown): string {
  try {
    f();
  } catch (e) {
    if (e instanceof PresentError) return e.pointer;
    throw e;
  }
  throw new Error('did not throw');
}

describe('the Kestrel audio profile', () => {
  const p = audioProfileFor(parsePresentBundle(fixture()), 'core:ac/kestrel');

  it('has the engine of GDD C.13', () => {
    expect(p?.engine.bladeCount).toBe(24);
    expect(p?.engine.idleRpm).toBe(8000);
    expect(p?.engine.militaryRpm).toBe(10400);
    expect(p?.engine.burnerRoarDb).toBe(6);
    expect(p?.engine.harmonicLevelsDb[0]).toBe(0);
  });

  it('has an rpm curve that rises from a stopped engine to military power', () => {
    const curve = p?.engine.rpmByCore ?? [];
    expect(curve[0]?.[0]).toBe(0);
    expect(curve.at(-1)).toEqual([1, 10400]);
    expect(curve.some((c) => c[1] === 8000)).toBe(true);
    for (let i = 1; i < curve.length; i++) {
      expect(curve[i]?.[0]).toBeGreaterThan(curve[i - 1]?.[0] ?? 0);
      expect(curve[i]?.[1]).toBeGreaterThan(curve[i - 1]?.[1] ?? 0);
    }
  });

  it('has the cockpit, the tones and a mix that ducks the aircraft bus', () => {
    expect(p?.cockpit.canopyLowpassHz).toBe(3000);
    expect(p?.cockpit.canopyAttenuationDb).toBe(-18);
    expect(p?.mix.duckDb).toBe(-6);
    expect(p?.warnings.stallHorn.pulseMaxHz).toBeGreaterThan(
      p?.warnings.stallHorn.pulseMinHz ?? 99,
    );
    expect(p?.warnings.pullUp.highHz).toBeGreaterThan(p?.warnings.pullUp.lowHz ?? 99);
    expect(p?.aero.windSlopeDb).toBe(10);
  });
});

describe("the Kestrel's weapon sounds", () => {
  const p = audioProfileFor(parsePresentBundle(fixture()), 'core:ac/kestrel');
  const w = p?.weapons;

  it('are in the profile, the gun at its rate of fire (1 700 rounds a minute)', () => {
    expect(w?.gun.rateHz).toBeCloseTo(1700 / 60, 4);
    expect(w?.gun.pulseMs).toBe(14);
    expect(w?.enemyGun.refDistanceM).toBe(150);
    expect(w?.launch.highHz).toBeGreaterThan(w?.launch.lowHz ?? 99_999);
    expect(w?.growl).toEqual({ baseHz: 420, spanHz: 520, amHz: 22, levelDb: -18 });
    expect(w?.lockTone).toEqual({ freqHz: 1320, levelDb: -16 });
    expect(w?.hit.lengthMs).toBe(60);
    expect(w?.explosion.refDistanceM).toBe(500);
  });

  it('are optional: a profile without them parses, with none', () => {
    const k = kestrel();
    delete k['weapons'];
    expect(parseAudioProfile(new Slice(k)).weapons).toBeUndefined();
  });

  it('name the value that is wrong', () => {
    const k = kestrel();
    ((k['weapons'] as Json)['gun'] as Json)['pulse_ms'] = 0;
    expect(pointerOf(() => parseAudioProfile(new Slice(k)))).toBe('/weapons/gun/pulse_ms');
    const m = kestrel();
    delete (m['weapons'] as Json)['lock_tone'];
    expect(pointerOf(() => parseAudioProfile(new Slice(m)))).toBe('/weapons');
  });
});

describe('parseAudioProfile errors', () => {
  it('names where a wrong value is', () => {
    const bad = (edit: (p: Json) => void): string => {
      const p = kestrel();
      edit(p);
      return pointerOf(() => parseAudioProfile(new Slice(p)));
    };
    const section = (p: Json, name: string): Json => p[name] as Json;
    expect(bad((p) => (section(p, 'engine')['idle_rpm'] = 'fast'))).toBe('/engine/idle_rpm');
    expect(bad((p) => delete section(p, 'engine')['blade_count'])).toBe('/engine');
    expect(bad((p) => (section(p, 'engine')['blade_count'] = 2.5))).toBe('/engine/blade_count');
    expect(bad((p) => (section(p, 'aero')['wind_ref_qbar_pa'] = 0))).toBe('/aero/wind_ref_qbar_pa');
    expect(bad((p) => (section(p, 'mix')['duck_db'] = 6))).toBe('/mix/duck_db');
    expect(bad((p) => ((section(p, 'warnings')['stall_horn'] as Json)['freq_hz'] = 99_999))).toBe(
      '/warnings/stall_horn/freq_hz',
    );
    expect(bad((p) => (section(p, 'engine')['rpm_by_core'] = [[0, 1]]))).toBe(
      '/engine/rpm_by_core',
    );
  });

  it('refuses a bundle whose aircraft has no audio profile', () => {
    const b = fixture();
    delete ((b['aircraft'] as Record<string, Json>)['core:ac/kestrel'] as Json)['audio'];
    expect(() => audioProfileFor(parsePresentBundle(b), 'core:ac/kestrel')).toThrow(
      /has no audio profile/,
    );
    expect(audioProfileFor(parsePresentBundle(fixture()), 'core:ac/ghost')).toBeUndefined();
  });
});
