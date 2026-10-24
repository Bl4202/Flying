/**
 * The input pump: reads the devices through the active profile and writes `InputFrame` v3 to the
 * input channel (GDD 58.2, ADR-37).
 *
 * `update()` is called from key events, every animation frame, and a 4 ms timer while the tab has
 * focus, so a change reaches the channel within a few milliseconds whatever the display rate. It
 * writes a frame when something changed, and at least every `minIntervalMs` otherwise (8 ms
 * through the shared ring, 33 ms when frames go by `postMessage`), so the Sim Worker always has a
 * recent sample.
 *
 * Latency is measured from the frame (AC-58.3, ADR-33), so the frame says when things happened:
 *
 * - `t_ms` is the time of the earliest source event not yet sent (a key press, a mouse move, a pad
 *   sample), on the shared clock. Devices tell the pump with `poke(eventTime)`; the next update
 *   uses it.
 * - `t_sent_ms` is when the frame was written.
 * - The CHANGED flag marks a frame that carries a new pilot input: an axis moved by more than
 *   1e-5, a button or a flag changed. Heartbeats are not CHANGED, so they are not measured.
 *
 * Besides the sticks the frame carries the player's standing requests: the assist level, the
 * spawn for the next respawn, and the acknowledgement of the spawn the throttle lever was
 * synchronised to (`observe`).
 *
 * `update()` makes no objects, arrays or closures: the bindings are flat arrays
 * (`binding-map.ts`), the frame is written into one buffer (`InputFrameWriter`), and the numbers
 * that change live in typed arrays. V8 can still box a fractional number that crosses a call it
 * does not inline; `pump.garbage.test.ts` measures how much (nothing at rest).
 *
 * When focus is lost or the tab is hidden, `neutral()` sends one frame with the NEUTRAL flag: the
 * core zeroes the sticks and holds the throttle where it was.
 */
import { AXIS, BUTTON, INPUT_FLAG, InputFrameWriter, type InputSink } from '@flying/sim-bridge';
import { ACTION_COUNT, ACTIONS, actionIndex, type ActionId } from './actions.ts';
import { type BindingMap, compileBindings, type Devices } from './binding-map.ts';
import { type GamepadHub } from './devices/gamepad.ts';
import { type MouseSource } from './devices/mouse.ts';
import { AxisEmulator, type KeyboardState } from './keyboard.ts';
import { MouseAim } from './mouse-aim.ts';
import { MouseStick } from './mouse-stick.ts';
import { builtinProfile, type ControlsProfile, shapeFor } from './profile.ts';
import { type AxisShape, AxisShaper, shapeInto } from './shaping.ts';
import { ThrottleController } from './throttle.ts';

export type MouseMode = 'off' | 'aim' | 'stick';
export type PumpContext = 'flight' | 'menu';

export interface PumpDeps {
  sink: InputSink;
  keyboard: KeyboardState;
  /** Gamepads and joysticks; without it only the keyboard and mouse are read. */
  pads?: GamepadHub;
  mouse?: MouseSource;
  /** The profile to start with (default: the built-in keyboard and mouse one). */
  profile?: ControlsProfile;
  /** Monotonic milliseconds: time steps, and the clock `poke` times are on. */
  nowMs: () => number;
  /** Milliseconds on the shared clock: the timestamps written into each frame. */
  sharedNowMs: () => number;
  /** Longest gap between frames when nothing changes. */
  minIntervalMs?: number;
  /** Called when an action only the page can carry out is pressed (pause, respawn, ...). */
  onAction?: (id: ActionId) => void;
}

/** What the pump needs to know about the simulation, from the snapshot. */
export interface PumpObservation {
  /** The aircraft's `spawn_seq`: changes at every spawn. */
  spawnSeq: number;
  /** The aircraft's throttle, 0 to 1. */
  throttle: number;
  /** The aircraft's assist level, 1 to 4 (0 if unknown). */
  assist: number;
  /** Where the nose points: azimuth clockwise from north, and elevation up, radians. */
  noseAzRad: number;
  noseElRad: number;
}

export interface PumpStats {
  /** Frames handed to the channel. */
  sent: number;
  /** Frames the channel refused (its queue was full). */
  refused: number;
  /** Frames sent only because the heartbeat interval passed. */
  heartbeats: number;
  /** Frames flagged CHANGED. */
  changed: number;
  neutralFrames: number;
}

