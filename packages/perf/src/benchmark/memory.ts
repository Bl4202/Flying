/** What the browser will say about memory. Chrome says the most; everywhere else some fields are null. */

export interface MemoryHints {
  /** `navigator.deviceMemory`: a rounded figure in GB that Chrome stops at 8, or null. */
  deviceMemoryGb: number | null;
  /** The JS heap limit in MB (Chrome's `performance.memory`), or null. */
  jsHeapLimitMb: number | null;
  jsHeapUsedMb: number | null;
  /** `measureUserAgentSpecificMemory` in MB; only available when the page is isolated. */
  measuredMb: number | null;
}

export interface MemoryEnv {
  navigator: { deviceMemory?: number };
  performance: {
    memory?: { jsHeapSizeLimit: number; usedJSHeapSize: number };
    measureUserAgentSpecificMemory?: () => Promise<{ bytes: number }>;
  };
}

const MB = 1024 * 1024;

export async function readMemoryHints(env: MemoryEnv): Promise<MemoryHints> {
  let measured: number | null = null;
  try {
    const m = await env.performance.measureUserAgentSpecificMemory?.();
    if (m !== undefined) measured = m.bytes / MB;
  } catch {
    // needs cross-origin isolation, and some browsers still refuse
  }
  const heap = env.performance.memory;
  return {
    deviceMemoryGb: env.navigator.deviceMemory ?? null,
    jsHeapLimitMb: heap === undefined ? null : heap.jsHeapSizeLimit / MB,
    jsHeapUsedMb: heap === undefined ? null : heap.usedJSHeapSize / MB,
    measuredMb: measured,
  };
}
