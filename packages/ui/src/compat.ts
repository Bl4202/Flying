/**
 * The compat check (GDD 53.3): which browser features the game needs, which it can do without,
 * what it will do instead, and what the person can do about each gap.
 *
 * `evaluateCompat` is pure: it takes what the platform probes found and the URL flags, and returns
 * rows (as string keys, so the screen decides the wording), a verdict, the modes the app will run
 * in, and the error codes of anything fatal. AC-53.5 is tested against a matrix of twelve feature
 * sets.
 */
import {
  chooseVariant,
  type DeepFeatures,
  type Flags,
  type Kv,
  type Variant,
} from '@flying/platform-web';
import type { Params, StringKey } from './strings.ts';

export type CompatState = 'ok' | 'degraded' | 'missing';
export type Verdict = 'ok' | 'degraded' | 'fatal';
export type CompatRowId =
  'wasm' | 'simd' | 'renderer' | 'isolation' | 'threads' | 'offscreen' | 'sw' | 'opfs' | 'gamepad';

/** `SYS-101` no WebAssembly, `SYS-102` no SIMD, `SYS-103` no usable renderer. */
export type CompatCode = 'SYS-101' | 'SYS-102' | 'SYS-103';

export interface CompatRow {
  id: CompatRowId;
  /** A missing required feature makes the verdict fatal. */
  required: boolean;
  state: CompatState;
  labelKey: StringKey;
  detailKey: StringKey;
  detailParams: Params;
  /** What to do about it; absent when the row is fine. */
  remedyKey: StringKey | undefined;
  /** Set on a required row that is missing. */
  code: CompatCode | undefined;
}

export interface CompatModes {
  /** The WebAssembly build that will run; `undefined` when none can. */
  variant: Variant | undefined;
  snapshotChannel: 'shared' | 'message';
  inputChannel: 'shared' | 'message';
  renderHost: 'worker' | 'main';
  renderer: 'webgpu' | 'webgl2' | 'null' | 'none';
  sleeper: 'atomics' | 'timer';
}

export interface CompatReport {
  rows: CompatRow[];
  verdict: Verdict;
  modes: CompatModes;
  /** Codes of the missing required rows, in row order. */
  codes: CompatCode[];
  /** Identifies this set of row states; a change brings the screen back (see `shouldShowCompat`). */
  signature: string;
}

export interface CompatInput {
  features: DeepFeatures;
  flags: Pick<Flags, 'renderer' | 'transfer' | 'mainrender' | 'timer'>;
}

const NO_PARAMS: Params = {};

function optional(
  id: CompatRowId,
  available: boolean,
  keys: {
    label: StringKey;
    ok: StringKey;
    degraded: StringKey;
    remedy: StringKey;
  },
): CompatRow {
  return {
    id,
    required: false,
    state: available ? 'ok' : 'degraded',
    labelKey: keys.label,
    detailKey: available ? keys.ok : keys.degraded,
    detailParams: NO_PARAMS,
    remedyKey: available ? undefined : keys.remedy,
    code: undefined,
  };
}

function chooseRenderer(
  f: DeepFeatures,
  choice: CompatInput['flags']['renderer'],
): CompatModes['renderer'] {
  switch (choice) {
    case 'null':
      return 'null';
    case 'webgpu':
      return f.webgpuAdapter ? 'webgpu' : 'none';
    case 'webgl2':
      return f.webgl2 ? 'webgl2' : 'none';
    case 'auto':
      return f.webgpuAdapter ? 'webgpu' : f.webgl2 ? 'webgl2' : 'none';
  }
}

function rendererRow(
  f: DeepFeatures,
  choice: CompatInput['flags']['renderer'],
  renderer: CompatModes['renderer'],
): CompatRow {
  const base = {
    id: 'renderer',
    required: true,
    labelKey: 'compat.renderer.label',
    remedyKey: 'compat.renderer.remedy',
  } as const;
  switch (renderer) {
    case 'webgpu':
      return {
        ...base,
        state: 'ok',
        detailKey: f.adapterInfo === '' ? 'compat.renderer.webgpu-bare' : 'compat.renderer.webgpu',
        detailParams: { adapter: f.adapterInfo },
        remedyKey: undefined,
        code: undefined,
      };
    case 'webgl2':
      return {
        ...base,
        state: 'degraded',
        detailKey: 'compat.renderer.webgl2',
        detailParams: NO_PARAMS,
        remedyKey: 'compat.renderer.remedy',
        code: undefined,
      };
    case 'null':
      return {
        ...base,
        state: 'ok',
        detailKey: 'compat.renderer.null',
        detailParams: NO_PARAMS,
        remedyKey: undefined,
        code: undefined,
      };
    case 'none':
      return {
        ...base,
        state: 'missing',
        detailKey: choice === 'auto' ? 'compat.renderer.missing' : 'compat.renderer.forced-missing',
        detailParams: { choice },
        code: 'SYS-103',
      };
  }
}

