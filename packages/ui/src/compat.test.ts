import { describe, expect, it } from 'vitest';
import { defaultFlags, type DeepFeatures } from '@flying/platform-web';
import {
  COMPAT_ACK_KEY,
  evaluateCompat,
  readCompatAck,
  shouldShowCompat,
  writeCompatAck,
  type CompatInput,
  type CompatModes,
  type CompatRowId,
  type CompatState,
  type Verdict,
} from './compat.ts';
import { memoryKv } from '@flying/platform-web';
import { STRINGS } from './strings.ts';

/** A desktop Chrome on a host that sends the isolation headers: everything available. */
const FULL: DeepFeatures = {
  crossOriginIsolated: true,
  sharedArrayBuffer: true,
  wasm: true,
  wasmSimd: true,
  wasmThreads: true,
  serviceWorker: true,
  webgpu: true,
  offscreenCanvas: true,
  webgpuAdapter: true,
  adapterInfo: 'apple metal-3',
  webgl2: true,
  opfs: true,
  gamepad: true,
};

function input(
  features: Partial<DeepFeatures> = {},
  flags: Partial<CompatInput['flags']> = {},
): CompatInput {
  return { features: { ...FULL, ...features }, flags: { ...defaultFlags(), ...flags } };
}

const ALL_OK: Record<CompatRowId, CompatState> = {
  wasm: 'ok',
  simd: 'ok',
  renderer: 'ok',
  isolation: 'ok',
  threads: 'ok',
  offscreen: 'ok',
  sw: 'ok',
  opfs: 'ok',
  gamepad: 'ok',
};

const FULL_MODES: CompatModes = {
  variant: 'threads',
  snapshotChannel: 'shared',
  inputChannel: 'shared',
  renderHost: 'worker',
  renderer: 'webgpu',
  sleeper: 'atomics',
};

interface Case {
  name: string;
  input: CompatInput;
  /** States that differ from "everything ok". */
  states: Partial<Record<CompatRowId, CompatState>>;
  modes: Partial<CompatModes>;
  verdict: Verdict;
  codes: string[];
}

/** AC-53.5: twelve simulated browser configurations. */
const MATRIX: Case[] = [
  {
    name: '1. Chrome on a host with isolation headers',
    input: input(),
    states: {},
    modes: {},
    verdict: 'ok',
    codes: [],
  },
  {
    name: '2. Chrome on a static host, no isolation yet',
    input: input({ crossOriginIsolated: false, sharedArrayBuffer: false }),
    states: { isolation: 'degraded', threads: 'degraded' },
    modes: {
      variant: 'single',
      snapshotChannel: 'message',
      inputChannel: 'message',
      sleeper: 'timer',
    },
    verdict: 'degraded',
    codes: [],
  },
  {
    name: '3. Chrome with a blocklisted GPU: WebGL2 only',
    input: input({ webgpu: true, webgpuAdapter: false, adapterInfo: '' }),
    states: { renderer: 'degraded' },
    modes: { renderer: 'webgl2' },
    verdict: 'degraded',
    codes: [],
  },
  {
    name: '4. Firefox: isolated, no WebGPU',
    input: input({ webgpu: false, webgpuAdapter: false, adapterInfo: '' }),
    states: { renderer: 'degraded' },
    modes: { renderer: 'webgl2' },
    verdict: 'degraded',
    codes: [],
  },
  {
    name: '5. Safari-like: isolated but no WASM threads, no OffscreenCanvas, no WebGPU',
    input: input({
      wasmThreads: false,
      offscreenCanvas: false,
      webgpu: false,
      webgpuAdapter: false,
      adapterInfo: '',
      gamepad: false,
    }),
    states: {
      threads: 'degraded',
      offscreen: 'degraded',
      renderer: 'degraded',
      gamepad: 'degraded',
    },
    modes: { variant: 'single', renderHost: 'main', renderer: 'webgl2' },
    verdict: 'degraded',
    codes: [],
  },
  {
    name: '6. Neither WebGPU nor WebGL2',
    input: input({ webgpu: false, webgpuAdapter: false, adapterInfo: '', webgl2: false }),
    states: { renderer: 'missing' },
    modes: { renderer: 'none' },
    verdict: 'fatal',
    codes: ['SYS-103'],
  },
  {
    name: '7. No WebAssembly at all',
    input: input({ wasm: false, wasmSimd: false, wasmThreads: false }),
    states: { wasm: 'missing', simd: 'missing', threads: 'degraded' },
    modes: { variant: undefined, snapshotChannel: 'shared' },
    verdict: 'fatal',
    codes: ['SYS-101'],
  },
  {
    name: '8. WebAssembly without SIMD (Safari 15)',
    input: input({ wasmSimd: false }),
    states: { simd: 'missing', threads: 'degraded' },
    modes: { variant: undefined },
    verdict: 'fatal',
    codes: ['SYS-102'],
  },
  {
    name: '9. Locked-down private window: no Service Worker, OPFS or gamepad',
    input: input({ serviceWorker: false, opfs: false, gamepad: false }),
    states: { sw: 'degraded', opfs: 'degraded', gamepad: 'degraded' },
    modes: {},
    verdict: 'degraded',
    codes: [],
  },
  {
    name: '10. ?renderer=webgpu on a machine without a WebGPU adapter',
    input: input({ webgpuAdapter: false, adapterInfo: '' }, { renderer: 'webgpu' }),
    states: { renderer: 'missing' },
    modes: { renderer: 'none' },
    verdict: 'fatal',
    codes: ['SYS-103'],
  },
  {
    name: '11. ?renderer=null on a machine with no graphics',
    input: input(
      { webgpu: false, webgpuAdapter: false, adapterInfo: '', webgl2: false },
      { renderer: 'null' },
    ),
    states: {},
    modes: { renderer: 'null' },
    verdict: 'ok',
    codes: [],
  },
  {
    name: '12. Test switches force copies, the main thread and timers on a full machine',
    input: input({}, { transfer: true, mainrender: true, timer: true }),
    states: {},
    modes: {
      snapshotChannel: 'message',
      inputChannel: 'message',
      renderHost: 'main',
      sleeper: 'timer',
    },
    verdict: 'ok',
    codes: [],
  },
];

