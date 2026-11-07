import { describe, expect, it } from 'vitest';
import { memoryKv, type Kv } from '@flying/platform-web';
import { ARENA, ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge';
import {
  applySettingsPatch,
  DEFAULT_SETTINGS,
  exportSettings,
  importSettings,
  LEGACY_SETTINGS_KEY,
  migrateSettings,
  parseSettings,
  reducedMotionOn,
  SETTINGS_KEY,
  SettingsStore,
  type Settings,
} from './settings.ts';

/** A complete `settings/1` entry as v0.1b stored it, every field away from its default. */
const V1_FIXTURE = {
  version: 1,
  assist: 4,
  gEffects: 0,
  gfxStrength: 40,
  spawn: 1,
  mouseMode: 'stick',
  mouseSensitivity: 2.5,
  mouseSpring: 0.4,
  autoGcas: true,
};

describe('parseSettings', () => {
  it('defaults to the runway, Standard assist, full g-effects, the cockpit and the rugged arena', () => {
    expect(DEFAULT_SETTINGS).toMatchObject({
      version: 2,
      spawn: SPAWN_KIND.RUNWAY,
      assist: ASSIST_LEVEL.STANDARD,
      gEffects: G_EFFECTS.FULL,
      gfxStrength: 100,
      arena: ARENA.RUGGED,
      mouseMode: 'off',
      mouseSensitivity: 1,
      autoGcas: false,
      camera: 'cockpit',
      fovDeg: 75,
      cameraShake: 100,
      reducedMotion: 'auto',
      setupDone: false,
    });
    expect(DEFAULT_SETTINGS.hud).toEqual({
      palette: 'green',
      brightness: 100,
      declutter: 0,
      overlayScale: 100,
    });
    expect(DEFAULT_SETTINGS.video).toEqual({
      renderer: 'auto',
      preset: 'auto',
      resolutionScale: 100,
    });
    expect(DEFAULT_SETTINGS.audio).toEqual({
      master: 80,
      aircraft: 100,
      warnings: 100,
      ui: 100,
      muteWhenHidden: true,
    });
    for (const raw of [undefined, null, 'text', [1, 2]]) {
      expect(parseSettings(raw)).toEqual(DEFAULT_SETTINGS);
    }
  });

  it('keeps usable values, nested groups included', () => {
    const stored: Settings = {
      version: 2,
      assist: 4,
      gEffects: 0,
      gfxStrength: 40,
      spawn: 1,
      arena: 1,
      mouseMode: 'stick',
      mouseSensitivity: 2.5,
      mouseSpring: 0.4,
      autoGcas: true,
      camera: 'chase',
      fovDeg: 100,
      cameraShake: 20,
      reducedMotion: 'on',
      hud: { palette: 'high-contrast', brightness: 60, declutter: 2, overlayScale: 150 },
      video: { renderer: 'webgl2', preset: 'low', resolutionScale: 80 },
      audio: { master: 50, aircraft: 70, warnings: 90, ui: 10, muteWhenHidden: false },
      setupDone: true,
    };
    expect(parseSettings(stored)).toEqual(stored);
  });

  it('replaces each unusable field with its default and keeps the rest', () => {
    const p = parseSettings({
      assist: 9,
      gEffects: 1.5,
      gfxStrength: 400,
      spawn: 'air',
      arena: 7,
      mouseMode: 'laser',
      mouseSensitivity: Number.NaN,
      mouseSpring: -1,
      autoGcas: 'yes',
      camera: 'orbit',
      fovDeg: 500,
      cameraShake: -5,
      reducedMotion: 'maybe',
      hud: { palette: 'pink', brightness: 5, declutter: 3, overlayScale: 'big' },
      video: { renderer: 'vulkan', preset: 'ultra', resolutionScale: 1 },
      audio: { master: 101, aircraft: 'loud', warnings: null, ui: -1, muteWhenHidden: 1 },
      setupDone: 'yes',
      extra: 1,
    });
    expect(p).toEqual(DEFAULT_SETTINGS);
    expect(
      parseSettings({ assist: 3, mouseMode: 'laser', hud: { palette: 'amber', declutter: 9 } }),
    ).toEqual({
      ...DEFAULT_SETTINGS,
      assist: 3,
      hud: { ...DEFAULT_SETTINGS.hud, palette: 'amber' },
    });
    expect(parseSettings({ hud: 'amber', video: 3, audio: [] })).toEqual(DEFAULT_SETTINGS);
  });

  it('rounds the percentages to whole numbers, but not the field of view or the mouse', () => {
    expect(parseSettings({ gfxStrength: 33.6 }).gfxStrength).toBe(34);
    expect(parseSettings({ audio: { master: 49.5 } }).audio.master).toBe(50);
    expect(parseSettings({ fovDeg: 83.5 }).fovDeg).toBe(83.5);
    expect(parseSettings({ mouseSensitivity: 1.05 }).mouseSensitivity).toBe(1.05);
  });

  it('does not share its default groups with a caller', () => {
    const a = parseSettings(undefined);
    a.hud.palette = 'amber';
    expect(parseSettings(undefined).hud.palette).toBe('green');
    expect(DEFAULT_SETTINGS.hud.palette).toBe('green');
  });
});

describe('migration from settings/1 (F2)', () => {
  it('keeps every field of a v0.1b entry and adds the new ones at their defaults', () => {
    const v1Fields: Record<string, unknown> = { ...V1_FIXTURE };
    delete v1Fields['version'];
    const p = parseSettings(migrateSettings(V1_FIXTURE));
    expect(p).toMatchObject(v1Fields);
    expect(p.version).toBe(2);
    expect(p).toEqual({
      ...DEFAULT_SETTINGS,
      ...v1Fields,
      hud: DEFAULT_SETTINGS.hud,
      video: DEFAULT_SETTINGS.video,
      audio: DEFAULT_SETTINGS.audio,
    });
    // The first-run flow is new to this player, and is still to be seen.
    expect(p.setupDone).toBe(false);
  });

  it('keeps a field only a v1 entry could have had wrong, as the default', () => {
    expect(parseSettings({ version: 1, assist: 99 }).assist).toBe(ASSIST_LEVEL.STANDARD);
  });
});

describe('applySettingsPatch', () => {
  it('lays a patch over the settings, groups in part', () => {
    const next = applySettingsPatch(DEFAULT_SETTINGS, {
      camera: 'chase',
      hud: { palette: 'cyan' },
      audio: { master: 10 },
    });
    expect(next.camera).toBe('chase');
    expect(next.hud).toEqual({ ...DEFAULT_SETTINGS.hud, palette: 'cyan' });
    expect(next.audio).toEqual({ ...DEFAULT_SETTINGS.audio, master: 10 });
    expect(next.video).toEqual(DEFAULT_SETTINGS.video);
    expect(DEFAULT_SETTINGS.hud.palette).toBe('green');
  });
});

describe('reducedMotionOn', () => {
  it('is the setting, or the system preference when the setting is auto', () => {
    expect(reducedMotionOn('on', false)).toBe(true);
    expect(reducedMotionOn('off', true)).toBe(false);
    expect(reducedMotionOn('auto', true)).toBe(true);
    expect(reducedMotionOn('auto', false)).toBe(false);
  });
});

describe('export and import (F2)', () => {
  const custom: Settings = applySettingsPatch(DEFAULT_SETTINGS, {
    assist: 3,
    arena: 1,
    camera: 'chase',
    fovDeg: 88.5,
    mouseSensitivity: 1.35,
    hud: { palette: 'amber', brightness: 70, declutter: 1 },
    video: { preset: 'medium', resolutionScale: 130 },
    audio: { master: 33, muteWhenHidden: false },
    setupDone: true,
  });

  it('writes stable, readable text', () => {
    const text = exportSettings(custom);
    expect(text.endsWith('}\n')).toBe(true);
    expect(text.startsWith('{\n  "version": 2,\n  "assist": 3,')).toBe(true);
    expect(exportSettings(custom)).toBe(text);
  });

  it('round-trips byte for byte', () => {
    for (const s of [DEFAULT_SETTINGS, custom, parseSettings(V1_FIXTURE)]) {
      const text = exportSettings(s);
      const back = importSettings(text);
      expect(back.ok).toBe(true);
      if (back.ok) {
        expect(back.settings).toEqual(parseSettings(s));
        expect(exportSettings(back.settings)).toBe(text);
      }
    }
  });

  it('imports a file from before a setting existed, taking the default for what it lacks', () => {
    const r = importSettings(JSON.stringify(V1_FIXTURE));
    expect(r.ok && r.settings).toEqual(parseSettings(V1_FIXTURE));
    const partial = importSettings(JSON.stringify({ version: 2, hud: { palette: 'white' } }));
    expect(partial.ok && partial.settings.hud).toEqual({
      ...DEFAULT_SETTINGS.hud,
      palette: 'white',
    });
  });

  it('names every field it cannot use, by pointer, and applies nothing', () => {
    const r = importSettings(
      JSON.stringify({
        version: 2,
        assist: 9,
        camera: 'orbit',
        hud: { palette: 'pink', brightness: 500 },
        audio: 'loud',
        fovDeg: 75,
      }),
    );
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.problems.map((p) => p.pointer)).toEqual([
        '/assist',
        '/camera',
        '/hud/palette',
        '/hud/brightness',
        '/audio',
      ]);
      expect(r.problems.every((p) => p.message.length > 0)).toBe(true);
    }
  });

  it('refuses text that is not settings', () => {
    for (const text of ['<html>', '', '[]', '3', 'null']) {
      const r = importSettings(text);
      expect(r.ok).toBe(false);
      if (!r.ok) expect(r.problems[0]?.pointer).toBe('');
    }
    const wrong = importSettings(JSON.stringify({ version: 9 }));
    expect(!wrong.ok && wrong.problems[0]?.pointer).toBe('/version');
  });
});

