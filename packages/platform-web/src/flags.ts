/**
 * URL flags: switches for debugging, testing and measuring, parsed from the query string.
 *
 * Unknown keys are ignored and bad values fall back to the default, each with a warning, so a
 * mistyped flag is visible rather than silently a different configuration.
 *
 * | Flag | Values |
 * |---|---|
 * | `debug` | `1` (overlay), `perf` (overlay plus the perf panel) |
 * | `renderer` | `auto` (default), `webgpu`, `webgl2`, `null` |
 * | `flags` | comma list of `transfer`, `mainrender`, `timer`, `stall` (test switches) |
 * | `compat` | `1`: show the compat screen even if it was acknowledged |
 * | `selftest` | `1` (golden hash), `renderer` (renderer contract), `audio` (the flight audio in this browser), `screens` (the screenshot gates; `case=horizon` or `terrain`) |
 * | `perf` | `ps0` (the empty loop) or `arena` (the circuit over the rugged arena, with the HUD and the sound): run that scenario |
 * | `seconds` | duration of a perf run, 1 to 3600 |
 * | `bench` | `quick` or `full`: run the first-run benchmark |
 * | `spawn` | `runway` (default) or `air`: where the aircraft starts |
 * | `assist` | `full`, `standard` (default), `reduced` or `manual` |
 * | `g` | `off`, `reduced` or `full` (default): the g-effects dial |
 * | `arena` | `rugged` (default) or `plain`: the procedural test arena |
 * | `fly` | `1`: skip the first run and the main menu and start flying at once (`perf` and `script` imply it) |
 * | `camera` | `cockpit` or `chase`: the camera a sortie starts in, in place of the player's setting |
 * | `script` | `circuit`: the virtual tester flies the circuit; `sortie`: the bot flies a duel's sortie (take-off, fight, landing) instead of the player (test aids) |
 * | `scenario` | `free` or `duel`: free flight or a duel, in place of the player's setting; `merge`: a duel with the bandit 3 km ahead (a test aid; forces `spawn=air`) |
 * | `skill` | `1`, `2` or `3`: the duel's bandit (Recruit, Regular, Veteran), in place of the player's setting |
 * | `res` | `WxH`, for example `1920x1080`: fixes the drawing buffer's size (perf and screenshot runs) |
 * | `quality` | `low`, `medium` or `high`: the graphics preset, in place of the one chosen (a test aid) |
 * | `view` | `classify`: draws flat key colours instead of the picture, so a test can tell from a pixel what drew it |
 * | `fault` | `flat-earth`, `no-skirts` or `hud-body-axes`: a seeded render fault the screenshot tests must catch; needs `debug` |
 */

export type RendererChoice = 'auto' | 'webgpu' | 'webgl2' | 'null';
export type SpawnChoice = 'runway' | 'air';
export type AssistChoice = 'full' | 'standard' | 'reduced' | 'manual';
export type GEffectsChoice = 'off' | 'reduced' | 'full';
export type ArenaChoice = 'rugged' | 'plain';
export type QualityChoice = 'low' | 'medium' | 'high';
export type FaultChoice = 'flat-earth' | 'no-skirts' | 'hud-body-axes';
export type ScenarioChoice = 'free' | 'duel' | 'merge';
export type ScriptChoice = 'circuit' | 'sortie';

/** Limits of `res`, pixels. */
export const RES_MIN = { width: 160, height: 90 } as const;
export const RES_MAX = { width: 7680, height: 4320 } as const;

