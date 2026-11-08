import { describe, expect, it } from 'vitest';
import { countLongTasks, type ObserverFactory } from './long-tasks.ts';

/** An observer the test feeds by hand. */
function fake() {
  let deliver: ((list: { getEntries(): readonly unknown[] }) => void) | undefined;
  const log: string[] = [];
  const make: ObserverFactory = (callback) => {
    deliver = callback;
    return {
      observe: (options) =>
        log.push(`observe ${options.type} buffered=${String(options.buffered)}`),
      disconnect: () => log.push('disconnect'),
    };
  };
  return {
    make,
    log,
    push: (n: number) => deliver?.({ getEntries: () => Array.from({ length: n }, () => ({})) }),
  };
}

describe('countLongTasks', () => {
  it('counts every long task from the first, including those from before it was made', () => {
    const f = fake();
    const c = countLongTasks(f.make);
    expect(f.log).toEqual(['observe longtask buffered=true']);
    expect(c.count()).toBe(0);
    f.push(2);
    f.push(1);
    expect(c.count()).toBe(3);
  });

  it('stops listening when told to', () => {
    const f = fake();
    countLongTasks(f.make).stop();
    expect(f.log).toContain('disconnect');
  });

  it('says it cannot tell where the browser reports no long tasks', () => {
    expect(countLongTasks(undefined).count()).toBeNull();
    const refusing: ObserverFactory = () => {
      throw new TypeError('unsupported entry type');
    };
    expect(countLongTasks(refusing).count()).toBeNull();
  });
});
