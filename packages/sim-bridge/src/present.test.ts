import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { CPACK_HEADER } from './layout.gen.ts';
import {
  checkBundleMatchesPack,
  cpackContentHash,
  parsePresentBundle,
  PresentError,
  Slice,
} from './present.ts';

const fixture = (): Record<string, unknown> =>
  JSON.parse(
    readFileSync(new URL('../../../tests/vectors/present_bundle.json', import.meta.url), 'utf8'),
  ) as Record<string, unknown>;

/** A pack-shaped buffer: the header's magic, version and a content hash of `fill`. */
function fakePack(hash: string): Uint8Array {
  const b = new Uint8Array(CPACK_HEADER.H_CONTENT_HASH + CPACK_HEADER.CONTENT_HASH_BYTES + 16);
  const dv = new DataView(b.buffer);
  dv.setUint32(CPACK_HEADER.H_MAGIC, CPACK_HEADER.MAGIC, true);
  dv.setUint32(CPACK_HEADER.H_VERSION, CPACK_HEADER.VERSION, true);
  for (let i = 0; i < CPACK_HEADER.CONTENT_HASH_BYTES; i++) {
    b[CPACK_HEADER.H_CONTENT_HASH + i] = Number.parseInt(hash.slice(2 * i, 2 * i + 2), 16);
  }
  return b;
}

function pointerOf(f: () => unknown): string {
  try {
    f();
  } catch (e) {
    if (e instanceof PresentError) return e.pointer;
    throw e;
  }
  throw new Error('did not throw');
}

describe('Slice', () => {
  const s = new Slice({ a: 1.5, b: 'x', c: [1, 2, 3], d: { e: true }, 'f/g': 2 }, '/root');

  it('reads typed values and keeps count of where it is', () => {
    expect(s.n('a')).toBe(1.5);
    expect(s.s('b')).toBe('x');
    expect(s.at('c').tuple(3)).toEqual([1, 2, 3]);
    expect(s.at('d').at('e').bool()).toBe(true);
    expect(s.at('c').items()[1]?.pointer).toBe('/root/c/1');
    expect(s.at('f/g').pointer).toBe('/root/f~1g');
    expect(s.at('b').oneOf(['x', 'y'] as const)).toBe('x');
    expect(s.keys()).toContain('d');
    expect(s.has('a') && !s.has('z')).toBe(true);
    expect(s.opt('z')).toBeUndefined();
    expect(s.at('c').numbers()).toEqual([1, 2, 3]);
    expect(s.at('c').items()[0]?.int()).toBe(1);
  });

  it('fails with the pointer of the wrong value', () => {
    expect(pointerOf(() => s.at('zzz'))).toBe('/root');
    expect(pointerOf(() => s.n('b'))).toBe('/root/b');
    expect(pointerOf(() => s.n('a', 2))).toBe('/root/a');
    expect(pointerOf(() => s.at('a').int())).toBe('/root/a');
    expect(pointerOf(() => s.at('c').tuple(2))).toBe('/root/c');
    expect(pointerOf(() => s.at('b').oneOf(['y'] as const))).toBe('/root/b');
    expect(pointerOf(() => s.at('d').items())).toBe('/root/d');
    expect(pointerOf(() => s.at('c').at('x'))).toBe('/root/c');
    expect(pointerOf(() => s.at('a').bool())).toBe('/root/a');
    expect(pointerOf(() => s.at('a').str())).toBe('/root/a');
    expect(() => new Slice(null).at('x')).toThrow(/SYS-040.*\(whole file\).*found null/);
  });

  it('does not accept NaN or infinity as a number', () => {
    expect(() => new Slice(Number.NaN).num()).toThrow(/expected a number/);
    expect(() => new Slice(Infinity).num()).toThrow(/expected a number/);
  });
});

describe('parsePresentBundle', () => {
  it("reads the baker's bundle", () => {
    const b = parsePresentBundle(fixture());
    expect(b.simPackSha256).toMatch(/^[0-9a-f]{64}$/);
    expect(b.aircraftIds()).toEqual(['core:ac/kestrel']);
    const a = b.forAircraft('core:ac/kestrel');
    expect(a?.hud?.s('id')).toBe('core:hud/kestrel');
    expect(a?.cockpit?.s('id')).toBe('core:cpt/kestrel');
    expect(a?.model?.s('id')).toBe('core:mdl/kestrel');
    expect(a?.audio?.s('id')).toBe('core:aud/kestrel');
    expect(a?.font?.s('id')).toBe('core:fnt/hudstroke');
    expect(b.forAircraft('core:ac/ghost')).toBeUndefined();
  });

  it('refuses another format, a missing table and a bad hash, naming where', () => {
    expect(pointerOf(() => parsePresentBundle({ ...fixture(), schema: 'present-bundle/2' }))).toBe(
      '/schema',
    );
    const noFonts = fixture();
    delete noFonts['fonts'];
    expect(pointerOf(() => parsePresentBundle(noFonts))).toBe('');
    expect(pointerOf(() => parsePresentBundle({ ...fixture(), sim_pack_sha256: 'ABC' }))).toBe(
      '/sim_pack_sha256',
    );
    expect(pointerOf(() => parsePresentBundle([]))).toBe('');
  });
});

describe('the sim pack check', () => {
  const hash = 'ab'.repeat(32);

  it('reads the content hash from the pack header', () => {
    expect(cpackContentHash(fakePack(hash))).toBe(hash);
  });

  it('refuses what is not a pack', () => {
    expect(() => cpackContentHash(new Uint8Array(10))).toThrow(/SYS-041.*too short/);
    const wrongMagic = fakePack(hash);
    wrongMagic[0] = 0;
    expect(() => cpackContentHash(wrongMagic)).toThrow(/wrong magic/);
    const wrongVersion = fakePack(hash);
    wrongVersion[CPACK_HEADER.H_VERSION] = 99;
    expect(() => cpackContentHash(wrongVersion)).toThrow(/another format version/);
  });

  it('accepts the pack the bundle was baked with and names both hashes otherwise', () => {
    const bundle = parsePresentBundle({ ...fixture(), sim_pack_sha256: hash });
    expect(() => {
      checkBundleMatchesPack(bundle, fakePack(hash));
    }).not.toThrow();
    const other = 'cd'.repeat(32);
    expect(() => {
      checkBundleMatchesPack(bundle, fakePack(other));
    }).toThrow(new RegExp(`${hash}.*${other}`));
  });
});
