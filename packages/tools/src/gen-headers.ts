/**
 * Writes a Netlify/Cloudflare-style `_headers` file from the single header source, for hosts
 * that can set response headers. Usage: node gen-headers.ts <dist-dir>
 */
import { writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { securityHeaders } from '@flying/platform-web/headers';

const dist = resolve(process.argv[2] ?? 'dist');
const lines = ['/*', ...Object.entries(securityHeaders()).map(([k, v]) => `  ${k}: ${v}`)];
writeFileSync(join(dist, '_headers'), `${lines.join('\n')}\n`);
console.log(`wrote ${join(dist, '_headers')}`);