/** Steps longer than this (a stalled tab) are treated as this long, so nothing jumps. */
const MAX_STEP_MS = 100;
/** Axis changes smaller than this do not count as a change. */
const EPSILON = 1e-5;
/** A source event older than this is not believed (a clock from another origin). */
const MAX_EVENT_AGE_MS = 500;
/** A pad's own timestamp is believed if it is this recent. */
const MAX_PAD_AGE_MS = 100;
/** A pad's reading must move this much to take the throttle lever from the keys. */
const LEVER_MOVED = 0.004;
const AXES = 12;

/**
 * The pump's fractional state lives in one Float64Array, not in fields: V8 allocates for every
 * store of a fractional number into an object field, and `update()` runs every few milliseconds.
 */
const R_LAST_UPDATE = 0;
const R_LAST_SENT = 1;
const R_PENDING = 2;
const R_ABS_PREV = 3;
const R_NOSE_AZ = 4;
const R_NOSE_EL = 5;
const R_MOUSE_SENS = 6;
const R_MOUSE_SPRING = 7;
const R_TMP = 8;
const R_RATE_UP = 9;
const R_RATE_DOWN = 10;
const REGISTERS = 11;

/** What `#emit` did. */
const SKIPPED = 0;
const SENT = 1;
const REFUSED = 2;

const A = {
  pitch: actionIndex('flight.pitch'),
  roll: actionIndex('flight.roll'),
  yaw: actionIndex('flight.yaw'),
  throttle: actionIndex('flight.throttle'),
  throttleUp: actionIndex('flight.throttle_up'),
  throttleDown: actionIndex('flight.throttle_down'),
  afterburner: actionIndex('flight.afterburner'),
  airbrake: actionIndex('flight.airbrake'),
  brakes: actionIndex('flight.brakes'),
  brakeLeft: actionIndex('flight.brake_left'),
  brakeRight: actionIndex('flight.brake_right'),
  mouseMode: actionIndex('flight.mouse_mode'),
  assistCycle: actionIndex('flight.assist_cycle'),
  respawn: actionIndex('system.respawn'),
  pause: actionIndex('system.pause'),
} as const;

/** Actions that set a button bit while active, and the bit each sets. */
const BUTTON_ACTION_IDS: readonly ActionId[] = [
  'system.respawn',
  'flight.trim_nose_up',
  'flight.trim_nose_down',
  'flight.trim_reset',
  'flight.gear',
  'flight.flaps_down',
  'flight.flaps_up',
  'flight.flaps_cycle',
  'flight.parking_brake',
  'flight.recover',
];
const BUTTON_ACTION_INDEX = Int32Array.from(BUTTON_ACTION_IDS, (id) => actionIndex(id));
/** 1 for the actions that act on a press (`button`), 0 for holds and the rest. */
const PRESS_ACTION = Uint8Array.from(ACTIONS, (a) => (a.kind === 'button' ? 1 : 0));
const BUTTON_BITS = Int32Array.of(
  BUTTON.RESET,
  BUTTON.TRIM_NOSE_UP,
  BUTTON.TRIM_NOSE_DOWN,
  BUTTON.TRIM_RESET,
  BUTTON.GEAR,
  BUTTON.FLAPS_DOWN,
  BUTTON.FLAPS_UP,
  BUTTON.FLAPS_CYCLE,
  BUTTON.PARKING_BRAKE,
  BUTTON.RECOVER,
);

/** Actions the pump or the page carries out on a press, reported through `onAction`. */
const PAGE_ACTION_IDS: readonly ActionId[] = [
  'flight.mouse_mode',
  'flight.assist_cycle',
  'system.respawn',
];
const PAGE_ACTION_INDEX = Int32Array.from(PAGE_ACTION_IDS, (id) => actionIndex(id));

const clamp1 = (x: number): number => Math.min(Math.max(x, -1), 1);
const clamp01 = (x: number): number => Math.min(Math.max(x, 0), 1);
/** Whichever of two sources is pushed harder wins (the keyboard and a stick can both be in use). */
const larger = (a: number, b: number): number => (Math.abs(a) >= Math.abs(b) ? a : b);

