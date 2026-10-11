import { afterEach, describe, expect, it } from 'vitest';
import { scriptUrl } from './trusted-types.ts';

const g = globalThis as { trustedTypes?: unknown };

afterEach(() => {
  delete g.trustedTypes;
});

describe('scriptUrl', () => {
  it('resolves same-origin URLs to a string without Trusted Types', () => {
    expect(scriptUrl('/Flying/sw.js', 'https://example.test')).toBe(
      'https://example.test/Flying/sw.js',
    );
  });

  it('rejects cross-origin URLs', () => {
    expect(() => scriptUrl('https://evil.test/x.js', 'https://example.test')).toThrow(
      /cross-origin/,
    );
  });

  it('routes through the policy when Trusted Types exist', () => {
    const made: string[] = [];
    g.trustedTypes = {
      createPolicy: (name: string) => {
        expect(name).toBe('flying-app');
        return {
          createScriptURL: (u: string) => {
            made.push(u);
            return `trusted:${u}`;
          },
        };
      },
    };
    expect(scriptUrl('/a.js', 'https://example.test')).toBe('trusted:https://example.test/a.js');
    expect(made).toEqual(['https://example.test/a.js']);
  });
});
