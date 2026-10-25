/**
 * Control profiles, `controls/1` (GDD 58.5, ADR-37).
 *
 * A profile says which physical input drives which action, and how each analogue axis is shaped.
 * It is plain data: the binding editor edits it, the binding map (`binding-map.ts`) compiles it
 * for the pump, and it travels as JSON (stored in IndexedDB, exported and imported as files).
 *
 * ```json
 * { "schema": "controls/1", "id": "default-km", "name": "Keyboard and mouse",
 *   "deviceClass": "keyboard-mouse",
 *   "bindings": [ { "action": "flight.pitch", "source": { "kind": "key", "code": "KeyW" },
 *                   "direction": -1 } ],
 *   "axes": { "flight.pitch": { "deadCentre": 0.03, ... } }, "calibration": {}, ... }
 * ```
 *
 * `parseControlsProfile` checks everything and reports each problem with a JSON pointer, so an
 * imported file that is wrong says where. `serializeProfile` writes a canonical form (fixed key
 * order, two-space indent), so export, import and export again give the same bytes.
 */
import {
  ACTIONS,
  actionDef,
  type ActionId,
  DEVICE_CLASSES,
  type DeviceClass,
  isActionId,
} from './actions.ts';
import { PAD_AXIS, PAD_BUTTON } from './devices/gamepad.ts';
import { type AxisEmulation, DEFAULT_EMULATION } from './keyboard.ts';
import {
  type AxisShape,
  type Calibration,
  type Curve,
  IDENTITY_SHAPE,
  LINEAR_SHAPE,
  STICK_SHAPE,
} from './shaping.ts';
import { deviceMatches, type HatDirection, type Half, type Source } from './sources.ts';

export const PROFILE_SCHEMA = 'controls/1';
/** The most bindings one action can have (the editor shows this many chips). */
export const MAX_BINDINGS_PER_ACTION = 4;
const MAX_BINDINGS = 256;

export type Direction = 1 | -1;

export interface Binding {
  readonly action: ActionId;
  readonly source: Source;
  /**
   * Axis actions: which way a key, button or hat pushes the axis (default 1). On a pad axis or
   * trigger it multiplies the reading, so the right trigger is +1 and the left is -1 on yaw.
   */
  readonly direction?: Direction;
  /** A pad axis: flip this one physical axis (a throttle that reads -1 at full power). */
  readonly invert?: boolean;
  /** Buttons driven by an analogue source: the reading above which it counts as pressed (0.5). */
  readonly threshold?: number;
}

export interface ThrottleSettings {
  /** How fast a held throttle key moves the lever, fraction of travel per second. */
  keyRatePerSecond: number;
  /** How fast a stick held off centre moves it, at full deflection. */
  stickRatePerSecond: number;
  /** One wheel notch, fraction of travel. */
  wheelStep: number;
}

export type ToggleMode = 'toggle' | 'hold';

export interface ToggleSettings {
  airbrake: ToggleMode;
  afterburner: ToggleMode;
}

export interface ControlsProfile {
  schema: typeof PROFILE_SCHEMA;
  /** Lower-case letters, digits, `-` and `_`. */
  id: string;
  name: string;
  deviceClass: DeviceClass;
  bindings: Binding[];
  /** Shaping per axis or rate action; an action without an entry uses `defaultShape`. */
  axes: Partial<Record<ActionId, AxisShape>>;
  /** Per physical axis, keyed `device:a:index` (`calibrationKey`). */
  calibration: Record<string, Calibration>;
  keyboard: AxisEmulation;
  throttle: ThrottleSettings;
  toggles: ToggleSettings;
}

export const DEFAULT_THROTTLE: ThrottleSettings = {
  keyRatePerSecond: 0.5,
  stickRatePerSecond: 1,
  wheelStep: 0.05,
};
export const DEFAULT_TOGGLES: ToggleSettings = { airbrake: 'toggle', afterburner: 'toggle' };

/**
 * Pitch and roll get the stick curve; the throttle lever and the brakes, which have end stops and
 * no centre, get none at all; everything else is linear with a small dead zone.
 */
export function defaultShape(action: ActionId): AxisShape {
  switch (action) {
    case 'flight.pitch':
    case 'flight.roll':
      return STICK_SHAPE;
    case 'flight.throttle':
    case 'flight.brake_left':
    case 'flight.brake_right':
      return IDENTITY_SHAPE;
    default:
      return LINEAR_SHAPE;
  }
}

/** The shaping a profile gives an action. */
export function shapeFor(p: ControlsProfile, action: ActionId): AxisShape {
  return p.axes[action] ?? defaultShape(action);
}

