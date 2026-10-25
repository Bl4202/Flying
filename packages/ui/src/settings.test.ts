import { describe, expect, it } from 'vitest';
import { memoryKv, type Kv } from '@flying/platform-web';
import { ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge';
import {
  DEFAULT_SETTINGS,
  migrateSettings,
  parseSettings,
  SETTINGS_KEY,
  SettingsStore,
} from './settings.ts';

describe('parseSettings', () => {
  it('defaults to the runway, Standard assist and full g-effects', () => {
    expect(DEFAULT_SETTINGS).toMatchObject({
      spawn: SPAWN_KIND.RUNWAY,
      assist: ASSIST_LEVEL.STANDARD,
      gEffects: G_EFFECTS.FULL,
      gfxStrength: 100,
      mouseMode: 'off',
      mouseSensitivity: 1,
      autoGcas: false,
    });
    expect(parseSettings(undefined)).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings(null)).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings('text')).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings([1, 2])).toEqual(DEFAULT_SETTINGS);
  });

  it('keeps usable values', () => {
    const stored = {
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
    expect(parseSettings(stored)).toEqual(stored);
  });

  it('replaces each unusable field with its default and keeps the rest', () => {
    const p = parseSettings({
      assist: 9,
      gEffects: 1.5,
      gfxStrength: 400,
      spawn: 'air',
      mouseMode: 'laser',
      mouseSensitivity: Number.NaN,
      mouseSpring: -1,
      autoGcas: 'yes',
      extra: 1,
    });
    expect(p).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings({ assist: 3, mouseMode: 'laser' })).toEqual({
      ...DEFAULT_SETTINGS,
      assist: 3,
    });
  });

  it('rounds the screen-effects strength to a whole per cent', () => {
    expect(parseSettings({ gfxStrength: 33.6 }).gfxStrength).toBe(34);
  });

  it('migrates nothing yet', () => {
    const raw = { version: 1, assist: 1 };
    expect(migrateSettings(raw)).toBe(raw);
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
});
