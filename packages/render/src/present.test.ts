import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { parsePresentBundle, PresentError, Slice } from '@flying/sim-bridge/present';
import {
  HUD_LAYER_TYPES,
  parseCockpit,
  parseFont,
  parseHud,
  parseModel,
  renderPresentation,
} from './present.ts';

type Json = Record<string, unknown>;
const fixture = (): Json =>
  JSON.parse(
    readFileSync(new URL('../../../tests/vectors/present_bundle.json', import.meta.url), 'utf8'),
  ) as Json;
const table = (b: Json, name: string, id: string): Json =>
  (b[name] as Record<string, Json>)[id] as Json;
const kestrel = {
  hud: 'core:hud/kestrel',
  cockpit: 'core:cpt/kestrel',
  model: 'core:mdl/kestrel',
  font: 'core:fnt/hudstroke',
};

function pointerOf(f: () => unknown): string {
  try {
    f();
  } catch (e) {
    if (e instanceof PresentError) return e.pointer;
    throw e;
  }
  throw new Error('did not throw');
}

describe('the Kestrel HUD', () => {
  const hud = parseHud(new Slice(table(fixture(), 'huds', kestrel.hud)));

  it('has the data of GDD 54.2 and 54.13', () => {
    expect(hud.tfovDeg).toEqual([28, 20]);
    expect(hud.eyeBoxMm).toEqual([120, 80, 160]);
    expect(hud.colour).toBe('#35ff6a');
    expect(hud.glowPx).toBe(0.6);
    expect(hud.textMrad).toBe(12);
    expect(hud.font).toBe('core:fnt/hudstroke');
  });

  it('has one of each element type, in the order drawn', () => {
    expect(hud.layers.map((l) => l.type)).toEqual([...HUD_LAYER_TYPES]);
    const speed = hud.layers.find((l) => l.type === 'speed_tape');
    expect(speed).toMatchObject({ channel: 'cas', windowKn: 100, tickKn: 10, machFrom: 0.6 });
    const fpm = hud.layers.find((l) => l.type === 'flight_path_marker');
    expect(fpm).toMatchObject({ cageBelowMps: 20 });
    const alt = hud.layers.find((l) => l.type === 'altitude_tape');
    expect(alt).toMatchObject({ radarBelowFt: 1500 });
    const warn = hud.layers.find((l) => l.type === 'warnings');
    expect(warn).toMatchObject({ maxLines: 3 });
  });

  it('names where a wrong value is', () => {
    const edit = (f: (h: Json) => void): Slice => {
      const h = structuredClone(table(fixture(), 'huds', kestrel.hud));
      f(h);
      return new Slice(h);
    };
    const layers = (h: Json): Json[] => h['layers'] as Json[];
    const index = (h: Json, type: string): number => layers(h).findIndex((l) => l['type'] === type);
    expect(pointerOf(() => parseHud(edit((h) => (h['colour'] = 'green'))))).toBe('/colour');
    expect(
      pointerOf(() =>
        parseHud(
          edit((h) => {
            const i = index(h, 'speed_tape');
            (layers(h)[i] as Json)['channel'] = 'ias';
          }),
        ),
      ),
    ).toMatch(/^\/layers\/\d+\/channel$/);
    expect(
      pointerOf(() =>
        parseHud(
          edit((h) => {
            layers(h)[0] = { ...(layers(h)[0] as Json), type: 'radar_scope' };
          }),
        ),
      ),
    ).toBe('/layers/0/type');
    expect(
      pointerOf(() =>
        parseHud(
          edit((h) => {
            delete (layers(h)[0] as Json)['size_mrad'];
          }),
        ),
      ),
    ).toBe('/layers/0');
    expect(
      pointerOf(() =>
        parseHud(
          edit((h) => {
            layers(h).push({ ...(layers(h)[0] as Json), id: 'again' });
          }),
        ),
      ),
    ).toMatch(/^\/layers\/\d+$/);
    expect(
      pointerOf(() =>
        parseHud(
          edit((h) => {
            const i = index(h, 'warnings');
            (layers(h)[i] as Json)['max_lines'] = 5;
          }),
        ),
      ),
    ).toMatch(/^\/layers\/\d+\/max_lines$/);
  });
});

describe('the Kestrel cockpit', () => {
  const k = parseCockpit(new Slice(table(fixture(), 'cockpits', kestrel.cockpit)));

  it('has the design eye, head limits and a glass that covers the HUD', () => {
    expect(k.eyePointM).toEqual([3.6, 0, -0.85]);
    expect(k.headLimitsDeg).toEqual({ yaw: 135, pitchUp: 60, pitchDown: 60 });
    expect(k.hudGlassM).toHaveLength(4);
    // The four rays of the 28 by 20 degree field, from the eye, pass through the glass plane
    // inside its edges (the baker checks this too; this is the same check in TypeScript).
    const [c0, c1, , c3] = k.hudGlassM;
    const plane = c0[0];
    const y = [c0[1], c1[1]];
    const z = [c0[2], c3[2]];
    for (const [su, sv] of [
      [-1, -1],
      [-1, 1],
      [1, -1],
      [1, 1],
    ] as const) {
      const t = plane - k.eyePointM[0];
      const hy = k.eyePointM[1] + su * Math.tan((14 * Math.PI) / 180) * t;
      const hz = k.eyePointM[2] - sv * Math.tan((10 * Math.PI) / 180) * t;
      expect(hy).toBeGreaterThanOrEqual(Math.min(...y));
      expect(hy).toBeLessThanOrEqual(Math.max(...y));
      expect(hz).toBeGreaterThanOrEqual(Math.min(...z));
      expect(hz).toBeLessThanOrEqual(Math.max(...z));
    }
  });

  it('keeps every box at least the near plane away from the eye', () => {
    for (const b of [k.glareShield, k.panel, ...k.consoles]) {
      const d = Math.hypot(
        ...[0, 1, 2].map((a) => {
          const e = k.eyePointM[a] ?? 0;
          return Math.max((b.minM[a] ?? 0) - e, 0, e - (b.maxM[a] ?? 0));
        }),
      );
      expect(d, b.id).toBeGreaterThanOrEqual(0.35);
    }
    expect(k.canopyBows).toHaveLength(4);
  });
});