/** True for actions that have a shape: axes, and the rate actions a stick can drive. */
export function hasShape(action: ActionId): boolean {
  const kind = actionDef(action)?.kind;
  return kind === 'axis' || kind === 'rate';
}

/** The `calibration` key of one physical axis. */
export function calibrationKey(device: string, index: number): string {
  return `${device}:a:${String(index)}`;
}

/** The device names a profile's bindings use, other than the keyboard and the mouse. */
export function profileDevices(p: ControlsProfile): string[] {
  const out = new Set<string>();
  for (const b of p.bindings) {
    const s = b.source;
    if (s.kind === 'pad-button' || s.kind === 'pad-axis' || s.kind === 'hat') out.add(s.device);
  }
  return [...out].sort();
}

/** True if every device the profile binds is among the `connected` device names. */
export function profileSatisfied(p: ControlsProfile, connected: readonly string[]): boolean {
  return profileDevices(p).every((bound) =>
    connected.some((actual) => deviceMatches(bound, actual)),
  );
}

// ---------------------------------------------------------------------------------------------
// Built-in profiles
// ---------------------------------------------------------------------------------------------

export const BUILTIN_IDS = ['default-km', 'gamepad-standard', 'generic-joystick'] as const;
export type BuiltinId = (typeof BUILTIN_IDS)[number];

export function isBuiltinId(id: string): id is BuiltinId {
  return (BUILTIN_IDS as readonly string[]).includes(id);
}

type Extra = { direction?: Direction; invert?: boolean; threshold?: number };
const bind = (action: ActionId, source: Source, extra: Extra = {}): Binding => ({
  action,
  source,
  ...extra,
});
const key = (code: string): Source => ({ kind: 'key', code });
const padButton = (device: string, index: number): Source => ({
  kind: 'pad-button',
  device,
  index,
});
const padAxis = (device: string, index: number, half?: Half): Source =>
  half === undefined
    ? { kind: 'pad-axis', device, index }
    : { kind: 'pad-axis', device, index, half };
const hat = (device: string, dir: HatDirection): Source => ({ kind: 'hat', device, hat: 0, dir });

/**
 * The keyboard and mouse bindings every built-in profile carries. W pushes the stick forward
 * (nose down) and S pulls it back; the throttle also answers to Page Up and Page Down because
 * Ctrl+W, Ctrl+R and Ctrl+S mean something else to the browser (GDD 58.3).
 */
function keyboardBindings(compact = false): Binding[] {
  // A profile holds four bindings an action at most. The pad profiles already spend one of the
  // throttle's on the stick, so they leave out the Page keys (Shift and Ctrl remain).
  const pages = (action: ActionId, code: string): Binding[] =>
    compact ? [] : [bind(action, key(code))];
  return [
    bind('flight.pitch', key('KeyW'), { direction: -1 }),
    bind('flight.pitch', key('KeyS'), { direction: 1 }),
    bind('flight.roll', key('KeyA'), { direction: -1 }),
    bind('flight.roll', key('KeyD'), { direction: 1 }),
    bind('flight.yaw', key('KeyQ'), { direction: -1 }),
    bind('flight.yaw', key('KeyE'), { direction: 1 }),
    bind('flight.throttle_up', key('ShiftLeft')),
    bind('flight.throttle_up', key('ShiftRight')),
    ...pages('flight.throttle_up', 'PageUp'),
    bind('flight.throttle_up', { kind: 'wheel', dir: 'up' }),
    bind('flight.throttle_down', key('ControlLeft')),
    bind('flight.throttle_down', key('ControlRight')),
    ...pages('flight.throttle_down', 'PageDown'),
    bind('flight.throttle_down', { kind: 'wheel', dir: 'down' }),
    bind('flight.afterburner', key('KeyZ')),
    bind('flight.trim_nose_up', key('BracketRight')),
    bind('flight.trim_nose_down', key('BracketLeft')),
    bind('flight.trim_reset', key('Backslash')),
    bind('flight.gear', key('KeyL')),
    bind('flight.flaps_cycle', key('KeyV')),
    bind('flight.airbrake', key('KeyN')),
    bind('flight.brakes', key('KeyB')),
    bind('flight.parking_brake', key('KeyP')),
    bind('flight.recover', key('KeyU')),
    bind('flight.mouse_mode', key('KeyO')),
    bind('flight.assist_cycle', key('F8')),
    bind('system.respawn', key('KeyR')),
    bind('system.pause', key('Escape')),
  ];
}

