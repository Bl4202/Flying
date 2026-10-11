import { securityHeaders } from './headers.ts';

/**
 * Returns a copy of `response` carrying the project's security headers.
 *
 * Opaque responses (status 0, e.g. cross-origin `no-cors`) cannot be re-wrapped and are returned
 * unchanged; the Service Worker only calls this for same-origin responses.
 */
export function withSecurityHeaders(response: Response): Response {
  if (response.status === 0 || response.type === 'opaque' || response.type === 'opaqueredirect') {
    return response;
  }
  const headers = new Headers(response.headers);
  for (const [name, value] of Object.entries(securityHeaders())) {
    headers.set(name, value);
  }
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}
