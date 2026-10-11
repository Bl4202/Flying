import { describe, expect, it } from 'vitest';
import { cspValue, ISOLATION_HEADERS, securityHeaders } from './headers.ts';

describe('headers', () => {
  it('isolation headers are exactly COOP/COEP/CORP', () => {
    expect(ISOLATION_HEADERS).toEqual({
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Resource-Policy': 'same-origin',
    });
  });

  it('CSP has no unsafe-inline or unsafe-eval and enforces trusted types', () => {
    const csp = cspValue();
    expect(csp).not.toContain('unsafe-inline');
    // 'wasm-unsafe-eval' is the one documented exception.
    expect(csp).not.toMatch(/(?<!wasm-)unsafe-eval/);
    expect(csp).toContain("script-src 'self' 'wasm-unsafe-eval'");
    expect(csp).toContain("require-trusted-types-for 'script'");
    expect(csp).toContain('trusted-types flying-app');
    expect(csp).toContain("object-src 'none'");
  });

  it('securityHeaders matches the pinned snapshot', () => {
    expect(securityHeaders()).toMatchInlineSnapshot(`
      {
        "Content-Security-Policy": "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'self'; require-trusted-types-for 'script'; trusted-types flying-app",
        "Cross-Origin-Embedder-Policy": "require-corp",
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Resource-Policy": "same-origin",
        "Referrer-Policy": "no-referrer",
        "X-Content-Type-Options": "nosniff",
      }
    `);
  });
});