describe('the Kestrel model', () => {
  const m = parseModel(new Slice(table(fixture(), 'models', kestrel.model)));

  it('matches the aircraft: nose, span and the three gear legs', () => {
    expect(m.fuselage[0]?.xM).toBe(7);
    expect(
      Math.max(...m.surfaces.flatMap((s) => [Math.abs(s.rootLeM[1]), Math.abs(s.tipLeM[1])])),
    ).toBeCloseTo(4.9, 6);
    expect(m.gear.map((g) => g.id).sort()).toEqual(['main_l', 'main_r', 'nose']);
    expect(m.gear.find((g) => g.id === 'nose')?.contactM).toEqual([4.58, 0, 2.29]);
  });

  it('binds each moving part to a channel the sim publishes', () => {
    const channels = new Set(m.parts.map((p) => p.channel));
    expect(channels).toEqual(
      new Set([
        'elevator_deg',
        'aileron_deg',
        'rudder_deg',
        'flap_deg',
        'airbrake',
        'gear_retract',
        'afterburner',
      ]),
    );
    expect(m.parts.filter((p) => p.motion === 'glow')).toHaveLength(1);
    const ids = new Set(m.parts.map((p) => p.id));
    for (const s of m.surfaces) if (s.part !== undefined) expect(ids.has(s.part)).toBe(true);
    for (const g of m.gear) expect(ids.has(g.part)).toBe(true);
    expect(ids.has(m.nozzle.part)).toBe(true);
  });

  it('refuses a part that does not exist, naming the surface', () => {
    const b = fixture();
    const model = table(b, 'models', kestrel.model);
    (model['surfaces'] as Json[])[1] = { ...(model['surfaces'] as Json[])[1], part: 'ghost' };
    expect(pointerOf(() => parseModel(new Slice(model)))).toBe('/surfaces/1/part');
    const wrongChannel = structuredClone(table(fixture(), 'models', kestrel.model));
    (wrongChannel['parts'] as Json[])[0] = {
      ...(wrongChannel['parts'] as Json[])[0],
      channel: 'elevator_rad',
    };
    expect(pointerOf(() => parseModel(new Slice(wrongChannel)))).toBe('/parts/0/channel');
  });
});

describe('the HUD font', () => {
  const f = parseFont(new Slice(table(fixture(), 'fonts', kestrel.font)));

  it('draws every character the HUD uses', () => {
    for (const ch of '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ .-+/%:°α') {
      expect(f.glyphs.has(ch), ch).toBe(true);
    }
    expect(f.cell).toEqual([4, 6]);
    expect(f.advance).toBe(6);
    expect(f.glyphs.get(' ')?.strokes).toEqual([]);
    expect(f.glyphs.get('0')?.strokes).toHaveLength(2);
  });

  it('keeps every point inside the cell', () => {
    for (const g of f.glyphs.values()) {
      for (const stroke of g.strokes) {
        expect(stroke.length, g.ch).toBeGreaterThanOrEqual(2);
        for (const [x, y] of stroke) {
          expect(x).toBeGreaterThanOrEqual(0);
          expect(x).toBeLessThanOrEqual(f.cell[0]);
          expect(y).toBeGreaterThanOrEqual(0);
          expect(y).toBeLessThanOrEqual(f.cell[1]);
        }
      }
    }
  });
});

describe('renderPresentation', () => {
  it('collects everything for the Kestrel and nothing for an unknown aircraft', () => {
    const bundle = parsePresentBundle(fixture());
    const p = renderPresentation(bundle, 'core:ac/kestrel');
    expect(p?.hud.layers.length).toBe(13);
    expect(p?.model.parts.length).toBe(12);
    expect(p?.cockpit.canopyBows.length).toBe(4);
    expect(p?.font.glyphs.size).toBe(45);
    expect(renderPresentation(bundle, 'core:ac/ghost')).toBeUndefined();
  });

  it('refuses a bundle that has a HUD but no cockpit', () => {
    const b = fixture();
    const aircraft = b['aircraft'] as Record<string, Json>;
    delete (aircraft['core:ac/kestrel'] as Json)['cockpit'];
    expect(() => renderPresentation(parsePresentBundle(b), 'core:ac/kestrel')).toThrow(
      /has no cockpit/,
    );
  });
});
