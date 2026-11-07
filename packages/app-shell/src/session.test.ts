import { defaultFlags, type Flags } from '@flying/platform-web';
import { ARENA, ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge/layout';
import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS } from '@flying/ui';
import { sessionFromFlags, sessionOverrides } from './session.ts';

const flags = (over: Partial<Flags>): Flags => ({ ...defaultFlags(), ...over });

describe('sessionFromFlags', () => {
  it('starts on the runway under Standard assist with full g-effects by default', () => {
    expect(sessionFromFlags(defaultFlags(), false)).toEqual({
      spawn: SPAWN_KIND.RUNWAY,
      assist: ASSIST_LEVEL.STANDARD,
      gEffects: G_EFFECTS.FULL,
      arena: ARENA.RUGGED,
    });
  });

  it('takes each flag independently over the defaults', () => {
    expect(sessionFromFlags(flags({ spawn: 'air' }), false).spawn).toBe(SPAWN_KIND.AIR);
    expect(sessionFromFlags(flags({ assist: 'manual' }), false).assist).toBe(ASSIST_LEVEL.MANUAL);
    expect(sessionFromFlags(flags({ g: 'off' }), false).gEffects).toBe(G_EFFECTS.OFF);
    expect(sessionFromFlags(flags({ arena: 'plain' }), false).arena).toBe(ARENA.PLAIN);
    const all = sessionFromFlags(
      flags({ spawn: 'air', assist: 'full', g: 'reduced', arena: 'plain' }),
      false,
    );
    expect(all).toEqual({
      spawn: SPAWN_KIND.AIR,
      assist: ASSIST_LEVEL.FULL,
      gEffects: G_EFFECTS.REDUCED,
      arena: ARENA.PLAIN,
    });
  });

  it('flies a performance run from the air under Standard assist whatever the URL says', () => {
    const s = sessionFromFlags(
      flags({ spawn: 'runway', assist: 'manual', g: 'off', arena: 'plain' }),
      true,
    );
    expect(s).toEqual({
      spawn: SPAWN_KIND.AIR,
      assist: ASSIST_LEVEL.STANDARD,
      gEffects: G_EFFECTS.FULL,
      arena: ARENA.RUGGED,
    });
  });

  it("takes the player's settings over the defaults, and the URL over the settings", () => {
    const settings = {
      spawn: SPAWN_KIND.AIR,
      assist: ASSIST_LEVEL.REDUCED,
      gEffects: G_EFFECTS.OFF,
      arena: ARENA.PLAIN,
    };
    expect(sessionFromFlags(defaultFlags(), false, settings)).toEqual(settings);
    expect(sessionFromFlags(flags({ assist: 'full' }), false, settings)).toEqual({
      ...settings,
      assist: ASSIST_LEVEL.FULL,
    });
    // A performance run ignores both.
    expect(sessionFromFlags(flags({ assist: 'full' }), true, settings)).toEqual({
      spawn: SPAWN_KIND.AIR,
      assist: ASSIST_LEVEL.STANDARD,
      gEffects: G_EFFECTS.FULL,
      arena: ARENA.RUGGED,
    });
  });
});

describe('sessionOverrides', () => {
  it('is empty when the session is the stored settings', () => {
    const session = sessionFromFlags(defaultFlags(), false, DEFAULT_SETTINGS);
    expect(sessionOverrides(session, DEFAULT_SETTINGS)).toEqual({});
  });

  it('lists only what the URL changed', () => {
    const session = sessionFromFlags(flags({ spawn: 'air', assist: 'manual' }), false);
    expect(sessionOverrides(session, DEFAULT_SETTINGS)).toEqual({
      spawn: SPAWN_KIND.AIR,
      assist: ASSIST_LEVEL.MANUAL,
    });
  });

  it('lists the arena the URL chose, so the menu shows the arena that flies', () => {
    const session = sessionFromFlags(flags({ arena: 'plain' }), false, DEFAULT_SETTINGS);
    expect(sessionOverrides(session, DEFAULT_SETTINGS)).toEqual({ arena: ARENA.PLAIN });
  });

  it('is the whole story at Fly: the settings with the overrides over them make the session, with no flags', () => {
    // The player's menu shows `{ ...stored, ...overrides }`; flying it needs no second look at the URL.
    const session = sessionFromFlags(
      flags({ spawn: 'air', arena: 'plain' }),
      false,
      DEFAULT_SETTINGS,
    );
    const shown = { ...DEFAULT_SETTINGS, ...sessionOverrides(session, DEFAULT_SETTINGS) };
    expect(sessionFromFlags(defaultFlags(), false, shown)).toEqual(session);
    // The player changes the arena and the assist in the menu: their choice flies, not the URL's.
    const changed = { ...shown, arena: ARENA.RUGGED, assist: ASSIST_LEVEL.FULL };
    expect(sessionFromFlags(defaultFlags(), false, changed)).toEqual({
      spawn: SPAWN_KIND.AIR,
      assist: ASSIST_LEVEL.FULL,
      gEffects: G_EFFECTS.FULL,
      arena: ARENA.RUGGED,
    });
  });

  it('lists the scripted session of a performance run', () => {
    const session = sessionFromFlags(defaultFlags(), true, DEFAULT_SETTINGS);
    expect(sessionOverrides(session, DEFAULT_SETTINGS).spawn).toBe(SPAWN_KIND.AIR);
  });
});