export interface Flags {
  debug: false | 'overlay' | 'perf';
  renderer: RendererChoice;
  /** Force the degraded snapshot channel (transferable buffers) even when isolated. */
  transfer: boolean;
  /** Run the render loop on the main thread. */
  mainrender: boolean;
  /** Force the timer sleeper in the Sim Worker (no `Atomics.wait`). */
  timer: boolean;
  /** Expose the stall hooks that tests use to freeze a thread. */
  stall: boolean;
  compat: boolean;
  selftest: false | 'hash' | 'renderer' | 'audio' | 'screens';
  perf: false | 'ps0' | 'arena';
  seconds: number | undefined;
  bench: false | 'quick' | 'full';
  /** Where the aircraft starts, if the URL says; otherwise the game's default (the runway). */
  spawn: SpawnChoice | undefined;
  /** The assist level, if the URL says. */
  assist: AssistChoice | undefined;
  /** The g-effects dial, if the URL says. */
  g: GEffectsChoice | undefined;
  /** The test arena, if the URL says. */
  arena: ArenaChoice | undefined;
  /** A scripted pilot in place of the player (the circuit the exit gate flies), if the URL says. */
  /** Start flying at once, with no first run or main menu (also implied by `perf` and `script`). */
  fly: boolean;
  /** The camera a sortie starts in, if the URL says; otherwise the player's setting. */
  camera: 'cockpit' | 'chase' | undefined;
  script: false | ScriptChoice;
  /** Free flight or a duel, if the URL says (`merge` is the duel's test start, 3 km apart). */
  scenario: ScenarioChoice | undefined;
  /** The duel's bandit, 1 Recruit to 3 Veteran, if the URL says. */
  skill: 1 | 2 | 3 | undefined;
  /** A fixed drawing-buffer size in pixels, if the URL says. */
  res: { width: number; height: number } | undefined;
  /** The graphics preset, if the URL says. */
  quality: QualityChoice | undefined;
  /** A seeded render fault, if the URL says and debugging is on. */
  fault: FaultChoice | undefined;
  /** `classify` draws flat key colours instead of the picture (a test aid). */
  view: false | 'classify';
  /** Problems found while parsing, in order. */
  warnings: string[];
}

const RENDERERS: readonly RendererChoice[] = ['auto', 'webgpu', 'webgl2', 'null'];
const SWITCHES = ['transfer', 'mainrender', 'timer', 'stall'] as const;
const SPAWNS: readonly SpawnChoice[] = ['runway', 'air'];
const ASSISTS: readonly AssistChoice[] = ['full', 'standard', 'reduced', 'manual'];
const G_DIALS: readonly GEffectsChoice[] = ['off', 'reduced', 'full'];
const ARENAS: readonly ArenaChoice[] = ['rugged', 'plain'];
const QUALITIES: readonly QualityChoice[] = ['low', 'medium', 'high'];
const KNOWN = new Set([
  'debug',
  'renderer',
  'flags',
  'compat',
  'selftest',
  'perf',
  'seconds',
  'bench',
  'spawn',
  'assist',
  'g',
  'arena',
  'fly',
  'camera',
  'script',
  'res',
  'quality',
  'fault',
  'view',
  'scenario',
  'skill',
]);

const size = (r: { width: number; height: number }): string =>
  `${String(r.width)}x${String(r.height)}`;

export function defaultFlags(): Flags {
  return {
    debug: false,
    renderer: 'auto',
    transfer: false,
    mainrender: false,
    timer: false,
    stall: false,
    compat: false,
    selftest: false,
    perf: false,
    seconds: undefined,
    bench: false,
    spawn: undefined,
    assist: undefined,
    g: undefined,
    arena: undefined,
    fly: false,
    camera: undefined,
    script: false,
    scenario: undefined,
    skill: undefined,
    res: undefined,
    quality: undefined,
    fault: undefined,
    view: false,
    warnings: [],
  };
}

