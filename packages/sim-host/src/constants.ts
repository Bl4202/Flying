/** Master simulation rate (ADR-07). */
export const SIM_HZ = 60;
/** Milliseconds per tick. */
export const SIM_DT_MS = 1000 / SIM_HZ;
/** Catch-up ticks allowed per loop iteration before the sim runs slow instead (GDD 0.6.1). */
export const MAX_CATCH_UP_TICKS = 5;
/** Terrain patch post spacing, metres: 128 intervals of 250 m make a 32 km patch. */
export const TERRAIN_SPACING_M = 250;
/** The patch is regenerated around the aircraft once it is this far from the centre, metres. */
export const TERRAIN_RECENTRE_M = 8_000;
/** Patch centres snap to this grid, metres. */
export const TERRAIN_SNAP_M = 1_000;
/** How often the worker checks whether the patch should move, milliseconds. */
export const TERRAIN_CHECK_MS = 500;
/** A patch fill slower than this is reported on the console, milliseconds. */
export const TERRAIN_FILL_WARN_MS = 8;
/** How often the worker reports statistics, milliseconds (4 Hz). */
export const STATS_INTERVAL_MS = 250;
/** Polling interval while paused or idle, milliseconds. */
export const IDLE_POLL_MS = 8;
