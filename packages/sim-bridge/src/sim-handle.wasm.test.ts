/**
 * The sim handle against the real built modules (`cargo xtask wasm`, `cargo xtask content`).
 *
 * Skipped when the build outputs are absent, so `pnpm test` works on a fresh checkout. CI's build
 * job sets REQUIRE_WASM=1 after building, which turns "absent" into a failure instead.
 */
import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { encodeInputFrame, neutralFrame } from './input-frame.ts';
import {
  ABI,
  ASSIST_LEVEL,
  AXIS,
  CORE_STATE,
  G_EFFECTS,
  HASH_KIND,
  INPUT_FRAME,
  INPUT_SCRIPT,
  SPAWN_KIND,
  TERRAIN,
} from './layout.gen.ts';
import { decodeProfile } from './profile.ts';
import { instantiateSim, type SimHandle } from './sim-handle.ts';
import { SnapshotReader } from './snapshot.ts';

const root = new URL('../../../', import.meta.url);
const wasmDir = new URL('wasm-out/', root);
const cpack = new URL('content-out/core.cpack', root);
const built = existsSync(new URL('wasm-manifest.json', wasmDir)) && existsSync(cpack);

if (!built && process.env['REQUIRE_WASM'] === '1') {
  throw new Error('REQUIRE_WASM=1 but wasm-out/ or content-out/ is missing');
}

const SEED = 0x1f1e_5eed_0000_0001n;
interface Golden {
  checkpoints: { tick: number; combined: string; content: string; ownship: string }[];
}
const readGolden = (name: string): Golden | undefined =>
  built
    ? (JSON.parse(readFileSync(new URL(`tests/golden/${name}`, root), 'utf8')) as Golden)
    : undefined;
const golden = readGolden('determinism.json');
const flight60 = readGolden('flight60.json');
const circuit = readGolden('circuit.json');

// The sessions the determinism suites run under (`cargo xtask determinism`, ADR-36): a golden
// is only reproduced by the session it was recorded under.
const ERRATIC_SESSION = {
  spawn: SPAWN_KIND.AIR,
  assist: ASSIST_LEVEL.STANDARD,
  gEffects: G_EFFECTS.FULL,
};
const FLIGHT60_SESSION = {
  spawn: SPAWN_KIND.AIR,
  assist: ASSIST_LEVEL.MANUAL,
  gEffects: G_EFFECTS.OFF,
};
const CIRCUIT_SESSION = {
  spawn: SPAWN_KIND.RUNWAY,
  assist: ASSIST_LEVEL.STANDARD,
  gEffects: G_EFFECTS.FULL,
};

async function load(variant: 'single' | 'threads', hostNow?: () => number): Promise<SimHandle> {
  const manifest = JSON.parse(readFileSync(new URL('wasm-manifest.json', wasmDir), 'utf8')) as {
    threads_initial_pages: number;
  };
  const bytes = readFileSync(new URL(`sim.${variant}.wasm`, wasmDir));
  return instantiateSim(
    bytes,
    variant,
    manifest.threads_initial_pages,
    hostNow ? { now: hostNow } : undefined,
  );
}

const pack = (): Uint8Array => new Uint8Array(readFileSync(cpack));
const hex = (v: bigint): string => v.toString(16).padStart(16, '0');

