import { defaultFlags, type Flags } from '@flying/platform-web';
import { ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge/layout';
import { describe, expect, it } from 'vitest';
import { sessionFromFlags } from './session.ts';

const flags = (over: Partial<Flags>): Flags => ({ ...defaultFlags(), ...over });

describe('sessionFromFlags', () => {
  it('starts on the runway under Standard assist with full g-effects by default', () => {
    expect(sessionFromFlags(defaultFlags(), false)).toEqual({
      spawn: SPAWN_KIND.RUNWAY,
      assist: ASSIST_LEVEL.STANDARD,
      gEffects: G_EFFECTS.FULL,
    });
  });

  it('takes each flag independently over the defaults', () => {
    expect(sessionFromFlags(flags({ spawn: 'air' }), false).spawn).toBe(SPAWN_KIND.AIR);
    expect(sessionFromFlags(flags({ assist: 'manual' }), false).assist).toBe(ASSIST_LEVEL.MANUAL);
    expect(sessionFromFlags(flags({ g: 'off' }), false).gEffects).toBe(G_EFFECTS.OFF);
    const all = sessionFromFlags(flags({ spawn: 'air', assist: 'full', g: 'reduced' }), false);
    expect(all).toEqual({
      spawn: SPAWN_KIND.AIR,
      assist: ASSIST_LEVEL.FULL,
      gEffects: G_EFFECTS.REDUCED,
    });
  });

  it('flies a performance run from the air under Standard assist whatever the URL says', () => {
    const s = sessionFromFlags(flags({ spawn: 'runway', assist: 'manual', g: 'off' }), true);
    expect(s).toEqual({
      spawn: SPAWN_KIND.AIR,
      assist: ASSIST_LEVEL.STANDARD,
      gEffects: G_EFFECTS.FULL,
    });
  });
});
