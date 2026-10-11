/** Master simulation rate (ADR-07). */
export const SIM_HZ = 60;
/** Milliseconds per tick. */
export const SIM_DT_MS = 1000 / SIM_HZ;
/** Catch-up ticks allowed per loop iteration before the sim runs slow instead (GDD 0.6.1). */
export const MAX_CATCH_UP_TICKS = 5;
