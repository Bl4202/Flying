import { describe, expect, it } from 'vitest';
import { classify } from './sw-routing.ts';

const scope = 'https://example.test/Flying/';
const get = (url: string, mode = 'cors') => ({ url, method: 'GET', mode });

describe('classify', () => {
  it('treats navigations in scope as navigation', () => {
    expect(classify(get('https://example.test/Flying/', 'navigate'), scope)).toBe('navigation');
    expect(classify(get('https://example.test/Flying/?x=1', 'navigate'), scope)).toBe('navigation');
  });
  it('treats other same-origin GETs in scope as assets', () => {
    expect(classify(get('https://example.test/Flying/assets/a-12345678.js'), scope)).toBe('asset');
  });
  it('passes through non-GET, cross-origin and out-of-scope requests', () => {
    expect(
      classify({ url: 'https://example.test/Flying/x', method: 'POST', mode: 'cors' }, scope),
    ).toBe('passthrough');
    expect(classify(get('https://cdn.test/Flying/a.js'), scope)).toBe('passthrough');
    expect(classify(get('https://example.test/other/a.js'), scope)).toBe('passthrough');
  });
});
