/** Helpers for the channel contract suites. Not production code. */

/** Polls `done` until it returns true, yielding to the event loop between tries. */
export async function waitFor(done: () => boolean, what: string, timeoutMs = 3000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!done()) {
    if (Date.now() > deadline) {
      throw new Error(`timed out waiting for ${what}`);
    }
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 1);
    });
  }
}