function gamepadBindings(): Binding[] {
  const d = 'pad';
  return [
    bind('flight.roll', padAxis(d, PAD_AXIS.RIGHT_X)),
    bind('flight.pitch', padAxis(d, PAD_AXIS.RIGHT_Y)),
    bind('flight.yaw', padButton(d, PAD_BUTTON.RIGHT_TRIGGER), { direction: 1 }),
    bind('flight.yaw', padButton(d, PAD_BUTTON.LEFT_TRIGGER), { direction: -1 }),
    bind('flight.throttle_up', padAxis(d, PAD_AXIS.LEFT_Y, 'negative')),
    bind('flight.throttle_down', padAxis(d, PAD_AXIS.LEFT_Y, 'positive')),
    bind('flight.afterburner', padButton(d, PAD_BUTTON.LEFT_STICK)),
    // The d-pad follows the stick: down is nose up.
    bind('flight.trim_nose_down', hat(d, 'up')),
    bind('flight.trim_nose_up', hat(d, 'down')),
    bind('flight.flaps_up', hat(d, 'left')),
    bind('flight.flaps_down', hat(d, 'right')),
    bind('flight.gear', padButton(d, PAD_BUTTON.Y)),
    bind('flight.brakes', padButton(d, PAD_BUTTON.X)),
    bind('flight.airbrake', padButton(d, PAD_BUTTON.B)),
    bind('flight.parking_brake', padButton(d, PAD_BUTTON.A)),
    bind('flight.recover', padButton(d, PAD_BUTTON.LEFT_BUMPER)),
    bind('system.respawn', padButton(d, PAD_BUTTON.BACK)),
    bind('system.pause', padButton(d, PAD_BUTTON.START)),
  ];
}

/**
 * A generic joystick or HOTAS: stick on axes 0 and 1, an absolute throttle lever on axis 2 (a
 * lever pushed forward reads -1), twist or pedals on axis 3 or 5, the hat for trim and flaps.
 * Which axis a given device uses for what varies; the binding editor fixes the rest. Untested on
 * hardware (no devices were available for v0.1b).
 */
function joystickBindings(): Binding[] {
  const d = 'joy';
  return [
    bind('flight.roll', padAxis(d, 0)),
    bind('flight.pitch', padAxis(d, 1)),
    bind('flight.throttle', padAxis(d, 2), { invert: true }),
    bind('flight.yaw', padAxis(d, 3)),
    bind('flight.yaw', padAxis(d, 5)),
    bind('flight.trim_nose_down', hat(d, 'up')),
    bind('flight.trim_nose_up', hat(d, 'down')),
    bind('flight.flaps_up', hat(d, 'left')),
    bind('flight.flaps_down', hat(d, 'right')),
    bind('flight.brakes', padButton(d, 0)),
    bind('flight.airbrake', padButton(d, 1)),
    bind('flight.gear', padButton(d, 2)),
    bind('flight.flaps_cycle', padButton(d, 3)),
  ];
}

function defaultAxes(): Partial<Record<ActionId, AxisShape>> {
  const out: Partial<Record<ActionId, AxisShape>> = {};
  for (const a of ACTIONS) {
    if (hasShape(a.id)) out[a.id] = cloneShape(defaultShape(a.id));
  }
  return out;
}

function cloneShape(s: AxisShape): AxisShape {
  return { ...s, curve: { ...s.curve } };
}

/** A fresh copy of a built-in profile. */
export function builtinProfile(id: BuiltinId): ControlsProfile {
  const common = {
    schema: PROFILE_SCHEMA,
    axes: defaultAxes(),
    calibration: {},
    keyboard: { ...DEFAULT_EMULATION },
    throttle: { ...DEFAULT_THROTTLE },
  } as const;
  switch (id) {
    case 'default-km':
      return {
        ...common,
        id,
        name: 'Keyboard and mouse',
        deviceClass: 'keyboard-mouse',
        bindings: keyboardBindings(),
        toggles: { ...DEFAULT_TOGGLES },
      };
    case 'gamepad-standard':
      return {
        ...common,
        id,
        name: 'Gamepad',
        deviceClass: 'gamepad',
        bindings: [...gamepadBindings(), ...keyboardBindings(true)],
        toggles: { airbrake: 'toggle', afterburner: 'hold' },
      };
    case 'generic-joystick':
      return {
        ...common,
        id,
        name: 'Joystick or HOTAS',
        deviceClass: 'joystick',
        bindings: [...joystickBindings(), ...keyboardBindings(true)],
        toggles: { ...DEFAULT_TOGGLES },
      };
  }
}

