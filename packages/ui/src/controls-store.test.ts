import { describe, expect, it } from 'vitest';
import { memoryKv, type Kv } from '@flying/platform-web';
import {
  builtinProfile,
  duplicateProfile,
  serializeProfile,
  type ControlsProfile,
} from '@flying/input';
import { ACTIVE_KEY, ControlsStore, INDEX_KEY, profileKey } from './controls-store.ts';

const mine = (name = 'Mine'): ControlsProfile =>
  duplicateProfile(builtinProfile('default-km'), [], name);

describe('ControlsStore', () => {
  it("lists the built-in profiles first and has nothing of the player's at the start", async () => {
    const store = new ControlsStore(memoryKv());
    await store.load();
    expect(store.all().map((p) => p.id)).toEqual([
      'default-km',
      'gamepad-standard',
      'generic-joystick',
    ]);
    expect(store.user()).toEqual([]);
    expect(store.activeId).toBeUndefined();
    expect(store.problems).toEqual([]);
    expect(store.isBuiltin('default-km')).toBe(true);
    expect(store.isBuiltin('mine')).toBe(false);
  });

  it('saves a profile and finds it again after a reload', async () => {
    const kv = memoryKv();
    const a = new ControlsStore(kv);
    await a.load();
    const p = mine();
    expect(await a.save(p)).toBe(true);
    expect(a.find('mine')).toEqual(p);
    expect(await kv.get<string>(profileKey('mine'))).toBe(serializeProfile(p));
    expect(await kv.get<string[]>(INDEX_KEY)).toEqual(['mine']);

    const b = new ControlsStore(kv);
    await b.load();
    expect(b.user()).toEqual([p]);
  });

  it('replaces a profile with the same id and keeps one entry in the index', async () => {
    const kv = memoryKv();
    const store = new ControlsStore(kv);
    await store.save(mine());
    await store.save({ ...mine(), name: 'Renamed' });
    expect(store.user()).toHaveLength(1);
    expect(store.find('mine')?.name).toBe('Renamed');
    expect(await kv.get<string[]>(INDEX_KEY)).toEqual(['mine']);
  });

  it('refuses to store over a built-in profile', async () => {
    const store = new ControlsStore(memoryKv());
    expect(await store.save(builtinProfile('default-km'))).toBe(false);
    expect(store.user()).toEqual([]);
  });

  it('remembers which profile is in use, and forgets it when that profile goes', async () => {
    const kv = memoryKv();
    const store = new ControlsStore(kv);
    await store.save(mine());
    await store.setActive('mine');
    expect(await kv.get<string>(ACTIVE_KEY)).toBe('mine');
    const again = new ControlsStore(kv);
    await again.load();
    expect(again.activeId).toBe('mine');
    await again.remove('mine');
    expect(again.activeId).toBeUndefined();
    expect(await kv.get<string>(ACTIVE_KEY)).toBeUndefined();
    expect(await kv.get<string>(profileKey('mine'))).toBeUndefined();
    expect(await kv.get<string[]>(INDEX_KEY)).toEqual([]);
  });

  it('accepts a built-in as the one in use, and clears the choice with undefined', async () => {
    const store = new ControlsStore(memoryKv());
    await store.setActive('gamepad-standard');
    expect(store.activeId).toBe('gamepad-standard');
    await store.setActive('nothing-like-it');
    expect(store.activeId).toBeUndefined();
    await store.setActive('default-km');
    await store.setActive(undefined);
    expect(store.activeId).toBeUndefined();
  });

  it('will not delete a built-in profile', async () => {
    const store = new ControlsStore(memoryKv());
    await store.remove('default-km');
    expect(store.find('default-km')).toBeDefined();
  });

  it('reports a stored profile that no longer parses, and loads the others', async () => {
    const kv = memoryKv();
    const good = mine('Good');
    await kv.set(INDEX_KEY, ['good', 'bad', 'liar', 'gone', 'default-km']);
    await kv.set(profileKey('good'), serializeProfile(good));
    await kv.set(profileKey('bad'), '{ "schema": "controls/9" }');
    await kv.set(profileKey('liar'), serializeProfile({ ...good, id: 'someone-else' }));
    const store = new ControlsStore(kv);
    await store.load();
    expect(store.user().map((p) => p.id)).toEqual(['good']);
    expect(store.problems.map((p) => p.id)).toEqual(['bad', 'liar']);
    expect(store.problems[0]?.errors[0]?.pointer).toBe('/schema');
    expect(store.problems[1]?.errors[0]?.pointer).toBe('/id');
  });

  it('survives a store that cannot be read or written', async () => {
    const kv: Kv = {
      backend: 'indexeddb',
      get: () => Promise.reject(new Error('denied')),
      set: () => Promise.reject(new Error('denied')),
      delete: () => Promise.reject(new Error('denied')),
    };
    const store = new ControlsStore(kv);
    await expect(store.load()).resolves.toBeUndefined();
    expect(await store.save(mine())).toBe(true);
    expect(store.user()).toHaveLength(1); // held for this visit
    await store.setActive('mine');
    expect(store.activeId).toBe('mine');
  });

  it('exports and imports a profile; an import that parses gets an id of its own', async () => {
    const store = new ControlsStore(memoryKv());
    await store.save(mine());
    const text = store.exportText(mine());
    const r = await store.importText(text);
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.profile.id).toBe('mine-2');
      expect(store.find('mine-2')?.bindings).toEqual(mine().bindings);
    }
    // A built-in's id is taken too.
    const builtinText = serializeProfile(builtinProfile('gamepad-standard'));
    const b = await store.importText(builtinText);
    expect(b.ok && b.profile.id).toBe('gamepad-standard-2');
  });

  it('lists every problem of an import by pointer and stores nothing', async () => {
    const store = new ControlsStore(memoryKv());
    const bad = JSON.stringify({
      schema: 'controls/1',
      id: 'x',
      name: 'x',
      deviceClass: 'foot',
      bindings: 'no',
    });
    const r = await store.importText(bad);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.errors.map((e) => e.pointer)).toEqual(['/deviceClass', '/bindings']);
    expect(store.user()).toEqual([]);
    const notJson = await store.importText('<html>');
    expect(notJson.ok).toBe(false);
  });

  it('tells the listeners when anything changes', async () => {
    const store = new ControlsStore(memoryKv());
    let n = 0;
    const off = store.onChange(() => {
      n += 1;
    });
    await store.save(mine());
    await store.setActive('mine');
    await store.remove('mine');
    expect(n).toBe(3);
    off();
    await store.save(mine());
    expect(n).toBe(3);
  });
});