describe('evaluateCompat: the twelve-configuration matrix (AC-53.5)', () => {
  it.each(MATRIX)('$name', (c) => {
    const report = evaluateCompat(c.input);
    expect(Object.fromEntries(report.rows.map((r) => [r.id, r.state]))).toEqual({
      ...ALL_OK,
      ...c.states,
    });
    expect(report.modes).toEqual({ ...FULL_MODES, ...c.modes });
    expect(report.verdict).toBe(c.verdict);
    expect(report.codes).toEqual(c.codes);
  });

  it('offers a remedy for every row that is not fine, and none for the rest', () => {
    for (const c of MATRIX) {
      for (const row of evaluateCompat(c.input).rows) {
        expect(row.remedyKey !== undefined, `${c.name} / ${row.id}`).toBe(row.state !== 'ok');
      }
    }
  });

  it('only required rows can be missing, and only they carry codes', () => {
    for (const c of MATRIX) {
      for (const row of evaluateCompat(c.input).rows) {
        if (row.state === 'missing') expect(row.required, row.id).toBe(true);
        if (row.code !== undefined) expect(row.state, row.id).toBe('missing');
      }
    }
  });

  it('refers only to strings that exist', () => {
    for (const c of MATRIX) {
      for (const row of evaluateCompat(c.input).rows) {
        for (const key of [row.labelKey, row.detailKey, row.remedyKey]) {
          if (key !== undefined) expect(key in STRINGS, key).toBe(true);
        }
      }
    }
  });

  it('puts the adapter name in the WebGPU row', () => {
    const row = evaluateCompat(input()).rows.find((r) => r.id === 'renderer');
    expect(row?.detailParams).toEqual({ adapter: 'apple metal-3' });
  });

  it('lists the rows in a fixed order', () => {
    expect(evaluateCompat(input()).rows.map((r) => r.id)).toEqual([
      'wasm',
      'simd',
      'renderer',
      'isolation',
      'threads',
      'offscreen',
      'sw',
      'opfs',
      'gamepad',
    ]);
  });
});

describe('signature and acknowledgement', () => {
  it('is the same for the same states and differs when one changes', () => {
    const a = evaluateCompat(input());
    const b = evaluateCompat(input({ adapterInfo: 'another gpu name' }));
    const c = evaluateCompat(input({ opfs: false }));
    expect(b.signature).toBe(a.signature);
    expect(c.signature).not.toBe(a.signature);
  });

  it('shows the screen on first run, on a change, when fatal, or when forced; otherwise not', () => {
    const ok = evaluateCompat(input());
    const ack = { signature: ok.signature, at: 1 };
    expect(shouldShowCompat({ forced: false, report: ok, ack: undefined })).toBe(true);
    expect(shouldShowCompat({ forced: false, report: ok, ack })).toBe(false);
    expect(shouldShowCompat({ forced: true, report: ok, ack })).toBe(true);
    expect(shouldShowCompat({ forced: false, report: ok, ack: { signature: 'old', at: 1 } })).toBe(
      true,
    );
    const fatal = evaluateCompat(input({ wasm: false }));
    expect(
      shouldShowCompat({
        forced: false,
        report: fatal,
        ack: { signature: fatal.signature, at: 1 },
      }),
    ).toBe(true);
  });

  it('stores and reads back the acknowledgement', async () => {
    const kv = memoryKv();
    const report = evaluateCompat(input());
    expect(await readCompatAck(kv)).toBeUndefined();
    await writeCompatAck(kv, report, 1234);
    expect(await readCompatAck(kv)).toEqual({ signature: report.signature, at: 1234 });
  });

  it('treats a damaged stored value as never acknowledged', async () => {
    const kv = memoryKv();
    await kv.set(COMPAT_ACK_KEY, { signature: 5 });
    expect(await readCompatAck(kv)).toBeUndefined();
    await kv.set(COMPAT_ACK_KEY, 'x');
    expect(await readCompatAck(kv)).toBeUndefined();
  });

  it('survives a store that throws', async () => {
    const broken = {
      backend: 'memory' as const,
      get: () => Promise.reject(new Error('no')),
      set: () => Promise.reject(new Error('no')),
      delete: () => Promise.reject(new Error('no')),
    };
    expect(await readCompatAck(broken)).toBeUndefined();
    await expect(writeCompatAck(broken, evaluateCompat(input()), 1)).resolves.toBeUndefined();
  });
});