/** Fresh copies of every built-in profile. */
export function builtinProfiles(): ControlsProfile[] {
  return BUILTIN_IDS.map(builtinProfile);
}

/**
 * Chooses the profile to fly with: the player's active one if the devices it binds are
 * connected, otherwise the built-in that suits what is connected (a raw joystick, then a
 * standard pad, then the keyboard). Called again whenever a device appears or goes, so a replugged
 * device gets its profile back (AC-58.6).
 */
export function chooseProfile(
  profiles: readonly ControlsProfile[],
  connected: readonly string[],
  activeId?: string,
): ControlsProfile {
  const find = (id: string): ControlsProfile | undefined => profiles.find((p) => p.id === id);
  const active = activeId === undefined ? undefined : find(activeId);
  if (active !== undefined && profileSatisfied(active, connected)) return active;
  const wanted: BuiltinId = connected.some((d) => d === 'joy' || d.startsWith('joy:'))
    ? 'generic-joystick'
    : connected.includes('pad')
      ? 'gamepad-standard'
      : 'default-km';
  return find(wanted) ?? builtinProfile(wanted);
}

// ---------------------------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------------------------

export interface ProfileError {
  /** JSON pointer to the problem; the empty string is the whole document. */
  pointer: string;
  message: string;
}

export type ParseResult =
  { ok: true; profile: ControlsProfile } | { ok: false; errors: ProfileError[] };

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);
const escapePointer = (k: string): string => k.replaceAll('~', '~0').replaceAll('/', '~1');
const DEVICE_PATTERN = /^(pad|joy|joy:[0-9a-f]{4}:[0-9a-f]{4})$/;
const ID_PATTERN = /^[a-z0-9][a-z0-9_-]{0,63}$/;
const KEY_PATTERN = /^[A-Za-z0-9]{1,24}$/;
const CALIBRATION_KEY_PATTERN = /^(pad|joy|joy:[0-9a-f]{4}:[0-9a-f]{4}):a:\d{1,2}$/;
const HAT_DIRECTIONS: readonly string[] = ['up', 'right', 'down', 'left'];

class Reader {
  readonly errors: ProfileError[] = [];

  add(pointer: string, message: string): void {
    this.errors.push({ pointer, message });
  }

  object(v: unknown, ptr: string, allowed: readonly string[]): Record<string, unknown> | undefined {
    if (!isObject(v)) {
      this.add(ptr, 'expected an object');
      return undefined;
    }
    for (const k of Object.keys(v)) {
      if (!allowed.includes(k)) this.add(`${ptr}/${escapePointer(k)}`, `unknown key "${k}"`);
    }
    return v;
  }

  number(v: unknown, ptr: string, min: number, max: number): number | undefined {
    if (typeof v !== 'number' || !Number.isFinite(v)) {
      this.add(ptr, 'expected a number');
      return undefined;
    }
    if (v < min || v > max) {
      this.add(ptr, `must be from ${String(min)} to ${String(max)}, got ${String(v)}`);
      return undefined;
    }
    return v;
  }

  integer(v: unknown, ptr: string, min: number, max: number): number | undefined {
    const n = this.number(v, ptr, min, max);
    if (n !== undefined && !Number.isInteger(n)) {
      this.add(ptr, `expected a whole number, got ${String(n)}`);
      return undefined;
    }
    return n;
  }

  string(v: unknown, ptr: string, pattern: RegExp, what: string): string | undefined {
    if (typeof v !== 'string') {
      this.add(ptr, 'expected a string');
      return undefined;
    }
    if (!pattern.test(v)) {
      this.add(ptr, `expected ${what}, got "${v}"`);
      return undefined;
    }
    return v;
  }

  boolean(v: unknown, ptr: string): boolean | undefined {
    if (typeof v !== 'boolean') {
      this.add(ptr, 'expected true or false');
      return undefined;
    }
    return v;
  }

  oneOf<T extends string>(v: unknown, ptr: string, options: readonly T[]): T | undefined {
    if (typeof v === 'string' && (options as readonly string[]).includes(v)) return v as T;
    this.add(ptr, `expected one of ${options.join(', ')}`);
    return undefined;
  }
}

const SOURCE_KEYS: Record<string, readonly string[]> = {
  key: ['kind', 'code'],
  'mouse-button': ['kind', 'button'],
  wheel: ['kind', 'dir'],
  'pad-button': ['kind', 'device', 'index'],
  'pad-axis': ['kind', 'device', 'index', 'half'],
  hat: ['kind', 'device', 'hat', 'dir'],
};

