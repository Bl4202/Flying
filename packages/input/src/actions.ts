/**
 * The catalogue of things a player can do with the controls (GDD 58.1, 58.5, ADR-37).
 *
 * An action is what the game understands ("pitch", "gear"); a binding (`profile.ts`) says which
 * physical input drives it. The catalogue is data: the binding editor lists it, the conflict check
 * reads it, the pump turns each action into a field of the input frame.
 *
 * Kinds:
 *
 * - `axis`: a value, -1 to 1 (`bipolar`) or 0 to 1 (`unipolar`). Keys push it with the keyboard
 *   emulation, sticks and pedals set it, buttons push it to an end.
 * - `button`: acts when pressed (the core sees the rising edge).
 * - `hold`: acts while held (trim, brakes, recover).
 * - `toggle`: each press flips a state the host keeps (airbrake, afterburner); the host sends the
 *   state as a held button, so the core never has to know.
 * - `rate`: moves the throttle lever while held, a fraction of its travel per second.
 */

export type ActionKind = 'axis' | 'button' | 'hold' | 'toggle' | 'rate';

/** Where an action works: in the flight, or while a menu is open. */
export type ActionContext = 'flight' | 'menu';

export interface ActionDef {
  /** Stable id, `group.name`. Profiles store it. */
  readonly id: string;
  readonly kind: ActionKind;
  /** Axes only: whether the value runs from -1 or from 0. */
  readonly range?: 'bipolar' | 'unipolar';
  readonly context: ActionContext;
  /** The group the binding editor lists it under. */
  readonly group: 'axes' | 'engine' | 'systems' | 'brakes' | 'weapons' | 'view' | 'system';
  /** `strings.ts` key (in the `ui` package) of its label. */
  readonly label: string;
}

/** An action definition that remembers its literal id, so `ActionId` is the union of the ids. */
type Def<I extends string> = ActionDef & { readonly id: I };

const def = <const I extends string>(
  id: I,
  kind: ActionKind,
  group: ActionDef['group'],
  extra: { range?: 'bipolar' | 'unipolar'; context?: ActionContext } = {},
): Def<I> => ({
  id,
  kind,
  group,
  context: extra.context ?? 'flight',
  label: `action.${id}`,
  ...(extra.range !== undefined ? { range: extra.range } : {}),
});

/**
 * The version of this catalogue. A profile records the version it was made for (`catalogue`; none
 * means 1, the v0.1b catalogue), so that actions added later can be given their default bindings
 * once, where the inputs are free, and not again if the player removes them.
 */
export const ACTION_CATALOGUE_VERSION = 3;

/** The actions added in each catalogue version after the first. */
export const ACTIONS_ADDED_IN: Readonly<Record<number, readonly string[]>> = {
  2: [
    'flight.camera_cycle',
    'flight.free_look',
    'flight.zoom',
    'flight.look_left',
    'flight.look_right',
    'flight.look_up',
    'flight.look_down',
    'flight.look_recentre',
    'flight.hud_declutter',
    'flight.hud_brightness_up',
    'flight.hud_brightness_down',
  ],
  // v0.1d (ADR-37 amendment): the gun and the missile.
  3: ['flight.fire_gun', 'flight.fire_missile'],
};

/** Every action, in the order the binding editor shows them. */
export const ACTIONS = [
  def('flight.pitch', 'axis', 'axes', { range: 'bipolar' }),
  def('flight.roll', 'axis', 'axes', { range: 'bipolar' }),
  def('flight.yaw', 'axis', 'axes', { range: 'bipolar' }),
  def('flight.throttle', 'axis', 'engine', { range: 'unipolar' }),
  def('flight.throttle_up', 'rate', 'engine'),
  def('flight.throttle_down', 'rate', 'engine'),
  def('flight.afterburner', 'toggle', 'engine'),
  def('flight.trim_nose_up', 'hold', 'axes'),
  def('flight.trim_nose_down', 'hold', 'axes'),
  def('flight.trim_reset', 'button', 'axes'),
  def('flight.gear', 'button', 'systems'),
  def('flight.flaps_down', 'button', 'systems'),
  def('flight.flaps_up', 'button', 'systems'),
  def('flight.flaps_cycle', 'button', 'systems'),
  def('flight.airbrake', 'toggle', 'systems'),
  def('flight.parking_brake', 'button', 'brakes'),
  def('flight.brakes', 'hold', 'brakes'),
  def('flight.brake_left', 'axis', 'brakes', { range: 'unipolar' }),
  def('flight.brake_right', 'axis', 'brakes', { range: 'unipolar' }),
  def('flight.recover', 'hold', 'systems'),
  def('flight.mouse_mode', 'button', 'view'),
  def('flight.assist_cycle', 'button', 'systems'),
  // The weapons (ADR-48): the gun fires while held, the missile launches on the press.
  def('flight.fire_gun', 'hold', 'weapons'),
  def('flight.fire_missile', 'button', 'weapons'),
  // The view (ADR-41): none of these reach the input frame; the pump reports them as `ViewInput`.
  def('flight.camera_cycle', 'button', 'view'),
  def('flight.free_look', 'hold', 'view'),
  def('flight.zoom', 'hold', 'view'),
  def('flight.look_left', 'hold', 'view'),
  def('flight.look_right', 'hold', 'view'),
  def('flight.look_up', 'hold', 'view'),
  def('flight.look_down', 'hold', 'view'),
  def('flight.look_recentre', 'button', 'view'),
  def('flight.hud_declutter', 'button', 'view'),
  def('flight.hud_brightness_up', 'button', 'view'),
  def('flight.hud_brightness_down', 'button', 'view'),
  def('system.respawn', 'button', 'system'),
  def('system.pause', 'button', 'system', { context: 'menu' }),
] as const satisfies readonly ActionDef[];

export type ActionId = (typeof ACTIONS)[number]['id'];

const BY_ID: ReadonlyMap<string, ActionDef> = new Map(ACTIONS.map((a) => [a.id, a]));
const INDEX_OF: ReadonlyMap<string, number> = new Map(ACTIONS.map((a, i) => [a.id, i]));

/** The action with this id, or `undefined`. */
export function actionDef(id: string): ActionDef | undefined {
  return BY_ID.get(id);
}

/** True if `id` names an action. */
export function isActionId(id: string): id is ActionId {
  return BY_ID.has(id);
}

/** The position of an action in `ACTIONS`, or -1. The pump's tables are indexed by it. */
export function actionIndex(id: string): number {
  return INDEX_OF.get(id) ?? -1;
}

/** How many actions there are. */
export const ACTION_COUNT = ACTIONS.length;

/** The device classes a profile is made for. */
export const DEVICE_CLASSES = ['keyboard-mouse', 'gamepad', 'joystick'] as const;
export type DeviceClass = (typeof DEVICE_CLASSES)[number];
