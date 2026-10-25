/**
 * Edits to a profile, as pure functions (GDD 58.5): the binding editor calls these and shows the
 * result. Each returns a new profile; the one it is given is never changed, so a pump holding the
 * old one is not disturbed until it is handed the new one.
 */
import { ACTIONS, actionDef, type ActionDef, type ActionId } from './actions.ts';
import {
  type Binding,
  BUILTIN_IDS,
  builtinProfile,
  type BuiltinId,
  type ControlsProfile,
  defaultShape,
  hasShape,
  MAX_BINDINGS_PER_ACTION,
} from './profile.ts';
import { type AxisShape } from './shaping.ts';

/** The built-in profile a profile of this class is restored from. */
export function defaultsFor(profile: ControlsProfile): ControlsProfile {
  const id: BuiltinId =
    profile.deviceClass === 'gamepad'
      ? 'gamepad-standard'
      : profile.deviceClass === 'joystick'
        ? 'generic-joystick'
        : 'default-km';
  return builtinProfile(id);
}

/** The bindings of one action, with their positions in `profile.bindings`. */
export function bindingsOf(
  profile: ControlsProfile,
  action: ActionId,
): { binding: Binding; index: number }[] {
  const out: { binding: Binding; index: number }[] = [];
  profile.bindings.forEach((binding, index) => {
    if (binding.action === action) out.push({ binding, index });
  });
  return out;
}

/** True if the action has room for another binding. */
export function hasRoom(profile: ControlsProfile, action: ActionId): boolean {
  return bindingsOf(profile, action).length < MAX_BINDINGS_PER_ACTION;
}

/** Adds a binding. Returns `undefined` if the action already has the most it can. */
export function addBinding(
  profile: ControlsProfile,
  binding: Binding,
): ControlsProfile | undefined {
  if (!hasRoom(profile, binding.action)) return undefined;
  return { ...profile, bindings: [...profile.bindings, binding] };
}

/** Removes the binding at `index` in `profile.bindings`. */
export function removeBinding(profile: ControlsProfile, index: number): ControlsProfile {
  return { ...profile, bindings: profile.bindings.filter((_, i) => i !== index) };
}

/**
 * Puts the bindings and the shape of the given actions back to what `defaults` has. If they are
 * there already the profile comes back unchanged, down to the order of its bindings.
 */
function restore(
  profile: ControlsProfile,
  defaults: ControlsProfile,
  actions: ReadonlySet<ActionId>,
): ControlsProfile {
  const removed = profile.bindings.filter((b) => actions.has(b.action));
  const restored = defaults.bindings.filter((b) => actions.has(b.action));
  // Each restored action takes the default's shape, or none if the default has none.
  const axes: ControlsProfile['axes'] = {};
  for (const a of ACTIONS) {
    if (!hasShape(a.id)) continue;
    const source = actions.has(a.id) ? defaults.axes[a.id] : profile.axes[a.id];
    if (source !== undefined) axes[a.id] = { ...source, curve: { ...source.curve } };
  }
  if (
    JSON.stringify(removed) === JSON.stringify(restored) &&
    JSON.stringify(axes) === JSON.stringify(profile.axes)
  ) {
    return profile;
  }
  const kept = profile.bindings.filter((b) => !actions.has(b.action));
  return { ...profile, bindings: [...kept, ...restored], axes };
}

/** Restores one action's bindings (and its shape) to the defaults of the profile's class. */
export function restoreAction(profile: ControlsProfile, action: ActionId): ControlsProfile {
  return restore(profile, defaultsFor(profile), new Set([action]));
}

/** Restores every action of an editor group. */
export function restoreGroup(profile: ControlsProfile, group: ActionDef['group']): ControlsProfile {
  const ids = new Set<ActionId>(ACTIONS.filter((a) => a.group === group).map((a) => a.id));
  return restore(profile, defaultsFor(profile), ids);
}

/** Restores everything: bindings, shapes, ramp rates, throttle and toggles. Id and name stay. */
export function restoreAll(profile: ControlsProfile): ControlsProfile {
  const d = defaultsFor(profile);
  return { ...d, id: profile.id, name: profile.name, deviceClass: profile.deviceClass };
}

/** Changes the shaping of an axis or rate action; a field not in `patch` is kept. */
export function setShape(
  profile: ControlsProfile,
  action: ActionId,
  patch: Partial<AxisShape>,
): ControlsProfile {
  if (!hasShape(action)) return profile;
  const current = profile.axes[action] ?? defaultShape(action);
  const next: AxisShape = { ...current, curve: { ...current.curve }, ...patch };
  return { ...profile, axes: { ...profile.axes, [action]: next } };
}

export function renameProfile(profile: ControlsProfile, name: string): ControlsProfile {
  return { ...profile, name: name.trim().slice(0, 64) };
}

/** An id built from `base` that none of `existing` has: `my-stick`, `my-stick-2`, ... */
export function uniqueProfileId(existing: readonly string[], base: string): string {
  const slug =
    base
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 56) || 'profile';
  const taken = new Set([...existing, ...BUILTIN_IDS]);
  if (!taken.has(slug)) return slug;
  for (let n = 2; ; n++) {
    const candidate = `${slug}-${String(n)}`;
    if (!taken.has(candidate)) return candidate;
  }
}

/** A copy of `profile` under a new id and name. */
export function duplicateProfile(
  profile: ControlsProfile,
  existing: readonly string[],
  name: string,
): ControlsProfile {
  return {
    ...structuredClone(profile),
    id: uniqueProfileId(existing, name),
    name: name.trim().slice(0, 64),
  };
}

/** A profile of the given class with the defaults of that class, under a new id and name. */
export function newProfile(
  deviceClass: ControlsProfile['deviceClass'],
  existing: readonly string[],
  name: string,
): ControlsProfile {
  const base = defaultsFor({ deviceClass } as ControlsProfile);
  return {
    ...base,
    id: uniqueProfileId(existing, name),
    name: name.trim().slice(0, 64),
    deviceClass,
  };
}

/** The label key of an action in the editor (`strings.ts`). */
export function actionLabelKey(action: ActionId): string {
  return actionDef(action)?.label ?? action;
}