function parseSource(v: unknown, ptr: string, r: Reader): Source | undefined {
  if (!isObject(v)) {
    r.add(ptr, 'expected an object');
    return undefined;
  }
  const kind = v['kind'];
  if (typeof kind !== 'string' || !(kind in SOURCE_KEYS)) {
    r.add(`${ptr}/kind`, `expected one of ${Object.keys(SOURCE_KEYS).join(', ')}`);
    return undefined;
  }
  const o = r.object(v, ptr, SOURCE_KEYS[kind] ?? []);
  if (o === undefined) return undefined;
  const need = (k: string): boolean => {
    if (k in o) return true;
    r.add(`${ptr}/${k}`, 'missing');
    return false;
  };
  switch (kind) {
    case 'key': {
      if (!need('code')) return undefined;
      const code = r.string(o['code'], `${ptr}/code`, KEY_PATTERN, 'a key code such as KeyW');
      return code === undefined ? undefined : { kind, code };
    }
    case 'mouse-button': {
      if (!need('button')) return undefined;
      const button = r.integer(o['button'], `${ptr}/button`, 0, 4);
      return button === undefined ? undefined : { kind, button };
    }
    case 'wheel': {
      if (!need('dir')) return undefined;
      const dir = r.oneOf(o['dir'], `${ptr}/dir`, ['up', 'down'] as const);
      return dir === undefined ? undefined : { kind, dir };
    }
    case 'pad-button': {
      if (!need('device') || !need('index')) return undefined;
      const device = r.string(
        o['device'],
        `${ptr}/device`,
        DEVICE_PATTERN,
        'pad, joy or joy:vvvv:pppp',
      );
      const index = r.integer(o['index'], `${ptr}/index`, 0, 63);
      return device === undefined || index === undefined ? undefined : { kind, device, index };
    }
    case 'pad-axis': {
      if (!need('device') || !need('index')) return undefined;
      const device = r.string(
        o['device'],
        `${ptr}/device`,
        DEVICE_PATTERN,
        'pad, joy or joy:vvvv:pppp',
      );
      const index = r.integer(o['index'], `${ptr}/index`, 0, 31);
      const half =
        'half' in o
          ? r.oneOf(o['half'], `${ptr}/half`, ['positive', 'negative'] as const)
          : undefined;
      if (device === undefined || index === undefined) return undefined;
      if ('half' in o && half === undefined) return undefined;
      return half === undefined ? { kind, device, index } : { kind, device, index, half };
    }
    default: {
      // hat
      if (!need('device') || !need('hat') || !need('dir')) return undefined;
      const device = r.string(
        o['device'],
        `${ptr}/device`,
        DEVICE_PATTERN,
        'pad, joy or joy:vvvv:pppp',
      );
      const hatIndex = r.integer(o['hat'], `${ptr}/hat`, 0, 3);
      const dir = r.oneOf(o['dir'], `${ptr}/dir`, HAT_DIRECTIONS as readonly HatDirection[]);
      return device === undefined || hatIndex === undefined || dir === undefined
        ? undefined
        : { kind: 'hat', device, hat: hatIndex, dir };
    }
  }
}

function parseBinding(v: unknown, ptr: string, r: Reader): Binding | undefined {
  const o = r.object(v, ptr, ['action', 'source', 'direction', 'invert', 'threshold']);
  if (o === undefined) return undefined;
  let action: ActionId | undefined;
  if (typeof o['action'] !== 'string') {
    r.add(`${ptr}/action`, 'expected an action id');
  } else if (!isActionId(o['action'])) {
    r.add(`${ptr}/action`, `unknown action "${o['action']}"`);
  } else {
    action = o['action'];
  }
  const source = 'source' in o ? parseSource(o['source'], `${ptr}/source`, r) : undefined;
  if (!('source' in o)) r.add(`${ptr}/source`, 'missing');
  let direction: Direction | undefined;
  if ('direction' in o) {
    if (o['direction'] === 1 || o['direction'] === -1) direction = o['direction'];
    else r.add(`${ptr}/direction`, 'expected 1 or -1');
  }
  const invert = 'invert' in o ? r.boolean(o['invert'], `${ptr}/invert`) : undefined;
  const threshold =
    'threshold' in o ? r.number(o['threshold'], `${ptr}/threshold`, 0.05, 0.95) : undefined;
  if (action === undefined || source === undefined) return undefined;
  if (action === 'flight.throttle' && source.kind !== 'pad-axis' && source.kind !== 'pad-button') {
    r.add(`${ptr}/source/kind`, 'the absolute throttle needs a pad axis or button');
    return undefined;
  }
  return {
    action,
    source,
    ...(direction === undefined ? {} : { direction }),
    ...(invert === undefined ? {} : { invert }),
    ...(threshold === undefined ? {} : { threshold }),
  };
}

