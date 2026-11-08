import { defaultFlags, type Flags } from '@flying/platform-web';
import { ARENA, PS0 } from '@flying/perf';
import { describe, expect, it } from 'vitest';
import { scenarioOf, withScenarioView } from './perf-run.ts';

const flags = (over: Partial<Flags> = {}): Flags => ({ ...defaultFlags(), ...over });

describe('scenarioOf', () => {
  it('names the scenario a ?perf= value asks for, and none without one', () => {
    expect(scenarioOf('ps0')).toBe(PS0);
    expect(scenarioOf('arena')).toBe(ARENA);
    expect(scenarioOf(false)).toBeUndefined();
  });
});

describe('withScenarioView', () => {
  it('lays the arena scenario under the flags: cockpit, High, 1920 x 1080', () => {
    const f = withScenarioView(flags({ perf: 'arena' }), ARENA);
    expect([f.camera, f.quality, f.res]).toEqual([
      'cockpit',
      'high',
      { width: 1920, height: 1080 },
    ]);
  });

  it('lets the URL win, so the chase run and a smaller buffer are one flag away', () => {
    const f = withScenarioView(
      flags({ perf: 'arena', camera: 'chase', quality: 'low', res: { width: 640, height: 360 } }),
      ARENA,
    );
    expect([f.camera, f.quality, f.res]).toEqual(['chase', 'low', { width: 640, height: 360 }]);
  });

  it('leaves the flags of the bare loop, and of no run, as they are', () => {
    const f = flags({ perf: 'ps0' });
    expect(withScenarioView(f, PS0)).toBe(f);
    expect(withScenarioView(f, undefined)).toBe(f);
  });
});