describe('SettingsStore', () => {
  it('starts with the defaults and loads what was stored', async () => {
    const kv = memoryKv();
    await kv.set(SETTINGS_KEY, { ...DEFAULT_SETTINGS, assist: 1, mouseMode: 'aim' });
    const store = new SettingsStore(kv);
    expect(store.value).toEqual(DEFAULT_SETTINGS);
    await store.load();
    expect(store.value).toMatchObject({ assist: 1, mouseMode: 'aim' });
    expect(store.backend).toBe('memory');
  });

  it('reads v0.1b settings once, keeps them, and leaves the old entry alone', async () => {
    const kv = memoryKv();
    await kv.set(LEGACY_SETTINGS_KEY, V1_FIXTURE);
    const store = new SettingsStore(kv);
    await store.load();
    expect(store.value).toMatchObject({ assist: 4, mouseMode: 'stick', autoGcas: true });
    expect(await kv.get(SETTINGS_KEY)).toBeUndefined();
    await store.update({ camera: 'chase' });
    expect((await kv.get<Settings>(SETTINGS_KEY))?.assist).toBe(4);
    expect(await kv.get(LEGACY_SETTINGS_KEY)).toEqual(V1_FIXTURE);
  });

  it('prefers settings/2 to settings/1', async () => {
    const kv = memoryKv();
    await kv.set(LEGACY_SETTINGS_KEY, V1_FIXTURE);
    await kv.set(SETTINGS_KEY, { ...DEFAULT_SETTINGS, assist: 1 });
    const store = new SettingsStore(kv);
    await store.load();
    expect(store.value.assist).toBe(1);
  });

  it('survives a store that fails to read', async () => {
    const kv: Kv = {
      backend: 'indexeddb',
      get: () => Promise.reject(new Error('denied')),
      set: () => Promise.reject(new Error('denied')),
      delete: () => Promise.reject(new Error('denied')),
    };
    const store = new SettingsStore(kv);
    await expect(store.load()).resolves.toEqual(DEFAULT_SETTINGS);
    await expect(store.update({ assist: 3 })).resolves.toMatchObject({ assist: 3 });
    expect(store.value.assist).toBe(3); // held for the visit though it could not be stored
  });

  it('updates, stores and tells the listeners', async () => {
    const kv = memoryKv();
    const store = new SettingsStore(kv);
    const seen: number[] = [];
    const off = store.onChange((s) => seen.push(s.assist));
    await store.update({ assist: 4 });
    expect(seen).toEqual([4]);
    expect((await kv.get<{ assist: number }>(SETTINGS_KEY))?.assist).toBe(4);
    off();
    await store.update({ assist: 1 });
    expect(seen).toEqual([4]);
  });

  it('changes one member of a group and keeps the others', async () => {
    const store = new SettingsStore(memoryKv());
    await store.update({ hud: { palette: 'amber' } });
    await store.update({ hud: { brightness: 50 } });
    expect(store.value.hud).toEqual({ ...DEFAULT_SETTINGS.hud, palette: 'amber', brightness: 50 });
  });

  it('ignores an out-of-range value and keeps the old one, not the default', async () => {
    const store = new SettingsStore(memoryKv());
    await store.update({ assist: 4, gfxStrength: 20 });
    await store.update({ assist: 99, gfxStrength: 55 });
    expect(store.value).toMatchObject({ assist: 4, gfxStrength: 55 });
  });

  it('can set a field back to its default value', async () => {
    const store = new SettingsStore(memoryKv());
    await store.update({ assist: 4 });
    await store.update({ assist: ASSIST_LEVEL.STANDARD });
    expect(store.value.assist).toBe(ASSIST_LEVEL.STANDARD);
  });

  it('resets everything but the first-run flow, which stays done', async () => {
    const kv = memoryKv();
    const store = new SettingsStore(kv);
    await store.update({ assist: 4, camera: 'chase', setupDone: true, audio: { master: 5 } });
    await store.reset();
    expect(store.value).toEqual({ ...DEFAULT_SETTINGS, setupDone: true });
    expect((await kv.get<Settings>(SETTINGS_KEY))?.assist).toBe(ASSIST_LEVEL.STANDARD);
  });

  it('replaces every setting with an imported set', async () => {
    const store = new SettingsStore(memoryKv());
    const seen: string[] = [];
    store.onChange((s) => seen.push(s.camera));
    const imported = applySettingsPatch(DEFAULT_SETTINGS, { camera: 'chase', assist: 1 });
    await store.replace(imported);
    expect(store.value).toEqual(imported);
    expect(seen).toEqual(['chase']);
  });
});