function parseCurve(v: unknown, ptr: string, r: Reader): Curve | undefined {
  const kind = isObject(v) ? v['kind'] : undefined;
  if (kind === 'expo') {
    const o = r.object(v, ptr, ['kind', 'k']);
    const k = o === undefined ? undefined : r.number(o['k'], `${ptr}/k`, 0, 1);
    return k === undefined ? undefined : { kind, k };
  }
  if (kind === 'power') {
    const o = r.object(v, ptr, ['kind', 'gamma']);
    const gamma = o === undefined ? undefined : r.number(o['gamma'], `${ptr}/gamma`, 0.2, 5);
    return gamma === undefined ? undefined : { kind, gamma };
  }
  r.add(
    isObject(v) ? `${ptr}/kind` : ptr,
    'expected a curve: { "kind": "expo", "k" } or { "kind": "power", "gamma" }',
  );
  return undefined;
}

function parseShape(v: unknown, ptr: string, r: Reader, base: AxisShape): AxisShape | undefined {
  const o = r.object(v, ptr, [
    'deadCentre',
    'deadOuter',
    'curve',
    'gain',
    'tauMs',
    'maxRatePerSecond',
    'invert',
    'offset',
  ]);
  if (o === undefined) return undefined;
  const s = cloneShape(base);
  const before = r.errors.length;
  if ('deadCentre' in o)
    s.deadCentre = r.number(o['deadCentre'], `${ptr}/deadCentre`, 0, 0.5) ?? s.deadCentre;
  if ('deadOuter' in o)
    s.deadOuter = r.number(o['deadOuter'], `${ptr}/deadOuter`, 0, 0.5) ?? s.deadOuter;
  if ('curve' in o) s.curve = parseCurve(o['curve'], `${ptr}/curve`, r) ?? s.curve;
  if ('gain' in o) s.gain = r.number(o['gain'], `${ptr}/gain`, 0.5, 2) ?? s.gain;
  if ('tauMs' in o) s.tauMs = r.number(o['tauMs'], `${ptr}/tauMs`, 0, 120) ?? s.tauMs;
  if ('maxRatePerSecond' in o) {
    s.maxRatePerSecond =
      o['maxRatePerSecond'] === null
        ? Infinity
        : (r.number(o['maxRatePerSecond'], `${ptr}/maxRatePerSecond`, 0.1, 100) ??
          s.maxRatePerSecond);
  }
  if ('invert' in o) s.invert = r.boolean(o['invert'], `${ptr}/invert`) ?? s.invert;
  if ('offset' in o) s.offset = r.number(o['offset'], `${ptr}/offset`, -1, 1) ?? s.offset;
  if (s.deadCentre + s.deadOuter >= 1) {
    r.add(ptr, 'deadCentre and deadOuter must leave some travel: their sum must be below 1');
  }
  return r.errors.length > before ? undefined : s;
}

function parseCalibration(v: unknown, ptr: string, r: Reader): Calibration | undefined {
  const o = r.object(v, ptr, ['min', 'centre', 'max']);
  if (o === undefined) return undefined;
  const min = r.number(o['min'], `${ptr}/min`, -2, 2);
  const centre = r.number(o['centre'], `${ptr}/centre`, -2, 2);
  const max = r.number(o['max'], `${ptr}/max`, -2, 2);
  if (min === undefined || centre === undefined || max === undefined) return undefined;
  if (!(min <= centre && centre <= max && max - min >= 0.2)) {
    r.add(ptr, 'needs min <= centre <= max with a span of at least 0.2');
    return undefined;
  }
  return { min, centre, max };
}

