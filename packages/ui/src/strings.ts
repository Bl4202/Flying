/**
 * User-visible text, by key (GDD 53.10). Code never builds a sentence from pieces: it asks for a
 * key and fills `{name}` slots, so a translation can reorder words. English is the only table in
 * v0.0b; the type of `t` makes a missing key a compile error, and a test checks that every key in
 * the table is used and every key-shaped string in the sources is in the table.
 */

const EN = {
  'app.name': 'Flying',
  'app.loading': 'Starting…',

  'compat.title': 'System check',
  'compat.verdict.ok': 'Everything this version needs is available.',
  'compat.verdict.degraded':
    'This browser can run the game, with the limits listed below. You can continue.',
  'compat.verdict.fatal': 'This browser cannot run the game. See the remedies below.',
  'compat.state.ok': 'OK',
  'compat.state.degraded': 'Reduced',
  'compat.state.missing': 'Missing',
  'compat.required': 'Required',
  'compat.optional': 'Optional',
  'compat.column.feature': 'Feature',
  'compat.column.need': 'Need',
  'compat.column.state': 'State',
  'compat.column.detail': 'What it means',
  'compat.remedy': 'What to do: {text}',
  'compat.modes.title': 'How the game will run',
  'compat.mode.simulation': 'Simulation build',
  'compat.mode.snapshots': 'Snapshot channel',
  'compat.mode.input': 'Input channel',
  'compat.mode.render-host': 'Drawing happens on',
  'compat.mode.renderer': 'Renderer',
  'compat.mode.sleeper': 'Tick timer',
  'compat.mode.variant.threads': 'multi-threaded WebAssembly',
  'compat.mode.variant.single': 'single-threaded WebAssembly',
  'compat.mode.variant.none': 'none',
  'compat.mode.channel.shared': 'shared memory',
  'compat.mode.channel.message': 'copied messages',
  'compat.mode.host.worker': 'a worker thread',
  'compat.mode.host.main': 'the main thread',
  'compat.mode.renderer.webgpu': 'WebGPU',
  'compat.mode.renderer.webgl2': 'WebGL2',
  'compat.mode.renderer.null': 'none (test mode)',
  'compat.mode.renderer.none': 'none available',
  'compat.mode.sleeper.atomics': 'precise (Atomics.wait)',
  'compat.mode.sleeper.timer': 'browser timers',
  'compat.codes.title': 'Error codes',
  'compat.continue': 'Continue',
  'compat.reload': 'Check again',
  'compat.hint.keyboard': 'Press Enter to continue.',

  'compat.wasm.label': 'WebAssembly',
  'compat.wasm.ok': 'WebAssembly is available.',
  'compat.wasm.missing': 'This browser has no WebAssembly, so the simulation cannot run.',
  'compat.wasm.remedy': 'Use a current version of Chrome, Edge, Firefox or Safari.',

  'compat.simd.label': 'WebAssembly SIMD',
  'compat.simd.ok': 'SIMD instructions are available.',
  'compat.simd.missing': 'This browser lacks WebAssembly SIMD, which the simulation is built for.',
  'compat.simd.remedy':
    'Update the browser. SIMD arrived in Chrome 91, Firefox 89 and Safari 16.4.',

  'compat.renderer.label': 'Graphics',
  'compat.renderer.webgpu': 'WebGPU is available ({adapter}).',
  'compat.renderer.webgpu-bare': 'WebGPU is available.',
  'compat.renderer.webgl2':
    'WebGPU is not available; the WebGL2 compatibility renderer will draw with fewer effects.',
  'compat.renderer.null': 'Drawing is switched off for testing.',
  'compat.renderer.missing': 'No usable graphics: neither WebGPU nor WebGL2 could be started.',
  'compat.renderer.forced-missing':
    'The renderer chosen in the address (renderer={choice}) is not available here.',
  'compat.renderer.remedy':
    'Turn on hardware acceleration in the browser settings, update the graphics driver, or use a Chromium-based browser.',

  'compat.isolation.label': 'Cross-origin isolation',
  'compat.isolation.ok': 'Shared memory is available between threads.',
  'compat.isolation.degraded':
    'No shared memory. The simulation and renderer exchange copies of the data, which uses a little more time.',
  'compat.isolation.remedy':
    'Reload once so the page can isolate itself, or open it from a host that sends the isolation headers.',

  'compat.threads.label': 'WebAssembly threads',
  'compat.threads.ok': 'The multi-threaded build will be used.',
  'compat.threads.degraded': 'The single-threaded build will be used.',
  'compat.threads.remedy': 'Needs cross-origin isolation and a browser with WebAssembly threads.',

  'compat.offscreen.label': 'OffscreenCanvas',
  'compat.offscreen.ok': 'Drawing runs on its own thread.',
  'compat.offscreen.degraded':
    'Drawing runs on the main thread, so a busy page can make the picture stutter.',
  'compat.offscreen.remedy': 'Use a current version of Chrome, Edge, Firefox or Safari.',

  'compat.sw.label': 'Service Worker',
  'compat.sw.ok': 'The game can be cached and started offline.',
  'compat.sw.degraded': 'No offline start, and no isolation on hosts that cannot send headers.',
  'compat.sw.remedy':
    'Allow site data and workers for this page (private windows and some settings block them).',

  'compat.opfs.label': 'File storage (OPFS)',
  'compat.opfs.ok': 'Large downloads can be kept in the browser’s file storage.',
  'compat.opfs.degraded': 'Large downloads will go to IndexedDB, which has lower limits.',
  'compat.opfs.remedy': 'Update the browser, or allow site storage for this page.',

  'compat.gamepad.label': 'Gamepad',
  'compat.gamepad.ok': 'Game controllers can be used.',
  'compat.gamepad.degraded': 'Game controllers are not supported here; the keyboard still works.',
  'compat.gamepad.remedy': 'Use a current version of Chrome, Edge or Firefox for controllers.',

  'fatal.title': 'The game stopped',
  'fatal.sim-worker': 'The simulation stopped responding.',
  'fatal.render-worker': 'Drawing stopped and could not be restarted.',
  'fatal.unknown': 'Something went wrong that the game could not recover from.',
  'fatal.reload': 'Reload',
  'fatal.code': 'Code {code}',
  'fatal.detail': 'Details',

  'overlay.title': 'Debug',
  'overlay.hint': 'Ctrl+Shift+D hides this',
  'overlay.hash': 'Hash',
  'overlay.selftest': 'Self-test',
  'overlay.benchmark': 'Benchmark',
  'overlay.pause': 'Pause',
  'overlay.resume': 'Resume',
  'overlay.busy': 'Running…',
  'overlay.pass': 'PASS',
  'overlay.fail': 'FAIL',

  'toast.info': 'Notice',
  'toast.warn': 'Warning',
  'toast.error': 'Problem',
  'toast.renderer-fallback': 'Switched to the {backend} renderer.',
  'toast.renderer-lost': 'The graphics device was lost: {reason}',
  'toast.sim-silent': 'The simulation has not reported for {seconds} seconds.',
  'toast.paused': 'Paused',
  'toast.resumed': 'Resumed',
} as const;

export type StringKey = keyof typeof EN;
export type Params = Readonly<Record<string, string | number>>;

/** Every key and its English text. */
export const STRINGS: Readonly<Record<StringKey, string>> = EN;

/** The text for `key`, with each `{name}` replaced from `params` (an unknown slot stays as is). */
export function t(key: StringKey, params?: Params): string {
  const text = STRINGS[key];
  if (params === undefined) {
    return text;
  }
  return text.replace(/\{(\w+)\}/g, (slot, name: string) => {
    const value = params[name];
    return value === undefined ? slot : String(value);
  });
}
