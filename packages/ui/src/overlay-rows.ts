/**
 * What the debug overlay says, as data: `buildOverlayRows` turns the app's current numbers into
 * labelled rows, and nothing here touches the DOM, so the formatting is easy to test.
 */
import type { RenderStats } from '@flying/render/messages';
import { OWNSHIP_FLAG, quatRotate, STAGE, type Quantiles, type SimStats } from '@flying/sim-bridge';

export interface OverlayModel {
  build: string;
  /** How the app is running, already in words (`threads`, `shared`, ...). */
  variant: string;
  snapshotChannel: string;
  renderHost: string;
  sleeper: string;
  abi: number;
  /** 16-hex content hash of the loaded pack. */
  contentHash: string;
  /** The GPU adapter as the browser describes it, or empty. */
  adapter: string;
  sim: SimStats | undefined;
  render: RenderStats | undefined;
  paused: boolean;
  /** Input frames sent to the core and refused by a full queue. */
  input: { sent: number; refused: number } | undefined;
  /** The stored benchmark result as `profile / preset`, or undefined if none was run. */
  benchmark: string | undefined;
}

export interface OverlayRow {
  /** Stable id; the DOM test id is `ov-<id>`. */
  id: string;
  label: string;
  value: string;
}

const DASH = '–';

function fixed(n: number | undefined, digits: number, unit = ''): string {
  return n === undefined || !Number.isFinite(n) ? DASH : `${n.toFixed(digits)}${unit}`;
}

function q(qs: Quantiles | undefined, pick: 'avg' | 'p95' | 'p99' | 'max'): number | undefined {
  return qs === undefined || qs.count === 0 ? undefined : qs[pick];
}

const CORE_STATE = ['idle', 'loading', 'running', 'paused', 'ended'];

/** Stage names in tick order, lower case, as the overlay's row ids use them. */
export const STAGE_NAMES = Object.entries(STAGE)
  .sort((a, b) => a[1] - b[1])
  .map(([name]) => name.toLowerCase());

const DEG = 180 / Math.PI;
/** Metres per second to knots. */
const KNOTS = 3600 / 1852;

/** The ownship's flags in words: `ok`, or what is wrong. */
export function flagsText(flags: number): string {
  const words: string[] = [];
  const names: [number, string][] = [
    [OWNSHIP_FLAG.SPECTATOR, 'spectator'],
    [OWNSHIP_FLAG.CRASHED, 'crashed'],
    [OWNSHIP_FLAG.NUMERICAL_FAULT, 'numerical fault'],
    [OWNSHIP_FLAG.FLAMEOUT, 'flameout'],
    [OWNSHIP_FLAG.STALL_WARN, 'stall'],
  ];
  for (const [bit, word] of names) {
    if ((flags & bit) !== 0) words.push(word);
  }
  return words.length === 0 ? 'ok' : words.join(', ');
}

/**
 * Ids and labels of the ownship rows, in order. Related numbers share a row so the page stays
 * within its DOM budget (AC-53.4); the first number in each row is the one its id names.
 */
const OWNSHIP_ROWS: [string, string][] = [
  ['alt', 'Altitude'],
  ['pitch', 'Pitch/roll'],
  ['heading', 'Heading'],
  ['tas', 'TAS/CAS'],
  ['mach', 'Mach'],
  ['alpha', 'Alpha/beta'],
  ['nz', 'n_z'],
  ['throttle', 'Throttle'],
  ['thrust', 'Thrust'],
  ['fuel', 'Fuel/mass'],
  ['surfaces', 'Surfaces'],
  ['status', 'Status'],
];

