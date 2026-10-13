/**
 * A bounded in-memory log. Hot paths log rarely; when a worker dies the main thread still has the
 * last few hundred lines to show, without any of it having touched the network.
 */

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

export interface LogEntry {
  /** Milliseconds on the shared clock (see `sharedNowMs`). */
  t: number;
  level: LogLevel;
  source: string;
  message: string;
}

export class RingLogger {
  readonly capacity: number;
  #entries: (LogEntry | undefined)[];
  #next = 0;
  #total = 0;
  #now: () => number;

  constructor(
    capacity = 256,
    now: () => number = () => performance.timeOrigin + performance.now(),
  ) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError('capacity must be a positive whole number');
    }
    this.capacity = capacity;
    this.#entries = new Array<LogEntry | undefined>(capacity);
    this.#now = now;
  }

  log(level: LogLevel, source: string, message: string): void {
    this.#entries[this.#next] = { t: this.#now(), level, source, message };
    this.#next = (this.#next + 1) % this.capacity;
    this.#total += 1;
  }

  debug(source: string, message: string): void {
    this.log('debug', source, message);
  }
  info(source: string, message: string): void {
    this.log('info', source, message);
  }
  warn(source: string, message: string): void {
    this.log('warn', source, message);
  }
  error(source: string, message: string): void {
    this.log('error', source, message);
  }

  /** Lines ever logged, including those that have since been overwritten. */
  get total(): number {
    return this.#total;
  }

  /** The retained lines, oldest first. */
  entries(): LogEntry[] {
    const out: LogEntry[] = [];
    for (let i = 0; i < this.capacity; i++) {
      const e = this.#entries[(this.#next + i) % this.capacity];
      if (e !== undefined) out.push(e);
    }
    return out;
  }
}

/** The slice of a global scope that can report uncaught errors. */
export interface ErrorScope {
  addEventListener(type: 'error', listener: (e: { message?: string }) => void): void;
  addEventListener(type: 'unhandledrejection', listener: (e: { reason?: unknown }) => void): void;
}

/** Sends uncaught errors and unhandled rejections to `logger`. Returns nothing to undo: install once. */
export function installGlobalHandlers(logger: RingLogger, scope: ErrorScope, source: string): void {
  scope.addEventListener('error', (e) => {
    logger.error(source, `uncaught: ${e.message ?? 'unknown error'}`);
  });
  scope.addEventListener('unhandledrejection', (e) => {
    const r = e.reason;
    logger.error(source, `unhandled rejection: ${r instanceof Error ? r.message : String(r)}`);
  });
}
