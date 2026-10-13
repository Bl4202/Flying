/**
 * Watches the Sim Worker's 4 Hz reports. If they stop for a while the person is told once (and
 * again after each further interval of silence), because a sim that has gone quiet looks, from the
 * outside, exactly like a frozen game. It does not kill anything: a worker that dies says so
 * itself through its error event.
 */

export const SILENCE_WARN_MS = 5000;

export interface SupervisorDeps {
  /** `performance.now()` of the newest report, or 0 before the first. */
  lastReportAt(): number;
  now(): number;
  /** Called with whole seconds of silence. */
  onSilent(seconds: number): void;
}

export class Supervisor {
  readonly #d: SupervisorDeps;
  readonly #startedAt: number;
  #warnedAt = Number.NEGATIVE_INFINITY;

  constructor(deps: SupervisorDeps) {
    this.#d = deps;
    this.#startedAt = deps.now();
  }

  /** Call about once a second. */
  check(): void {
    const now = this.#d.now();
    // Silence runs from the newest report, or from when watching began if none came yet.
    const since = Math.max(this.#d.lastReportAt(), this.#startedAt);
    const silent = now - since;
    if (silent >= SILENCE_WARN_MS && now - this.#warnedAt >= SILENCE_WARN_MS) {
      this.#warnedAt = now;
      this.#d.onSilent(Math.floor(silent / 1000));
    }
  }
}
