/**
 * What the debug overlay says, as data: `buildOverlayRows` turns the app's current numbers into
 * labelled rows, and nothing here touches the DOM, so the formatting is easy to test.
 */
import type { RenderStats } from '@flying/render/messages';
import {
  type AircraftStatus,
  OWNSHIP_FLAG,
  OWNSHIP_FLAG2,
  quatRotate,
  STAGE,
  type Quantiles,
  type SimStats,
} from '@flying/sim-bridge';

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
  /** The ownship's flight control, wheel and g state from the newest snapshot, if read yet. */
  flight: AircraftStatus | undefined;
  /** Control latency, split by where the time goes (ADR-33), or undefined before it is measured. */
  latency: ControlLatency | undefined;
}

/**
 * From the source event of a control change to the end of the tick that acted on it (`control`),
 * split into the wait for the next poll of the device (`poll`) and the wait for the tick (`tick`).
 */
export interface ControlLatency {
  control: Quantiles;
  poll: Quantiles;
  tick: Quantiles;
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
export function flagsText(flags: number, flags2 = 0): string {
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
  const names2: [number, string][] = [
    [OWNSHIP_FLAG2.ALPHA_LIMIT, 'alpha limit'],
    [OWNSHIP_FLAG2.G_LIMIT, 'g limit'],
    [OWNSHIP_FLAG2.OVERSPEED, 'overspeed'],
    [OWNSHIP_FLAG2.OVERSTRESS, 'overstress'],
    [OWNSHIP_FLAG2.BLACKOUT, 'blackout'],
    [OWNSHIP_FLAG2.GLOC, 'g-loc'],
    [OWNSHIP_FLAG2.REDOUT, 'redout'],
    [OWNSHIP_FLAG2.HARD_LANDING, 'hard landing'],
    [OWNSHIP_FLAG2.GEAR_OVERSPEED, 'gear overspeed'],
    [OWNSHIP_FLAG2.FLAP_OVERSPEED, 'flap overspeed'],
    [OWNSHIP_FLAG2.DEPARTED, 'departed'],
    [OWNSHIP_FLAG2.GEAR_WARNING, 'gear warning'],
  ];
  for (const [bit, word] of names2) {
    if ((flags2 & bit) !== 0) words.push(word);
  }
  return words.length === 0 ? 'ok' : words.join(', ');
}

const ASSIST_NAMES = ['none', 'Full', 'Standard', 'Reduced', 'Manual'];
const FCS_MODES = ['flight', 'ground', 'blend', 'recover', 'gcas', 'g-loc'];
const FLAP_NAMES = ['up', 'TO', 'LDG'];

/** The assist level, what the control system is doing, and the load factor it is commanding. */
function assistText(f: AircraftStatus): string {
  const assist = ASSIST_NAMES[f.assist] ?? String(f.assist);
  const mode = FCS_MODES[f.fcsMode] ?? String(f.fcsMode);
  return `${assist} · ${mode} · n cmd ${f.nCmd.toFixed(2)} g`;
}

/** Ground speed first, then the gear, flaps, airbrake and brakes, and the height or the wheels. */
function groundText(f: AircraftStatus): string {
  const wheels = (f.flags2 & OWNSHIP_FLAG2.WOW) !== 0;
  const parts = [
    `${(f.groundSpeed * KNOTS).toFixed(0)} kn`,
    `gear ${(f.gearPos * 100).toFixed(0)}%`,
    `flaps ${FLAP_NAMES[f.flapDetent] ?? String(f.flapDetent)}`,
    `airbrake ${(f.airbrake * 100).toFixed(0)}%`,
    `brakes ${(f.brakeL * 100).toFixed(0)}/${(f.brakeR * 100).toFixed(0)}%`,
    wheels ? 'on wheels' : `AGL ${f.agl.toFixed(0)} m`,
  ];
  if ((f.flags2 & OWNSHIP_FLAG2.PARKING_BRAKE) !== 0) parts.push('parking');
  return parts.join(' · ');
}

/**
 * Ids and labels of the ownship rows, in order. Related numbers share a row so the page stays
 * within its DOM budget (AC-53.4); the first number in each row is the one its id names.
 */
const OWNSHIP_ROWS: [string, string][] = [
  ['alt', 'Altitude'],
  ['pitch', 'Pitch/roll'],
  ['heading', 'Heading'],
  ['tas', 'TAS/CAS/Mach'],
  ['alpha', 'Alpha/beta'],
  ['nz', 'n_z'],
  ['throttle', 'Throttle/thrust'],
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
  const f = m.flight;
  const rows: OverlayRow[] = [];
  const add = (id: string, label: string, value: string): void => {
    rows.push({ id, label, value });
  };

  add('build', 'Build', `${m.build.slice(0, 12)} · ABI ${String(m.abi)}`);
  add('mode', 'Mode', `${m.variant} · ${m.snapshotChannel} · ${m.sleeper}`);
  add('renderer', 'Renderer', r === undefined ? DASH : `${r.backend} on ${m.renderHost}`);
  const gpu = [m.adapter, r === undefined ? '' : r.description.slice(0, 60)].filter(
    (x) => x !== '',
  );
  add('adapter', 'Adapter', gpu.length === 0 ? DASH : gpu.join(' · '));
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
    s === undefined
      ? DASH
      : `${fixed(s.droppedMs, 1)} ms in ${String(s.droppedEvents)} · inputs dropped ${String(s.inputsDropped)}, snapshots dropped ${String(s.snapshotsDropped)}`,
  );
  // Control latency (ADR-33): the source event to the end of the tick that acted on it, then the
  // wait for the poll, the wait for the tick, and the older "input age" (sampled to published).
  add(
    'ctl',
    'Control ms',
    `${fixed(q(m.latency?.control, 'p95'), 2, ' p95')} · poll ${fixed(q(m.latency?.poll, 'p95'), 1)} · tick ${fixed(q(m.latency?.tick, 'p95'), 1)} · age ${fixed(q(s?.inputLatency, 'p95'), 1)}`,
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
  add(
    'frame',
    'Frame ms',
    `${fixed(q(r?.frameMs, 'p95'), 2, ' p95')} · render ${fixed(q(r?.renderCpuMs, 'avg'), 2, ' avg')}`,
  );
  add(
    'age',
    'Snap age',
    r === undefined
      ? DASH
      : `${fixed(q(r.snapshotAgeMs, 'p95'), 1, ' ms p95')} · ${String(r.frames)} frames (${String(r.staleFrames)} stale, ${String(r.gappedFrames)} gapped)`,
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
    add(
      'tas',
      label('tas'),
      `${fixed(o.tas, 1, ' m/s')} / ${fixed(o.cas * KNOTS, 0, ' kn')} · M ${fixed(o.mach, 3)}`,
    );
    add(
      'alpha',
      label('alpha'),
      `${fixed(o.alpha * DEG, 1, '°')} / ${fixed(o.beta * DEG, 1, '°')}`,
    );
    add('nz', label('nz'), fixed(o.nz, 2, ' g'));
    add(
      'throttle',
      label('throttle'),
      `${fixed(o.throttle, 2)}${o.abStage > 0 ? ` AB ${String(o.abStage)}` : ''} · ${fixed(o.thrust / 1000, 1, ' kN')}`,
    );
    add('fuel', label('fuel'), `${fixed(o.fuel, 0, ' kg')} / ${fixed(o.mass, 0, ' kg')}`);
    add(
      'surfaces',
      label('surfaces'),
      `e ${fixed(o.surfaces[0] * DEG, 1)} a ${fixed(o.surfaces[1] * DEG, 1)} r ${fixed(o.surfaces[2] * DEG, 1)}°`,
    );
    add('status', label('status'), flagsText(o.flags, f?.flags2 ?? 0));
  }

  add('assist', 'Assist', f === undefined ? DASH : assistText(f));
  add('ground', 'Ground', f === undefined ? DASH : groundText(f));
  add(
    'g',
    'G state',
    f === undefined
      ? DASH
      : `${fixed(f.gLagged, 2, ' g')} · S ${fixed(f.gStress, 2)} · R ${fixed(f.redout, 2)}`,
  );

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
