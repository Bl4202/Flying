import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { CLASSIFY_KEYS, countKey, isKey } from './classify.ts';

const common = readFileSync(new URL('../shaders/common.wgsl', import.meta.url), 'utf8');

/** The `vec3<f32>(r, g, b)` constant called `name` in common.wgsl, as bytes. */
function shaderKey(name: string): number[] {
  const m = new RegExp(`const ${name} : vec3<f32> = vec3<f32>\\(([^)]*)\\);`).exec(common);
  if (!m?.[1]) throw new Error(`no constant ${name}`);
  return m[1].split(',').map((v) => Math.round(Number(v) * 255));
}

describe('the classify keys', () => {
  it('match the constants in the shaders', () => {
    expect(shaderKey('KEY_SKY')).toEqual([...CLASSIFY_KEYS.sky]);
    expect(shaderKey('KEY_GLOBE')).toEqual([...CLASSIFY_KEYS.globe]);
    expect(shaderKey('KEY_MARKER')).toEqual([...CLASSIFY_KEYS.marker]);
    expect(shaderKey('KEY_MODEL')).toEqual([...CLASSIFY_KEYS.model]);
    expect(shaderKey('KEY_COCKPIT')).toEqual([...CLASSIFY_KEYS.cockpit]);
    expect(shaderKey('KEY_HUD')).toEqual([...CLASSIFY_KEYS.hud]);
  });

  it('match the terrain levels the shader derives: green 255 minus 64 per level', () => {
    expect(common).toMatch(
      /fn key_level\(level : f32\) -> vec3<f32> \{[^}]*\(255\.0 - level \* 64\.0\) \/ 255\.0/,
    );
    expect(CLASSIFY_KEYS.terrain.map((k) => k[1])).toEqual([0, 1, 2, 3].map((l) => 255 - 64 * l));
    for (const k of CLASSIFY_KEYS.terrain) expect([k[0], k[2]]).toEqual([0, 0]);
  });

  it('are pairwise far apart, so a blended edge never passes for another key', () => {
    const all = [
      CLASSIFY_KEYS.sky,
      CLASSIFY_KEYS.globe,
      CLASSIFY_KEYS.marker,
      CLASSIFY_KEYS.model,
      CLASSIFY_KEYS.cockpit,
      CLASSIFY_KEYS.hud,
      ...CLASSIFY_KEYS.terrain,
    ];
    for (const [i, a] of all.entries()) {
      for (const b of all.slice(i + 1)) {
        const d = Math.max(...a.map((v, k) => Math.abs(v - (b[k] ?? 0))));
        expect(d, `${a.join(',')} and ${b.join(',')}`).toBeGreaterThanOrEqual(60);
      }
    }
  });
});

describe('isKey and countKey', () => {
  const px = new Uint8Array([0, 0, 255, 255, 1, 0, 254, 255, 9, 9, 9, 255]);
  it('match within the tolerance', () => {
    expect(isKey(px, 0, CLASSIFY_KEYS.sky)).toBe(true);
    expect(isKey(px, 4, CLASSIFY_KEYS.sky)).toBe(true);
    expect(isKey(px, 8, CLASSIFY_KEYS.sky)).toBe(false);
    expect(isKey(px, 4, CLASSIFY_KEYS.sky, 0)).toBe(false);
  });
  it('count pixels', () => {
    expect(countKey(px, CLASSIFY_KEYS.sky)).toBe(2);
    expect(countKey(px, CLASSIFY_KEYS.globe)).toBe(0);
  });
});
