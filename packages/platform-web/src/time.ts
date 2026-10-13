/**
 * Time that two threads can compare. `performance.now()` is relative to each thread's own origin,
 * so a timestamp taken in the Sim Worker means nothing in the Render Worker; adding
 * `performance.timeOrigin` makes both sit on the same wall-clock axis (to within the browser's
 * clock resolution and drift). Used for latency and publish-to-draw measurement only: wall-clock
 * values never feed simulation state or hashes.
 */
export function sharedNowMs(): number {
  return performance.timeOrigin + performance.now();
}

/** Monotonic milliseconds on this thread, for durations. */
export function monotonicMs(): number {
  return performance.now();
}