export function evaluateCompat(input: CompatInput): CompatReport {
  const { features: f, flags } = input;
  const isolated = f.crossOriginIsolated && f.sharedArrayBuffer;
  const variant = chooseVariant(f);
  const renderer = chooseRenderer(f, flags.renderer);

  const rows: CompatRow[] = [
    {
      id: 'wasm',
      required: true,
      state: f.wasm ? 'ok' : 'missing',
      labelKey: 'compat.wasm.label',
      detailKey: f.wasm ? 'compat.wasm.ok' : 'compat.wasm.missing',
      detailParams: NO_PARAMS,
      remedyKey: f.wasm ? undefined : 'compat.wasm.remedy',
      code: f.wasm ? undefined : 'SYS-101',
    },
    {
      id: 'simd',
      required: true,
      state: f.wasmSimd ? 'ok' : 'missing',
      labelKey: 'compat.simd.label',
      detailKey: f.wasmSimd ? 'compat.simd.ok' : 'compat.simd.missing',
      detailParams: NO_PARAMS,
      remedyKey: f.wasmSimd ? undefined : 'compat.simd.remedy',
      // Without WebAssembly at all there is nothing to say about SIMD: SYS-101 covers it.
      code: f.wasmSimd || !f.wasm ? undefined : 'SYS-102',
    },
    rendererRow(f, flags.renderer, renderer),
    optional('isolation', isolated, {
      label: 'compat.isolation.label',
      ok: 'compat.isolation.ok',
      degraded: 'compat.isolation.degraded',
      remedy: 'compat.isolation.remedy',
    }),
    optional('threads', variant === 'threads', {
      label: 'compat.threads.label',
      ok: 'compat.threads.ok',
      degraded: 'compat.threads.degraded',
      remedy: 'compat.threads.remedy',
    }),
    optional('offscreen', f.offscreenCanvas, {
      label: 'compat.offscreen.label',
      ok: 'compat.offscreen.ok',
      degraded: 'compat.offscreen.degraded',
      remedy: 'compat.offscreen.remedy',
    }),
    optional('sw', f.serviceWorker, {
      label: 'compat.sw.label',
      ok: 'compat.sw.ok',
      degraded: 'compat.sw.degraded',
      remedy: 'compat.sw.remedy',
    }),
    optional('opfs', f.opfs, {
      label: 'compat.opfs.label',
      ok: 'compat.opfs.ok',
      degraded: 'compat.opfs.degraded',
      remedy: 'compat.opfs.remedy',
    }),
    optional('gamepad', f.gamepad, {
      label: 'compat.gamepad.label',
      ok: 'compat.gamepad.ok',
      degraded: 'compat.gamepad.degraded',
      remedy: 'compat.gamepad.remedy',
    }),
  ];

  const channel = isolated && !flags.transfer ? 'shared' : 'message';
  const modes: CompatModes = {
    variant,
    snapshotChannel: channel,
    inputChannel: channel,
    renderHost: f.offscreenCanvas && !flags.mainrender ? 'worker' : 'main',
    renderer,
    sleeper: isolated && !flags.timer ? 'atomics' : 'timer',
  };

  const codes = rows.flatMap((r) => (r.code === undefined ? [] : [r.code]));
  const fatal = rows.some((r) => r.required && r.state === 'missing');
  const verdict: Verdict = fatal ? 'fatal' : rows.some((r) => r.state !== 'ok') ? 'degraded' : 'ok';
  const signature = `${rows.map((r) => `${r.id}=${r.state}`).join(',')};variant=${variant ?? 'none'};renderer=${renderer}`;
  return { rows, verdict, modes, codes, signature };
}

/** What is stored once the person has seen the screen and continued. */
export interface CompatAck {
  signature: string;
  at: number;
}

export const COMPAT_ACK_KEY = 'compat-ack';

/** Whether the compat screen should be shown before the game starts. */
export function shouldShowCompat(args: {
  /** `?compat=1` */
  forced: boolean;
  report: CompatReport;
  ack: CompatAck | undefined;
}): boolean {
  return (
    args.forced ||
    args.report.verdict === 'fatal' ||
    args.ack === undefined ||
    args.ack.signature !== args.report.signature
  );
}

export async function readCompatAck(kv: Kv): Promise<CompatAck | undefined> {
  try {
    const value = await kv.get<unknown>(COMPAT_ACK_KEY);
    if (
      typeof value === 'object' &&
      value !== null &&
      typeof (value as { signature?: unknown }).signature === 'string' &&
      typeof (value as { at?: unknown }).at === 'number'
    ) {
      const { signature, at } = value as CompatAck;
      return { signature, at };
    }
  } catch {
    // an unreadable store counts as "never acknowledged"
  }
  return undefined;
}

export async function writeCompatAck(kv: Kv, report: CompatReport, at: number): Promise<void> {
  try {
    await kv.set(COMPAT_ACK_KEY, { signature: report.signature, at } satisfies CompatAck);
  } catch {
    // not being able to remember only means the screen shows again next time
  }
}
