import { describe, expect, it } from 'vitest';
import { installGlobalHandlers, RingLogger, type ErrorScope } from './log.ts';

describe('RingLogger', () => {
  it('keeps the newest lines, oldest first, and counts all of them', () => {
    let t = 0;
    const log = new RingLogger(3, () => ++t);
    for (let i = 1; i <= 5; i++) log.info('sim', `line ${String(i)}`);
    expect(log.total).toBe(5);
    expect(log.entries().map((e) => e.message)).toEqual(['line 3', 'line 4', 'line 5']);
    expect(log.entries().map((e) => e.t)).toEqual([3, 4, 5]);
  });

  it('records level and source', () => {
    const log = new RingLogger(4, () => 0);
    log.debug('a', '1');
    log.warn('b', '2');
    log.error('c', '3');
    expect(log.entries().map((e) => `${e.level}:${e.source}`)).toEqual([
      'debug:a',
      'warn:b',
      'error:c',
    ]);
  });

  it('is empty before use and rejects a bad capacity', () => {
    expect(new RingLogger(2).entries()).toEqual([]);
    expect(() => new RingLogger(0)).toThrow(RangeError);
  });
});

describe('installGlobalHandlers', () => {
  it('logs uncaught errors and unhandled rejections', () => {
    const handlers: Record<string, (e: never) => void> = {};
    const scope = {
      addEventListener: (type: string, fn: (e: never) => void) => {
        handlers[type] = fn;
      },
    } as unknown as ErrorScope;
    const log = new RingLogger(4, () => 0);
    installGlobalHandlers(log, scope, 'main');
    handlers['error']?.({ message: 'boom' } as never);
    handlers['unhandledrejection']?.({ reason: new Error('nope') } as never);
    handlers['unhandledrejection']?.({ reason: 'text' } as never);
    expect(log.entries().map((e) => e.message)).toEqual([
      'uncaught: boom',
      'unhandled rejection: nope',
      'unhandled rejection: text',
    ]);
    expect(log.entries().every((e) => e.level === 'error' && e.source === 'main')).toBe(true);
  });
});
