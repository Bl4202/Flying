/// <reference types="vite/client" />

/** Git commit the build was made from (or `dev`). Injected by Vite `define` and by the SW build. */
declare const __BUILD_ID__: string;
/** Initial pages of the shared memory the threads WASM build imports (from wasm-manifest.json). */
declare const __THREADS_INITIAL_PAGES__: number;
