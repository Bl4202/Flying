import { describe, expect, it } from 'vitest';
import type { SelfTestResult } from './selftest.worker.ts';
import { goldenCheckpoint, judgeSelfTest, SEED, SELFTEST_TICKS } from './selftest-judge.ts';

const EXPECTED = { combined: 'aaaaaaaaaaaaaaaa', content: 'bbbbbbbbbbbbbbbb' };

function result(over: Partial<Extract<SelfTestResult, { ok: true }>> = {}): SelfTestResult {
  return {
    ok: true,
    variant: 'single',
    abi: 2,
    tick: '10000',
    hash: EXPECTED.combined,
    content: EXPECTED.content,
    ms: 42.4,
    ...over,
  };
}

describe('the committed golden', () => {
  it('has the self-test checkpoint, for the seed the app runs with', () => {
    expect(SEED).toBe('1f1e5eed00000001');
    expect(goldenCheckpoint()?.combined).toMatch(/^[0-9a-f]{16}$/);
    expect(goldenCheckpoint()?.content).toMatch(/^[0-9a-f]{16}$/);
    expect(SELFTEST_TICKS).toBe(10_000);
  });
});

describe('judgeSelfTest', () => {
  it('passes when state and content both match', () => {
    const j = judgeSelfTest(result(), EXPECTED);
    expect(j.ok).toBe(true);
    expect(j.text).toContain('single build, ABI 2, 10000 ticks in 42 ms');
    expect(j.text).not.toContain('expected');
  });

  it('fails, and says what was expected, when the state hash differs', () => {
    const j = judgeSelfTest(result({ hash: 'cccccccccccccccc' }), EXPECTED);
    expect(j.ok).toBe(false);
    expect(j.text).toContain('cccccccccccccccc  expected aaaaaaaaaaaaaaaa');
  });

  it('fails when the content pack is not the one the golden was made with', () => {
    const j = judgeSelfTest(result({ content: 'dddddddddddddddd' }), EXPECTED);
    expect(j.ok).toBe(false);
    expect(j.text).toContain('dddddddddddddddd  expected bbbbbbbbbbbbbbbb');
  });

  it('fails with the worker’s message when the run failed', () => {
    expect(judgeSelfTest({ ok: false, error: 'SYS-005: nope' }, EXPECTED)).toEqual({
      ok: false,
      text: 'SYS-005: nope',
    });
  });

  it('fails when there is no golden to compare with', () => {
    expect(judgeSelfTest(result(), undefined).ok).toBe(false);
  });

  it('accepts exactly the committed golden values', () => {
    const g = goldenCheckpoint();
    expect(g).toBeDefined();
    const same = result({ hash: g?.combined ?? '', content: g?.content ?? '' });
    expect(judgeSelfTest(same, g).ok).toBe(true);
  });
});
