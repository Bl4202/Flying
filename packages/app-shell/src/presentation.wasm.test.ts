/**
 * The presentation loader against the real baked sim pack (`cargo xtask content`): the bundle has to
 * name the pack it was baked with.
 *
 * Skipped when the build outputs are absent, so `pnpm test` works on a fresh checkout. CI's build
 * job sets REQUIRE_WASM=1 after building, which turns "absent" into a failure instead.
 */
import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { parsePresentBundle } from '@flying/sim-bridge';
import { AIRCRAFT_ID, loadPresentation } from './presentation.ts';

const packUrl = new URL('../../../content-out/core.cpack', import.meta.url);
const built = existsSync(packUrl);
if (!built && process.env['REQUIRE_WASM'] === '1') {
  throw new Error('REQUIRE_WASM=1 but content-out/ is missing');
}

const text = (rel: string): string => readFileSync(new URL(rel, import.meta.url), 'utf8');
const vector = (): unknown => JSON.parse(text('../../../tests/vectors/present_bundle.json'));
const pack = (): Uint8Array => new Uint8Array(readFileSync(packUrl));

describe.skipIf(!built)('loadPresentation', () => {
  it("loads the Kestrel's HUD, cockpit, model, font and audio from a bundle that matches the pack", async () => {
    const p = await loadPresentation({
      bundle: () => Promise.resolve(parsePresentBundle(vector())),
      pack: () => Promise.resolve(pack()),
    });
    expect(p.error).toBeUndefined();
    expect(p.render?.hud).toBeDefined();
    expect(p.render?.model).toBeDefined();
    expect(p.render?.font).toBeDefined();
    expect(p.cockpit?.eyeBodyM).toHaveLength(3);
    expect(p.audio?.engine.bladeCount).toBe(24);
  });

  it('refuses a bundle baked with another pack, and says which hashes', async () => {
    const other = { ...(vector() as Record<string, unknown>), sim_pack_sha256: '0'.repeat(64) };
    const p = await loadPresentation({
      bundle: () => Promise.resolve(parsePresentBundle(other)),
      pack: () => Promise.resolve(pack()),
    });
    expect(p.render).toBeUndefined();
    expect(p.audio).toBeUndefined();
    expect(p.error).toContain('/sim_pack_sha256');
    expect(p.error).toContain('0'.repeat(64));
  });

  it('is not fatal when the bundle cannot be fetched or is malformed', async () => {
    const failed = await loadPresentation({
      bundle: () => Promise.reject(new Error('SYS-021: the presentation bundle returned 404')),
      pack: () => Promise.resolve(pack()),
    });
    expect(failed).toMatchObject({ render: undefined, audio: undefined });
    expect(failed.error).toContain('404');
    const malformed = await loadPresentation({
      bundle: () => Promise.resolve(parsePresentBundle({ schema: 'wrong' })),
      pack: () => Promise.resolve(pack()),
    });
    expect(malformed.error).toContain('/schema');
  });

  it('has nothing for an aircraft the bundle does not know', async () => {
    const p = await loadPresentation(
      {
        bundle: () => Promise.resolve(parsePresentBundle(vector())),
        pack: () => Promise.resolve(pack()),
      },
      'core:ac/ghost',
    );
    expect(p.render).toBeUndefined();
    expect(p.error).toContain('core:ac/ghost');
    expect(AIRCRAFT_ID).toBe('core:ac/kestrel');
  });
});