function titleCase(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Pitch, roll and heading in degrees, from the body-to-BF attitude quaternion. */
export function attitudeDegrees(quat: SimStats['aircraft']['quat']): {
  pitch: number;
  roll: number;
  heading: number;
} {
  const nose = quatRotate(quat, [1, 0, 0]);
  const wing = quatRotate(quat, [0, 1, 0]);
  const deg = 180 / Math.PI;
  const clamp = (v: number): number => Math.min(1, Math.max(-1, v));
  return {
    pitch: Math.asin(clamp(nose[2])) * deg,
    // Positive when the right wing is down, as the stick's roll axis is.
    roll: Math.asin(clamp(-wing[2])) * deg,
    heading: (Math.atan2(nose[0], nose[1]) * deg + 360) % 360,
  };
}

export function buildOverlayRows(m: OverlayModel): OverlayRow[] {
  const s = m.sim;
  const r = m.render;
  const rows: OverlayRow[] = [];
  const add = (id: string, label: string, value: string): void => {
    rows.push({ id, label, value });
  };

  add('build', 'Build', m.build.slice(0, 12));
  add('abi', 'ABI', String(m.abi));
  add('mode', 'Mode', `${m.variant} · ${m.snapshotChannel} · ${m.sleeper}`);
  add('renderer', 'Renderer', r === undefined ? DASH : `${r.backend} on ${m.renderHost}`);
  add('adapter', 'Adapter', m.adapter === '' ? DASH : m.adapter);
  add('device', 'Device', r === undefined ? DASH : r.description.slice(0, 60));
  add('content', 'Content', m.contentHash);
  add('bench', 'Benchmark', m.benchmark ?? DASH);

  add('tick', 'Tick', s === undefined ? DASH : String(s.tick));
  add('core', 'Core', s === undefined ? DASH : (CORE_STATE[s.coreState] ?? String(s.coreState)));
  add('sim-hz', 'Sim Hz', fixed(s?.hz, 2));
  add(
    'step',
    'Step ms',
    s === undefined
      ? DASH
      : `${fixed(q(s.step, 'avg'), 3)} avg ${fixed(q(s.step, 'p99'), 3)} p99 ${fixed(q(s.step, 'max'), 3)} max`,
  );
  add(
    'dropped',
    'Dropped',
    s === undefined ? DASH : `${fixed(s.droppedMs, 1)} ms in ${String(s.droppedEvents)}`,
  );
  add('latency', 'Input ms', fixed(q(s?.inputLatency, 'p95'), 2, ' p95'));
  add(
    'queues',
    'Queues',
    s === undefined
      ? DASH
      : `inputs dropped ${String(s.inputsDropped)}, snapshots dropped ${String(s.snapshotsDropped)}`,
  );
  add(
    'input',
    'Input sent',
    m.input === undefined
      ? DASH
      : `${String(m.input.sent)} (${String(m.input.refused)} refused)` +
          (s === undefined ? '' : `, core at ${String(s.aircraft.inputSeq)}`),
  );

  add('fps', 'FPS', fixed(r?.fps, 1));
  add('frame', 'Frame ms', fixed(q(r?.frameMs, 'p95'), 2, ' p95'));
  add('render-cpu', 'Render ms', fixed(q(r?.renderCpuMs, 'avg'), 2, ' avg'));
  add('age', 'Snap age', fixed(q(r?.snapshotAgeMs, 'p95'), 1, ' ms p95'));
  add(
    'frames',
    'Frames',
    r === undefined
      ? DASH
      : `${String(r.frames)} (${String(r.staleFrames)} stale, ${String(r.gappedFrames)} gapped)`,
  );

  if (s === undefined) {
    for (const [id, label] of OWNSHIP_ROWS) {
      add(id, label, DASH);
    }
  } else {
    const o = s.aircraft;
    const a = attitudeDegrees(o.quat);
    const label = (id: string): string => OWNSHIP_ROWS.find((r) => r[0] === id)?.[1] ?? id;
    add(
      'alt',
      label('alt'),
      `${fixed(o.altitude, 0, ' m')} at E ${o.pos[0].toFixed(0)} N ${o.pos[1].toFixed(0)}`,
    );
    add('pitch', label('pitch'), `${fixed(a.pitch, 1, '°')} / ${fixed(a.roll, 1, '°')}`);
    // Rounded first, so 359.96 reads as 0.0 rather than 360.0.
    add('heading', label('heading'), fixed((Math.round(a.heading * 10) / 10) % 360, 1, '°'));
    add('tas', label('tas'), `${fixed(o.tas, 1, ' m/s')} / ${fixed(o.cas * KNOTS, 0, ' kn')}`);
    add('mach', label('mach'), fixed(o.mach, 3));
    add(
      'alpha',
      label('alpha'),
      `${fixed(o.alpha * DEG, 1, '°')} / ${fixed(o.beta * DEG, 1, '°')}`,
    );
    add('nz', label('nz'), fixed(o.nz, 2, ' g'));
    add(
      'throttle',
      label('throttle'),
      `${fixed(o.throttle, 2)}${o.abStage > 0 ? ` AB ${String(o.abStage)}` : ''}`,
    );
    add('thrust', label('thrust'), fixed(o.thrust / 1000, 1, ' kN'));
    add('fuel', label('fuel'), `${fixed(o.fuel, 0, ' kg')} / ${fixed(o.mass, 0, ' kg')}`);
    add(
      'surfaces',
      label('surfaces'),
      `e ${fixed(o.surfaces[0] * DEG, 1)} a ${fixed(o.surfaces[1] * DEG, 1)} r ${fixed(o.surfaces[2] * DEG, 1)}°`,
    );
    add('status', label('status'), flagsText(o.flags));
  }

  const profile = s?.profile ?? null;
  STAGE_NAMES.forEach((name, i) => {
    add(`stage-${name}`, titleCase(name), fixed(profile?.stageMsEwma[i], 3, ' ms'));
  });
  return rows;
}

/** Lines for the Hash button: the tick and each column as 16 hex digits. */
export function formatHashReport(report: {
  tick: number;
  hashes: Readonly<Record<string, string>>;
}): string {
  const width = Math.max(...Object.keys(report.hashes).map((k) => k.length), 4);
  return [
    `tick ${String(report.tick)}`,
    ...Object.entries(report.hashes).map(([name, hash]) => `${name.padEnd(width)} ${hash}`),
  ].join('\n');
}