/** Checks a decoded JSON document and builds the profile, or lists every problem found. */
export function parseControlsProfile(json: unknown): ParseResult {
  const r = new Reader();
  const root = r.object(json, '', [
    'schema',
    'id',
    'name',
    'deviceClass',
    'bindings',
    'axes',
    'calibration',
    'keyboard',
    'throttle',
    'toggles',
  ]);
  if (root === undefined) return { ok: false, errors: r.errors };

  if (root['schema'] !== PROFILE_SCHEMA) {
    r.add('/schema', `expected "${PROFILE_SCHEMA}", got ${JSON.stringify(root['schema'] ?? null)}`);
  }
  const id =
    'id' in root
      ? r.string(root['id'], '/id', ID_PATTERN, 'lower-case letters, digits, - and _')
      : undefined;
  if (!('id' in root)) r.add('/id', 'missing');
  let name: string | undefined;
  if (typeof root['name'] !== 'string') {
    r.add('/name', 'expected a string');
  } else if (root['name'].trim().length === 0 || root['name'].length > 64) {
    r.add('/name', 'expected 1 to 64 characters');
  } else {
    name = root['name'];
  }
  const deviceClass =
    'deviceClass' in root
      ? r.oneOf(root['deviceClass'], '/deviceClass', DEVICE_CLASSES)
      : undefined;
  if (!('deviceClass' in root)) r.add('/deviceClass', 'missing');

  const bindings: Binding[] = [];
  const rawBindings = root['bindings'];
  if (!Array.isArray(rawBindings)) {
    r.add('/bindings', 'expected an array');
  } else {
    if (rawBindings.length > MAX_BINDINGS) {
      r.add('/bindings', `at most ${String(MAX_BINDINGS)} bindings`);
    }
    const perAction = new Map<string, number>();
    rawBindings.forEach((raw: unknown, i) => {
      const b = parseBinding(raw, `/bindings/${String(i)}`, r);
      if (b === undefined) return;
      const n = (perAction.get(b.action) ?? 0) + 1;
      perAction.set(b.action, n);
      if (n === MAX_BINDINGS_PER_ACTION + 1) {
        r.add(
          `/bindings/${String(i)}/action`,
          `"${b.action}" has more than ${String(MAX_BINDINGS_PER_ACTION)} bindings`,
        );
      }
      bindings.push(b);
    });
  }

  const axes: Partial<Record<ActionId, AxisShape>> = {};
  if ('axes' in root) {
    const o = r.object(
      root['axes'],
      '/axes',
      ACTIONS.map((a) => a.id),
    );
    if (o !== undefined) {
      for (const [k, v] of Object.entries(o)) {
        if (!isActionId(k)) continue; // already reported as an unknown key
        if (!hasShape(k)) {
          r.add(`/axes/${escapePointer(k)}`, `"${k}" is not an axis and has no shape`);
          continue;
        }
        const shape = parseShape(v, `/axes/${escapePointer(k)}`, r, defaultShape(k));
        if (shape !== undefined) axes[k] = shape;
      }
    }
  }

  const calibration: Record<string, Calibration> = {};
  if ('calibration' in root) {
    const o = r.object(
      root['calibration'],
      '/calibration',
      Object.keys(isObject(root['calibration']) ? root['calibration'] : {}),
    );
    if (o !== undefined) {
      for (const [k, v] of Object.entries(o)) {
        const ptr = `/calibration/${escapePointer(k)}`;
        if (!CALIBRATION_KEY_PATTERN.test(k)) {
          r.add(ptr, 'expected a key like pad:a:0 or joy:046d:c215:a:2');
          continue;
        }
        const c = parseCalibration(v, ptr, r);
        if (c !== undefined) calibration[k] = c;
      }
    }
  }

  const keyboard = { ...DEFAULT_EMULATION };
  if ('keyboard' in root) {
    const o = r.object(root['keyboard'], '/keyboard', ['rampUpPerSecond', 'returnPerSecond']);
    if (o !== undefined) {
      if ('rampUpPerSecond' in o)
        keyboard.rampUpPerSecond =
          r.number(o['rampUpPerSecond'], '/keyboard/rampUpPerSecond', 0.5, 20) ??
          keyboard.rampUpPerSecond;
      if ('returnPerSecond' in o)
        keyboard.returnPerSecond =
          r.number(o['returnPerSecond'], '/keyboard/returnPerSecond', 0.5, 20) ??
          keyboard.returnPerSecond;
    }
  }

  const throttle = { ...DEFAULT_THROTTLE };
  if ('throttle' in root) {
    const o = r.object(root['throttle'], '/throttle', [
      'keyRatePerSecond',
      'stickRatePerSecond',
      'wheelStep',
    ]);
    if (o !== undefined) {
      if ('keyRatePerSecond' in o)
        throttle.keyRatePerSecond =
          r.number(o['keyRatePerSecond'], '/throttle/keyRatePerSecond', 0.05, 5) ??
          throttle.keyRatePerSecond;
      if ('stickRatePerSecond' in o)
        throttle.stickRatePerSecond =
          r.number(o['stickRatePerSecond'], '/throttle/stickRatePerSecond', 0.05, 5) ??
          throttle.stickRatePerSecond;
      if ('wheelStep' in o)
        throttle.wheelStep =
          r.number(o['wheelStep'], '/throttle/wheelStep', 0.005, 0.25) ?? throttle.wheelStep;
    }
  }

  const toggles = { ...DEFAULT_TOGGLES };
  if ('toggles' in root) {
    const o = r.object(root['toggles'], '/toggles', ['airbrake', 'afterburner']);
    if (o !== undefined) {
      if ('airbrake' in o)
        toggles.airbrake =
          r.oneOf(o['airbrake'], '/toggles/airbrake', ['toggle', 'hold'] as const) ??
          toggles.airbrake;
      if ('afterburner' in o)
        toggles.afterburner =
          r.oneOf(o['afterburner'], '/toggles/afterburner', ['toggle', 'hold'] as const) ??
          toggles.afterburner;
    }
  }

  if (r.errors.length > 0 || id === undefined || name === undefined || deviceClass === undefined) {
    return { ok: false, errors: r.errors };
  }
  return {
    ok: true,
    profile: {
      schema: PROFILE_SCHEMA,
      id,
      name,
      deviceClass,
      bindings,
      axes,
      calibration,
      keyboard,
      throttle,
      toggles,
    },
  };
}