export class InputPump {
  readonly stats: PumpStats = { sent: 0, refused: 0, heartbeats: 0, changed: 0, neutralFrames: 0 };
  readonly #d: PumpDeps;
  readonly #dev: Devices;
  readonly #minInterval: number;
  readonly #writer = new InputFrameWriter();
  readonly #lever = new ThrottleController(0.5);
  readonly #aim = new MouseAim();
  readonly #stick = new MouseStick();
  /** The values of the update in progress, then of the last frame sent. */
  readonly #cur = new Float64Array(AXES);
  readonly #last = new Float64Array(AXES);
  readonly #r = new Float64Array(REGISTERS);
  readonly #prevActive = new Uint8Array(ACTION_COUNT);
  /** Press-type buttons that were already down at a resync: ignored until let go. */
  readonly #suppress = new Uint8Array(ACTION_COUNT);
  #buttons = 0;
  #lastButtons = 0;
  #lastFlags = 0;
  #lastAssist = -1;
  #lastSpawnAck = -1;
  #lastSpawnReq = -1;
  #primed = false;

  #profile: ControlsProfile;
  #map: BindingMap;
  #emuPitch = new AxisEmulator();
  #emuRoll = new AxisEmulator();
  #emuYaw = new AxisEmulator();
  #shPitch = new AxisShaper();
  #shRoll = new AxisShaper();
  #shYaw = new AxisShaper();
  #shThrottle: AxisShape;
  #shBrakeL: AxisShape;
  #shBrakeR: AxisShape;
  #shUp: AxisShape;
  #shDown: AxisShape;
  #padRevision = -1;

  #context: PumpContext = 'flight';
  #resync = false;
  #airbrakeOpen = false;
  #afterburnerHeld = false;
  #absSync = true;

  #mouseMode: MouseMode = 'off';
  #assistReq = 0;
  #assistSeen = 0;
  #spawnAck = 0;
  #spawnReq = 0;
  #autoGcas = false;

  #seq = 0;

  constructor(deps: PumpDeps) {
    this.#d = deps;
    this.#r[R_LAST_UPDATE] = Number.NaN;
    this.#r[R_LAST_SENT] = Number.NEGATIVE_INFINITY;
    this.#r[R_PENDING] = Number.POSITIVE_INFINITY;
    this.#r[R_ABS_PREV] = Number.NaN;
    this.#r[R_MOUSE_SENS] = 1;
    this.#dev = { keyboard: deps.keyboard, mouse: deps.mouse, pads: deps.pads };
    this.#minInterval = deps.minIntervalMs ?? 8;
    this.#profile = deps.profile ?? builtinProfile('default-km');
    this.#map = compileBindings(this.#profile);
    this.#shThrottle = shapeFor(this.#profile, 'flight.throttle');
    this.#shBrakeL = shapeFor(this.#profile, 'flight.brake_left');
    this.#shBrakeR = shapeFor(this.#profile, 'flight.brake_right');
    this.#shUp = shapeFor(this.#profile, 'flight.throttle_up');
    this.#shDown = shapeFor(this.#profile, 'flight.throttle_down');
    this.#applyProfile();
  }

  // -- state the page sets and reads ---------------------------------------------------------

  /** The virtual throttle lever, 0 to 1. */
  get throttle(): number {
    return this.#lever.value;
  }

  get profile(): ControlsProfile {
    return this.#profile;
  }

  /** The compiled bindings: `keyCodes` is what the page must stop the browser acting on. */
  get bindings(): BindingMap {
    return this.#map;
  }

  get context(): PumpContext {
    return this.#context;
  }

  get mouseMode(): MouseMode {
    return this.#mouseMode;
  }

  /** The aim direction (radians), for drawing the aim marker. */
  get aim(): Readonly<MouseAim> {
    return this.#aim;
  }

  /** The assist level last requested (0 = none yet). */
  get assistRequest(): number {
    return this.#assistReq;
  }

