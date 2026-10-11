import { describe, expect, it } from 'vitest';
import { withSecurityHeaders } from './sw-headers.ts';

describe('withSecurityHeaders', () => {
  it('adds headers and keeps status, body and content type', async () => {
    const original = new Response('hello', {
      status: 200,
      statusText: 'OK',
      headers: { 'Content-Type': 'text/plain', 'X-Custom': 'kept' },
    });
    const out = withSecurityHeaders(original);
    expect(out.status).toBe(200);
    expect(await out.text()).toBe('hello');
    expect(out.headers.get('content-type')).toBe('text/plain');
    expect(out.headers.get('x-custom')).toBe('kept');
    expect(out.headers.get('cross-origin-opener-policy')).toBe('same-origin');
    expect(out.headers.get('cross-origin-embedder-policy')).toBe('require-corp');
    expect(out.headers.get('content-security-policy')).toContain("default-src 'self'");
  });

  it('overrides conflicting upstream values', () => {
    const out = withSecurityHeaders(
      new Response('x', { headers: { 'Cross-Origin-Embedder-Policy': 'unsafe-none' } }),
    );
    expect(out.headers.get('cross-origin-embedder-policy')).toBe('require-corp');
  });

  it('preserves null-body statuses', () => {
    const out = withSecurityHeaders(new Response(null, { status: 204 }));
    expect(out.status).toBe(204);
    expect(out.body).toBeNull();
  });

  it('returns opaque responses untouched', () => {
    const opaque = { status: 0, type: 'opaque' } as unknown as Response;
    expect(withSecurityHeaders(opaque)).toBe(opaque);
  });
});