/** Parses JSON text; a syntax error is reported against the whole document. */
export function parseControlsProfileText(text: string): ParseResult {
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch (e) {
    return {
      ok: false,
      errors: [{ pointer: '', message: `not valid JSON: ${(e as Error).message}` }],
    };
  }
  return parseControlsProfile(json);
}

function sourceJson(s: Source): Record<string, unknown> {
  switch (s.kind) {
    case 'key':
      return { kind: s.kind, code: s.code };
    case 'mouse-button':
      return { kind: s.kind, button: s.button };
    case 'wheel':
      return { kind: s.kind, dir: s.dir };
    case 'pad-button':
      return { kind: s.kind, device: s.device, index: s.index };
    case 'pad-axis':
      return s.half === undefined
        ? { kind: s.kind, device: s.device, index: s.index }
        : { kind: s.kind, device: s.device, index: s.index, half: s.half };
    case 'hat':
      return { kind: s.kind, device: s.device, hat: s.hat, dir: s.dir };
  }
}

function shapeJson(s: AxisShape): Record<string, unknown> {
  return {
    deadCentre: s.deadCentre,
    deadOuter: s.deadOuter,
    curve:
      s.curve.kind === 'expo'
        ? { kind: 'expo', k: s.curve.k }
        : { kind: 'power', gamma: s.curve.gamma },
    gain: s.gain,
    tauMs: s.tauMs,
    maxRatePerSecond: Number.isFinite(s.maxRatePerSecond) ? s.maxRatePerSecond : null,
    invert: s.invert,
    offset: s.offset,
  };
}

/** The profile as a JSON-ready value with its keys in canonical order. */
export function profileToJson(p: ControlsProfile): Record<string, unknown> {
  const axes: Record<string, unknown> = {};
  for (const a of ACTIONS) {
    const shape = p.axes[a.id];
    if (shape !== undefined) axes[a.id] = shapeJson(shape);
  }
  const calibration: Record<string, unknown> = {};
  for (const k of Object.keys(p.calibration).sort()) {
    const c = p.calibration[k];
    if (c !== undefined) calibration[k] = { min: c.min, centre: c.centre, max: c.max };
  }
  return {
    schema: p.schema,
    id: p.id,
    name: p.name,
    deviceClass: p.deviceClass,
    bindings: p.bindings.map((b) => ({
      action: b.action,
      source: sourceJson(b.source),
      ...(b.direction === undefined ? {} : { direction: b.direction }),
      ...(b.invert === undefined ? {} : { invert: b.invert }),
      ...(b.threshold === undefined ? {} : { threshold: b.threshold }),
    })),
    axes,
    calibration,
    keyboard: {
      rampUpPerSecond: p.keyboard.rampUpPerSecond,
      returnPerSecond: p.keyboard.returnPerSecond,
    },
    throttle: {
      keyRatePerSecond: p.throttle.keyRatePerSecond,
      stickRatePerSecond: p.throttle.stickRatePerSecond,
      wheelStep: p.throttle.wheelStep,
    },
    toggles: { airbrake: p.toggles.airbrake, afterburner: p.toggles.afterburner },
  };
}

/** Canonical JSON text: export, import and export again give identical bytes. */
export function serializeProfile(p: ControlsProfile): string {
  return `${JSON.stringify(profileToJson(p), null, 2)}\n`;
}