describe.skipIf(!built).each(['single', 'threads'] as const)('sim handle (%s)', (variant) => {
  it('reports the ABI version it was built against', async () => {
    expect((await load(variant)).abiVersion()).toBe(ABI.VERSION);
  });

  it('starts idle and only steps once started', async () => {
    const sim = await load(variant);
    expect(sim.coreState()).toBe(CORE_STATE.NONE);
    sim.init(SEED);
    expect(sim.coreState()).toBe(CORE_STATE.IDLE);
    expect(sim.step(5)).toBe(0);
    expect(sim.start()).toBe(true);
    expect(sim.start()).toBe(false);
    expect(sim.coreState()).toBe(CORE_STATE.RUNNING);
    expect(sim.step(5)).toBe(5);
    expect(sim.tick()).toBe(5n);
    expect(sim.pause()).toBe(true);
    expect(sim.step(5)).toBe(0);
    expect(sim.resume()).toBe(true);
    expect(sim.step(1)).toBe(1);
  });

  it('reproduces the golden state hash at tick 10 000 with content and the scripted pilot', async () => {
    const sim = await load(variant);
    sim.init(SEED);
    sim.loadContent(pack());
    sim.configure(ERRATIC_SESSION);
    expect(sim.setInputScript(INPUT_SCRIPT.ERRATIC)).toBe(true);
    expect(sim.start()).toBe(true);
    sim.step(10_000);
    const expected = golden?.checkpoints.find((c) => c.tick === 10_000);
    expect(expected).toBeDefined();
    expect(hex(sim.hash(HASH_KIND.COMBINED))).toBe(expected?.combined);
    expect(hex(sim.hash(HASH_KIND.CONTENT))).toBe(expected?.content);
  });

  it('flies the 60 s scripted flight to the golden hashes (AC-11.7)', async () => {
    const sim = await load(variant);
    sim.init(SEED);
    sim.loadContent(pack());
    sim.configure(FLIGHT60_SESSION);
    expect(sim.setInputScript(INPUT_SCRIPT.FLIGHT60)).toBe(true);
    expect(sim.start()).toBe(true);
    for (const tick of [600, 3_600]) {
      sim.step(tick - Number(sim.tick()));
      const expected = flight60?.checkpoints.find((c) => c.tick === tick);
      expect(expected).toBeDefined();
      expect(hex(sim.hash(HASH_KIND.OWNSHIP))).toBe(expected?.ownship);
      expect(hex(sim.hash(HASH_KIND.COMBINED))).toBe(expected?.combined);
    }
  });

  it("flies the virtual tester's circuit to the golden hashes (ADR-33)", async () => {
    const sim = await load(variant);
    sim.init(SEED);
    sim.loadContent(pack());
    sim.configure(CIRCUIT_SESSION);
    expect(sim.setInputScript(INPUT_SCRIPT.CIRCUIT)).toBe(true);
    expect(sim.start()).toBe(true);
    // Take-off, the first turn and the landing: 600 is on the roll, 6 000 in the circuit,
    // 14 400 after the stop.
    for (const tick of [600, 6_000, 14_400]) {
      sim.step(tick - Number(sim.tick()));
      const expected = circuit?.checkpoints.find((c) => c.tick === tick);
      expect(expected).toBeDefined();
      expect(hex(sim.hash(HASH_KIND.OWNSHIP))).toBe(expected?.ownship);
      expect(hex(sim.hash(HASH_KIND.COMBINED))).toBe(expected?.combined);
    }
  });

  it('refuses damaged content, content after start, and unknown scripts', async () => {
    const sim = await load(variant);
    sim.init(SEED);
    const damaged = pack();
    damaged[damaged.length - 1] = (damaged[damaged.length - 1] ?? 0) ^ 1;
    expect(() => {
      sim.loadContent(damaged);
    }).toThrow(/SYS-007/);
    expect(() => {
      sim.loadContent(new Uint8Array(0));
    }).toThrow(/SYS-006/);
    expect(sim.setInputScript(99)).toBe(false);
    const before = sim.hash(HASH_KIND.COMBINED);
    sim.loadContent(pack());
    expect(sim.hash(HASH_KIND.COMBINED)).not.toBe(before);
    sim.start();
    expect(() => {
      sim.loadContent(pack());
    }).toThrow(/SYS-007/);
  });

  it('carries host input to the core and reports it in the snapshot', async () => {
    const sim = await load(variant);
    sim.init(SEED);
    sim.start();
    const frames = new Uint8Array(ABI.INPUT_BATCH * INPUT_FRAME.SIZE);
    const f = neutralFrame(41);
    f.axes[AXIS.PITCH] = 0.5;
    f.axes[AXIS.THROTTLE] = 0.5;
    f.tMs = 1234.5;
    encodeInputFrame(f, frames);
    expect(sim.pushInputs(frames, 1)).toBe(1);
    sim.step(2);
    const snap = new SnapshotReader(sim.snapshot());
    expect(snap.valid()).toBe(true);
    expect(snap.tick()).toBe(2);
    expect(snap.lastInputSeq()).toBe(41);
    expect(snap.inputTimeMs()).toBe(1234.5);
    expect(sim.pushInputs(frames, 100)).toBe(ABI.INPUT_BATCH);
    expect(() => sim.pushInputs(new Uint8Array(10), 1)).toThrow(RangeError);
  });

  it('times the stages through the host clock only while profiling is on', async () => {
    let t = 0;
    const sim = await load(variant, () => (t += 0.25));
    sim.init(SEED);
    sim.start();
    sim.step(10);
    expect(decodeProfile(sim.profile()).tickMsLast).toBe(0);
    sim.setProfiling(true);
    sim.step(10);
    const p = decodeProfile(sim.profile());
    expect(p.ticks).toBeGreaterThan(0);
    expect(p.tickMsLast).toBeGreaterThan(0);
    expect(p.stageMsLast.some((ms) => ms > 0)).toBe(true);
    sim.setProfiling(false);
  });

  it('hands out a terrain patch and keeps views valid across the allocation', async () => {
    const sim = await load(variant);
    sim.init(SEED);
    sim.start();
    sim.step(3);
    const patch = sim.fillTerrain(0, 0, 100);
    expect(patch?.byteLength).toBe(TERRAIN.SIZE);
    const dv = new DataView(patch?.buffer ?? new ArrayBuffer(0), patch?.byteOffset ?? 0);
    expect(dv.getUint32(TERRAIN.H_POSTS, true)).toBe(TERRAIN.POSTS);
    expect(dv.getFloat32(TERRAIN.H_SPACING_M, true)).toBe(100);
    // The terrain fill may have grown memory; the snapshot view must still read correctly.
    expect(new SnapshotReader(sim.snapshot()).tick()).toBe(3);
    expect(sim.fillTerrain(Number.NaN, 0, 100)).toBeUndefined();
  });

  it('is deterministic: two instances agree at every step', async () => {
    const [a, b] = [await load(variant), await load(variant)];
    for (const s of [a, b]) {
      s.init(SEED);
      s.loadContent(pack());
      s.setInputScript(INPUT_SCRIPT.ERRATIC);
      s.start();
    }
    for (let i = 0; i < 20; i++) {
      a.step(137);
      b.step(137);
      expect(a.hash()).toBe(b.hash());
    }
  });
});