/** Parses `location.search` (with or without the leading `?`). */
export function parseFlags(search: string): Flags {
  const flags = defaultFlags();
  const params = new URLSearchParams(search);
  const bad = (key: string, value: string, allowed: string): void => {
    flags.warnings.push(`ignored ?${key}=${value} (use ${allowed})`);
  };

  for (const key of new Set(params.keys())) {
    if (!KNOWN.has(key)) {
      flags.warnings.push(`unknown flag ?${key}`);
    }
  }

  const debug = params.get('debug');
  if (debug === '1') {
    flags.debug = 'overlay';
  } else if (debug === 'perf') {
    flags.debug = 'perf';
  } else if (debug !== null) {
    bad('debug', debug, '1 or perf');
  }

  const renderer = params.get('renderer');
  if (renderer !== null) {
    const choice = RENDERERS.find((r) => r === renderer);
    if (choice === undefined) {
      bad('renderer', renderer, RENDERERS.join(', '));
    } else {
      flags.renderer = choice;
    }
  }

  const list = params.get('flags');
  if (list !== null) {
    for (const item of list.split(',').filter((s) => s !== '')) {
      const name = SWITCHES.find((s) => s === item);
      if (name === undefined) {
        bad('flags', item, SWITCHES.join(', '));
      } else {
        flags[name] = true;
      }
    }
  }

  const compat = params.get('compat');
  if (compat === '1') {
    flags.compat = true;
  } else if (compat !== null) {
    bad('compat', compat, '1');
  }

  const selftest = params.get('selftest');
  if (selftest === '1') {
    flags.selftest = 'hash';
  } else if (selftest === 'renderer') {
    flags.selftest = 'renderer';
  } else if (selftest === 'audio') {
    flags.selftest = 'audio';
  } else if (selftest === 'screens') {
    flags.selftest = 'screens';
  } else if (selftest !== null) {
    bad('selftest', selftest, '1, renderer, audio or screens');
  }

  const perf = params.get('perf');
  if (perf === 'ps0' || perf === 'arena') {
    flags.perf = perf;
  } else if (perf !== null) {
    bad('perf', perf, 'ps0 or arena');
  }

  const seconds = params.get('seconds');
  if (seconds !== null) {
    const n = Number(seconds);
    if (Number.isInteger(n) && n >= 1 && n <= 3600) {
      flags.seconds = n;
    } else {
      bad('seconds', seconds, 'a whole number from 1 to 3600');
    }
  }

  const bench = params.get('bench');
  if (bench === 'quick' || bench === 'full') {
    flags.bench = bench;
  } else if (bench !== null) {
    bad('bench', bench, 'quick or full');
  }

  const spawn = params.get('spawn');
  if (spawn !== null) {
    const choice = SPAWNS.find((s) => s === spawn);
    if (choice === undefined) bad('spawn', spawn, SPAWNS.join(' or '));
    else flags.spawn = choice;
  }

  const assist = params.get('assist');
  if (assist !== null) {
    const choice = ASSISTS.find((a) => a === assist);
    if (choice === undefined) bad('assist', assist, ASSISTS.join(', '));
    else flags.assist = choice;
  }

  const g = params.get('g');
  if (g !== null) {
    const choice = G_DIALS.find((d) => d === g);
    if (choice === undefined) bad('g', g, G_DIALS.join(', '));
    else flags.g = choice;
  }

  const arena = params.get('arena');
  if (arena !== null) {
    const choice = ARENAS.find((a) => a === arena);
    if (choice === undefined) bad('arena', arena, ARENAS.join(' or '));
    else flags.arena = choice;
  }

  const fly = params.get('fly');
  if (fly === '1') {
    flags.fly = true;
  } else if (fly !== null) {
    bad('fly', fly, '1');
  }

  const camera = params.get('camera');
  if (camera === 'cockpit' || camera === 'chase') {
    flags.camera = camera;
  } else if (camera !== null) {
    bad('camera', camera, 'cockpit or chase');
  }

  const script = params.get('script');
  if (script === 'circuit' || script === 'sortie') {
    flags.script = script;
  } else if (script !== null) {
    bad('script', script, 'circuit or sortie');
  }

  const scenario = params.get('scenario');
  if (scenario === 'free' || scenario === 'duel' || scenario === 'merge') {
    flags.scenario = scenario;
    // The merge starts both aircraft in the air, 3 km apart.
    if (scenario === 'merge') flags.spawn = 'air';
  } else if (scenario !== null) {
    bad('scenario', scenario, 'free, duel or merge');
  }

  const skill = params.get('skill');
  if (skill === '1' || skill === '2' || skill === '3') {
    flags.skill = Number(skill) as 1 | 2 | 3;
  } else if (skill !== null) {
    bad('skill', skill, '1, 2 or 3');
  }

  const res = params.get('res');
  if (res !== null) {
    const m = /^(\d{1,5})x(\d{1,5})$/.exec(res);
    const [w, h] = [Number(m?.[1]), Number(m?.[2])];
    if (
      m === null ||
      w < RES_MIN.width ||
      h < RES_MIN.height ||
      w > RES_MAX.width ||
      h > RES_MAX.height
    ) {
      bad('res', res, `WxH between ${size(RES_MIN)} and ${size(RES_MAX)}`);
    } else {
      flags.res = { width: w, height: h };
    }
  }

  const quality = params.get('quality');
  if (quality !== null) {
    const choice = QUALITIES.find((q) => q === quality);
    if (choice === undefined) bad('quality', quality, QUALITIES.join(', '));
    else flags.quality = choice;
  }

  const fault = params.get('fault');
  if (fault === 'flat-earth' || fault === 'no-skirts' || fault === 'hud-body-axes') {
    if (flags.debug === false) bad('fault', fault, 'combined with debug=1 (faults are test aids)');
    else flags.fault = fault;
  } else if (fault !== null) {
    bad('fault', fault, 'flat-earth, no-skirts or hud-body-axes');
  }

  const view = params.get('view');
  if (view === 'classify') {
    flags.view = 'classify';
  } else if (view !== null) {
    bad('view', view, 'classify');
  }

  return flags;
}
