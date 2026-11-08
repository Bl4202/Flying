/**
 * Counts the main thread's long tasks (more than 50 ms, the browser's `longtask` entries) from the
 * moment it is made, for the arena run's "no long task after warm-up" gate. The runner takes the
 * difference between two readings, so it does not matter that the count starts at page load.
 *
 * Where the browser does not report them (Firefox, Safari) the count is null: a missing reading,
 * which the gate says is missing, not a good one.
 */

interface EntryList {
  getEntries(): readonly unknown[];
}

interface ObserverLike {
  observe(options: { type: string; buffered: boolean }): void;
  disconnect(): void;
}

/** Makes an observer that calls `callback` with each batch of entries, or is undefined where there is no `longtask`. */
export type ObserverFactory = (callback: (list: EntryList) => void) => ObserverLike;

export interface LongTaskCount {
  /** Long tasks seen so far, or null where the browser cannot tell. */
  count(): number | null;
  stop(): void;
}

function browserFactory(): ObserverFactory | undefined {
  if (typeof PerformanceObserver === 'undefined') return undefined;
  const types: readonly string[] = PerformanceObserver.supportedEntryTypes;
  if (!types.includes('longtask')) return undefined;
  return (callback) =>
    new PerformanceObserver((list) => {
      callback(list);
    });
}

export function countLongTasks(
  make: ObserverFactory | undefined = browserFactory(),
): LongTaskCount {
  const unsupported: LongTaskCount = { count: () => null, stop: () => undefined };
  if (make === undefined) return unsupported;
  let seen = 0;
  try {
    const observer = make((list) => {
      seen += list.getEntries().length;
    });
    observer.observe({ type: 'longtask', buffered: true });
    return {
      count: () => seen,
      stop: () => {
        observer.disconnect();
      },
    };
  } catch {
    return unsupported;
  }
}