  /**
   * Switches to another profile. The throttle lever, toggles and requests carry over. The pump
   * keeps the profile it is given and reads its shapes; to change a profile, pass a new object.
   */
  setProfile(profile: ControlsProfile): void {
    this.#profile = profile;
    this.#map = compileBindings(profile);
    this.#shThrottle = shapeFor(profile, 'flight.throttle');
    this.#shBrakeL = shapeFor(profile, 'flight.brake_left');
    this.#shBrakeR = shapeFor(profile, 'flight.brake_right');
    this.#shUp = shapeFor(profile, 'flight.throttle_up');
    this.#shDown = shapeFor(profile, 'flight.throttle_down');
    this.#applyProfile();
    this.#padRevision = -1; // resolve the new bindings on the next update
    this.#resync = true;
    this.#absSync = true;
  }

  /** `menu` silences every flight action (sticks centre, the throttle holds); pause still works. */
  setContext(context: PumpContext): void {
    if (context === this.#context) return;
    this.#context = context;
    this.#resync = true;
    if (context === 'menu') {
      this.#resetShaping();
      this.#aim.disable();
    } else if (this.#mouseMode === 'aim') {
      this.#aim.enable(this.#r[R_NOSE_AZ] ?? 0, this.#r[R_NOSE_EL] ?? 0);
    }
  }

  setMouseMode(mode: MouseMode): void {
    if (mode === this.#mouseMode) return;
    this.#mouseMode = mode;
    this.#stick.reset();
    if (mode === 'aim') this.#aim.enable(this.#r[R_NOSE_AZ] ?? 0, this.#r[R_NOSE_EL] ?? 0);
    else this.#aim.disable();
  }

  /** Mouse-aim, then mouse-stick, then off. */
  cycleMouseMode(): MouseMode {
    this.setMouseMode(
      this.#mouseMode === 'off' ? 'aim' : this.#mouseMode === 'aim' ? 'stick' : 'off',
    );
    return this.#mouseMode;
  }

  /** Mouse sensitivity, 0 to 3 times, and how hard the aim or stick returns to centre, 0 to 1. */
  setMouseTuning(sensitivity: number, spring: number): void {
    this.#r[R_MOUSE_SENS] = Math.min(Math.max(sensitivity, 0), 3);
    this.#r[R_MOUSE_SPRING] = clamp01(spring);
  }

  /** Asks for an assist level (1 Full, 2 Standard, 3 Reduced, 4 Manual). It stays requested. */
  requestAssist(level: number): void {
    if (level >= 1 && level <= 4) this.#assistReq = level;
  }

  /** Requests the next assist level after the current one: Full, Standard, Reduced, Manual. */
  cycleAssist(): number {
    const base = this.#assistReq || this.#assistSeen || 2;
    this.#assistReq = (base % 4) + 1;
    return this.#assistReq;
  }

  /** The spawn a respawn goes to: 0 the session's, 1 the runway, 2 in the air. */
  setRespawnKind(kind: number): void {
    this.#spawnReq = kind >= 0 && kind <= 2 ? kind : 0;
  }

  setAutoGcas(on: boolean): void {
    this.#autoGcas = on;
  }

  /**
   * Tells the pump what the simulation is doing. At each new spawn the throttle lever is set to
   * the aircraft's throttle and the spawn is acknowledged, which releases the throttle the sim
   * held for it (ADR-36): no lever jump. Cheap enough to call every animation frame.
   */
  observe(o: PumpObservation): void {
    if (o.spawnSeq !== this.#spawnAck) {
      this.#lever.set(clamp01(o.throttle));
      this.#spawnAck = o.spawnSeq & 0xff;
      this.#absSync = true;
    }
    this.#assistSeen = o.assist;
    this.#r[R_NOSE_AZ] = o.noseAzRad;
    this.#r[R_NOSE_EL] = o.noseElRad;
  }

  /**
   * Records that a source event happened at `eventTimeMs` (an event's `timeStamp`, on the clock of
   * `nowMs`). The next `update()` stamps its frame with the earliest such time.
   */
  poke(eventTimeMs?: number): void {
    const now = this.#d.nowMs();
    let t = eventTimeMs ?? now;
    if (!(t <= now) || now - t > MAX_EVENT_AGE_MS) t = now;
    if (t < (this.#r[R_PENDING] ?? Number.POSITIVE_INFINITY)) this.#r[R_PENDING] = t;
  }

  // -- the work ------------------------------------------------------------------------------

  /** Samples the devices and writes a frame if one is due. Returns whether it wrote one. */
  update(): boolean {
    const now = this.#d.nowMs();
    const last = this.#r[R_LAST_UPDATE] ?? Number.NaN;
    let dt = Number.isNaN(last) ? 0 : Math.min(Math.max(now - last, 0), MAX_STEP_MS);
    this.#r[R_LAST_UPDATE] = now;
    // An event just happened: there is a step to apply, so a key press makes a change at once.
    if (this.#r[R_PENDING] !== Number.POSITIVE_INFINITY && dt < 1) dt = 1;

    const pads = this.#d.pads;
    if (pads !== undefined) {
      pads.poll();
      if (pads.revision !== this.#padRevision) {
        // A device came, went or changed: look the bindings up again, drop smoothed readings (a
        // lost device is neutral at once), and let a new throttle lever take over.
        this.#padRevision = pads.revision;
        this.#map.resolve(pads);
        this.#shPitch.reset();
        this.#shRoll.reset();
        this.#shYaw.reset();
        this.#absSync = true;
      }
    }
    this.#d.mouse?.consume();
    this.#applyResync();

    this.#cur.fill(0);
    this.#buttons = 0;
    let flags = 0;
    if (this.#context === 'flight') {
      flags = this.#flight(dt);
    } else {
      this.#cur[AXIS.THROTTLE] = this.#lever.value;
      this.#pageEdges(false);
    }

    if (this.#r[R_PENDING] === Number.POSITIVE_INFINITY) this.#padSampleTime(now);
    const result = this.#emit(now, flags, false);
    // A refused frame leaves its change unsent, and with it the time of the event.
    if (result !== REFUSED) this.#r[R_PENDING] = Number.POSITIVE_INFINITY;
    return result === SENT;
  }

  /**
   * Sends one neutral frame (focus lost, tab hidden) and forgets the emulated and smoothed state,
   * so nothing stale is replayed when control returns. The throttle lever keeps its setting.
   */
  neutral(): void {
    this.#resetShaping();
    this.#prevActive.fill(0);
    this.#resync = true;
    this.#r[R_LAST_UPDATE] = Number.NaN;
    this.#r[R_PENDING] = Number.POSITIVE_INFINITY;
    this.stats.neutralFrames += 1;
    this.#cur.fill(0);
    this.#buttons = 0;
    this.#cur[AXIS.THROTTLE] = this.#lever.value;
    this.#emit(this.#d.nowMs(), INPUT_FLAG.NEUTRAL, true);
  }

  /**
   * For a change no event announced: when a pad last reported, if recently. Puts that time in the
   * pending-event register, which stays at infinity when there is none (the frame is then stamped
   * with the poll time).
   */
  #padSampleTime(now: number): void {
    const pads = this.#d.pads;
    if (pads === undefined) return;
    let latest = Number.NEGATIVE_INFINITY;
    for (let i = 0; i < pads.slots.length; i++) {
      const s = pads.slots[i];
      if (s?.present !== true) continue;
      const t = s.stamp[0] ?? 0;
      if (t <= now && now - t < MAX_PAD_AGE_MS && t > latest) latest = t;
    }
    if (latest !== Number.NEGATIVE_INFINITY) this.#r[R_PENDING] = latest;
  }

  // -- flight --------------------------------------------------------------------------------

  /** Reads every flight action into `#cur` and `#buttons`. Returns the frame flags. */
  #flight(dt: number): number {
    const mouse = this.#d.mouse;
    const locked = mouse?.locked === true;
    const r = this.#r;

    // Mouse motion drives the aim or the stick.
    if (locked && this.#mouseMode === 'aim') {
      this.#aim.move(mouse.dx, mouse.dy, r[R_MOUSE_SENS] ?? 1);
    } else if (locked && this.#mouseMode === 'stick') {
      this.#stick.move(mouse.dx, mouse.dy, r[R_MOUSE_SENS] ?? 1);
    }
    const spring = r[R_MOUSE_SPRING] ?? 0;
    if (this.#mouseMode === 'aim') {
      this.#aim.follow(r[R_NOSE_AZ] ?? 0, r[R_NOSE_EL] ?? 0, dt, spring);
    } else if (this.#mouseMode === 'stick') {
      this.#stick.update(dt, spring);
    }

    const cur = this.#cur;
    this.#bipolar(A.pitch, this.#emuPitch, this.#shPitch, dt, AXIS.PITCH);
    this.#bipolar(A.roll, this.#emuRoll, this.#shRoll, dt, AXIS.ROLL);
    this.#bipolar(A.yaw, this.#emuYaw, this.#shYaw, dt, AXIS.YAW);
    if (this.#mouseMode === 'stick' && locked) {
      cur[AXIS.PITCH] = larger(cur[AXIS.PITCH] ?? 0, this.#stick.y);
      cur[AXIS.ROLL] = larger(cur[AXIS.ROLL] ?? 0, this.#stick.x);
    }

    this.#throttle(dt);
    cur[AXIS.THROTTLE] = this.#lever.value;

    this.#unipolar(A.brakeLeft, this.#shBrakeL, AXIS.BRAKE_L);
    this.#unipolar(A.brakeRight, this.#shBrakeR, AXIS.BRAKE_R);
    if (this.#anyActive(A.brakes)) {
      cur[AXIS.BRAKE_L] = 1;
      cur[AXIS.BRAKE_R] = 1;
    }

    let flags = 0;
    if (this.#mouseMode === 'aim' && locked) {
      flags |= INPUT_FLAG.MOUSE_AIM;
      cur[AXIS.CURSOR_X] = this.#aim.cursorX;
      cur[AXIS.CURSOR_Y] = this.#aim.cursorY;
    }
    if (this.#autoGcas) flags |= INPUT_FLAG.AUTO_GCAS;

    // Toggles and the page's own actions are decided on their edges.
    this.#pageEdges(true);
    if (this.#profile.toggles.airbrake === 'hold') {
      this.#airbrakeOpen = this.#anyActive(A.airbrake);
      this.#prevActive[A.airbrake] = this.#airbrakeOpen ? 1 : 0;
    } else if (this.#edge(A.airbrake)) {
      this.#airbrakeOpen = !this.#airbrakeOpen;
    }
    if (this.#airbrakeOpen) this.#buttons |= 1 << BUTTON.AIRBRAKE;
    for (let k = 0; k < BUTTON_ACTION_INDEX.length; k++) {
      const a = BUTTON_ACTION_INDEX[k] ?? 0;
      const down = this.#anyActive(a);
      if (this.#suppress[a] === 1) {
        if (!down) this.#suppress[a] = 0;
      } else if (down) {
        this.#buttons |= 1 << (BUTTON_BITS[k] ?? 0);
      }
    }
    return flags;
  }

  /**
   * A bipolar axis into `cur[axis]`: the keyboard ramp and the shaped analogue sum, whichever is
   * pushed harder.
   */
  #bipolar(a: number, emulator: AxisEmulator, shaper: AxisShaper, dt: number, axis: number): void {
    const m = this.#map;
    let negative = false;
    let positive = false;
    let analogue = 0;
    for (let i = m.first(a), end = m.last(a); i < end; i++) {
      m.read(i, this.#dev);
      const v = m.out[0] ?? 0;
      if (m.isDigital(i)) {
        if (v > 0.5) {
          if (m.direction(i) < 0) negative = true;
          else positive = true;
        }
      } else {
        analogue += v * m.direction(i);
      }
    }
    emulator.step(dt, negative, positive);
    shaper.advance(clamp1(analogue), dt);
    this.#cur[axis] = larger(emulator.value, shaper.value);
  }

  /** A 0-to-1 axis (a brake) into `cur[axis]`: the strongest source, a key counting as full. */
  #unipolar(a: number, shape: AxisShape, axis: number): void {
    const m = this.#map;
    let best = 0;
    for (let i = m.first(a), end = m.last(a); i < end; i++) {
      if (m.isDigital(i)) {
        m.read(i, this.#dev);
        if ((m.out[0] ?? 0) > 0.5) best = 1;
      } else {
        m.readUnipolar(i, this.#dev);
        best = Math.max(best, clamp01(m.out[0] ?? 0));
      }
    }
    shapeInto(this.#cur, axis, best, shape);
    this.#cur[axis] = clamp01(this.#cur[axis] ?? 0);
  }

  /** Moves the throttle lever: an absolute lever, then keys, sticks and wheel, then afterburner. */
  #throttle(dt: number): void {
    const m = this.#map;
    const lever = this.#lever;
    const r = this.#r;

    // An absolute lever (a HOTAS throttle) sets the throttle when it moves, and when it first
    // appears or a new aircraft is spawned.
    let absolute = -1;
    for (let i = m.first(A.throttle), end = m.last(A.throttle); i < end; i++) {
      if (!m.present(i)) continue;
      m.readUnipolar(i, this.#dev);
      shapeInto(r, R_TMP, clamp01(m.out[0] ?? 0), this.#shThrottle);
      absolute = Math.max(absolute, clamp01(r[R_TMP] ?? 0));
    }
    if (absolute >= 0) {
      const prev = r[R_ABS_PREV] ?? Number.NaN;
      if (this.#absSync || Math.abs(absolute - prev) > LEVER_MOVED) lever.set(absolute);
      r[R_ABS_PREV] = absolute;
      this.#absSync = false;
    } else {
      r[R_ABS_PREV] = Number.NaN;
    }

    this.#rate(A.throttleUp, this.#shUp, R_RATE_UP);
    this.#rate(A.throttleDown, this.#shDown, R_RATE_DOWN);
    lever.step(dt, (r[R_RATE_UP] ?? 0) - (r[R_RATE_DOWN] ?? 0));
    const notches = this.#wheel(A.throttleUp) - this.#wheel(A.throttleDown);
    if (notches !== 0) lever.nudge(notches * this.#profile.throttle.wheelStep);

    if (this.#profile.toggles.afterburner === 'hold') {
      const held = this.#anyActive(A.afterburner);
      if (held !== this.#afterburnerHeld) lever.holdAfterburner(held);
      this.#afterburnerHeld = held;
      this.#prevActive[A.afterburner] = held ? 1 : 0;
    } else if (this.#edge(A.afterburner)) {
      lever.toggleAfterburner();
    }
  }

  /** The speed a throttle key or stick asks for (fraction of travel per second) into `r[reg]`. */
  #rate(a: number, shape: AxisShape, reg: number): void {
    const m = this.#map;
    const r = this.#r;
    const keyRate = this.#profile.throttle.keyRatePerSecond;
    const stickRate = this.#profile.throttle.stickRatePerSecond;
    let keys = 0;
    let stick = 0;
    for (let i = m.first(a), end = m.last(a); i < end; i++) {
      if (m.isWheel(i)) continue;
      m.read(i, this.#dev);
      const v = m.out[0] ?? 0;
      if (m.isDigital(i)) {
        if (v > 0.5) keys = keyRate;
      } else {
        shapeInto(r, R_TMP, clamp01(v), shape);
        stick = Math.max(stick, clamp01(r[R_TMP] ?? 0) * stickRate);
      }
    }
    r[reg] = Math.max(keys, stick);
  }

  /** Wheel notches this update on the wheel sources of an action (a whole number). */
  #wheel(a: number): number {
    const m = this.#map;
    let n = 0;
    for (let i = m.first(a), end = m.last(a); i < end; i++) {
      if (!m.isWheel(i)) continue;
      m.read(i, this.#dev);
      n += m.out[0] ?? 0;
    }
    return n;
  }

  // -- edges ---------------------------------------------------------------------------------

  #anyActive(a: number): boolean {
    const m = this.#map;
    for (let i = m.first(a), end = m.last(a); i < end; i++) {
      if (m.active(i, this.#dev)) return true;
    }
    return false;
  }

  /** True once when the action goes from released to pressed. */
  #edge(a: number): boolean {
    const now = this.#anyActive(a);
    const was = this.#prevActive[a] === 1;
    this.#prevActive[a] = now ? 1 : 0;
    return now && !was && !this.#resync;
  }

  /**
   * Carries out the actions the pump or the page handles on a press: pause, and in flight the
   * mouse mode, the assist cycle and the respawn notice. After a change of context (or a profile)
   * keys that are already down are noted, not pressed.
   */
  #pageEdges(flight: boolean): void {
    const onAction = this.#d.onAction;
    if (this.#edge(A.pause)) onAction?.('system.pause');
    if (flight) {
      for (let k = 0; k < PAGE_ACTION_INDEX.length; k++) {
        const a = PAGE_ACTION_INDEX[k] ?? 0;
        if (!this.#edge(a)) continue;
        if (a === A.mouseMode) this.cycleMouseMode();
        else if (a === A.assistCycle) this.cycleAssist();
        const id = PAGE_ACTION_IDS[k];
        if (id !== undefined) onAction?.(id);
      }
    }
  }

  /**
   * After a change of context or profile, notes what is already held so that it does not count as
   * a press on the next update.
   */
  #applyResync(): void {
    if (!this.#resync) return;
    for (let a = 0; a < ACTION_COUNT; a++) {
      const down = this.#anyActive(a);
      this.#prevActive[a] = down ? 1 : 0;
      // A press-type button held through the change is not pressed again by it.
      this.#suppress[a] = down && PRESS_ACTION[a] === 1 ? 1 : 0;
    }
    this.#resync = false;
  }

  // -- frames --------------------------------------------------------------------------------

  /** Builds the shapers and key ramps from the profile. */
  #applyProfile(): void {
    const p = this.#profile;
    this.#emuPitch = new AxisEmulator(p.keyboard);
    this.#emuRoll = new AxisEmulator(p.keyboard);
    this.#emuYaw = new AxisEmulator(p.keyboard);
    this.#shPitch = new AxisShaper(shapeFor(p, 'flight.pitch'));
    this.#shRoll = new AxisShaper(shapeFor(p, 'flight.roll'));
    this.#shYaw = new AxisShaper(shapeFor(p, 'flight.yaw'));
  }

  #resetShaping(): void {
    this.#emuPitch.reset();
    this.#emuRoll.reset();
    this.#emuYaw.reset();
    this.#shPitch.reset();
    this.#shRoll.reset();
    this.#shYaw.reset();
    this.#stick.reset();
  }

  /**
   * Writes a frame if something changed or the heartbeat is due. `pend` is the earliest unsent
   * source event time, if any.
   */
  #emit(now: number, flagsIn: number, force: boolean): number {
    const pend = this.#r[R_PENDING] ?? Number.POSITIVE_INFINITY;
    const cur = this.#cur;
    const last = this.#last;
    const flagBits = flagsIn & (INPUT_FLAG.MOUSE_AIM | INPUT_FLAG.AUTO_GCAS);
    let changed =
      !this.#primed || this.#buttons !== this.#lastButtons || flagBits !== this.#lastFlags;
    for (let i = 0; i < AXES; i++) {
      if (Math.abs((cur[i] ?? 0) - (last[i] ?? 0)) > EPSILON) changed = true;
    }
    const neutral = (flagsIn & INPUT_FLAG.NEUTRAL) !== 0;
    const meta =
      this.#assistReq !== this.#lastAssist ||
      this.#spawnAck !== this.#lastSpawnAck ||
      this.#spawnReq !== this.#lastSpawnReq;
    const due = now - (this.#r[R_LAST_SENT] ?? Number.NEGATIVE_INFINITY) >= this.#minInterval;
    if (!force && !changed && !meta && !due) return SKIPPED;
    if (!force && !changed && !meta) this.stats.heartbeats += 1;

    const w = this.#writer;
    this.#seq = (this.#seq + 1) >>> 0;
    const sentAt = this.#d.sharedNowMs();
    const flagged = changed && !neutral;
    w.seq = this.#seq;
    w.flags = flagsIn | (flagged ? INPUT_FLAG.CHANGED : 0);
    w.times[1] = sentAt;
    w.times[0] =
      flagged && pend !== Number.POSITIVE_INFINITY ? sentAt - (now - Math.min(pend, now)) : sentAt;
    for (let i = 0; i < AXES; i++) w.axes[i] = (cur[i] ?? 0) + 0;
    w.buttons = this.#buttons;
    w.assistReq = this.#assistReq;
    w.spawnAck = this.#spawnAck;
    w.spawnReq = this.#spawnReq;

    if (!this.#d.sink.send(w.encode())) {
      this.stats.refused += 1;
      return REFUSED;
    }
    this.stats.sent += 1;
    if (flagged) this.stats.changed += 1;
    last.set(cur);
    this.#lastButtons = this.#buttons;
    this.#lastFlags = flagBits;
    this.#lastAssist = this.#assistReq;
    this.#lastSpawnAck = this.#spawnAck;
    this.#lastSpawnReq = this.#spawnReq;
    this.#primed = true;
    this.#r[R_LAST_SENT] = now;
    return SENT;
  }
}
