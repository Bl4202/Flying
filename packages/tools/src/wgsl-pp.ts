import { readFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';

/** Thrown for any preprocessing problem; the message names file and line. */
export class WgslPreprocessError extends Error {
  override name = 'WgslPreprocessError';
}

export interface PreprocessOptions {
  /** Predefined `#define`s. */
  defines?: Record<string, string>;
  /** File reader; injectable for tests. Must throw if the file does not exist. */
  readFile?: (path: string) => string;
}

const INCLUDE = /^\s*#include\s+"([^"]+)"\s*$/;
const DEFINE = /^\s*#define\s+([A-Za-z_]\w*)\s+(.+?)\s*$/;

/**
 * Expands `#include "file"` and `#define NAME value` in a WGSL file (GDD 61.3).
 *
 * - Includes resolve relative to the including file, and are emitted once (diamonds are fine).
 * - An include cycle or a missing file is an error.
 * - `#define` is whole-word textual substitution for the lines that follow it, across files.
 * - Any other `#` directive is an error, because WGSL has no preprocessor of its own.
 * - Include boundaries are marked with `//` comments so errors in the output can be traced back.
 */
export function preprocess(entry: string, options: PreprocessOptions = {}): string {
  const readFile = options.readFile ?? ((p: string) => readFileSync(p, 'utf8'));
  const root = dirname(resolve(entry));
  const defines = new Map<string, string>(Object.entries(options.defines ?? {}));
  const included = new Set<string>();
  const out: string[] = [];
  const name = (path: string): string => relative(root, path).split('\\').join('/');

  const expand = (path: string, stack: readonly string[], from: string): void => {
    if (stack.includes(path)) {
      throw new WgslPreprocessError(`include cycle: ${[...stack, path].map(name).join(' -> ')}`);
    }
    if (included.has(path)) {
      return;
    }
    included.add(path);
    let text: string;
    try {
      text = readFile(path);
    } catch {
      throw new WgslPreprocessError(`include not found: ${name(path)} (from ${from})`);
    }
    const lines = text.split(/\r?\n/);
    lines.forEach((line, i) => {
      const where = `${name(path)}:${String(i + 1)}`;
      const inc = INCLUDE.exec(line);
      if (inc?.[1] !== undefined) {
        const target = resolve(dirname(path), inc[1]);
        out.push(`// >>> include ${name(target)}`);
        expand(target, [...stack, path], where);
        out.push(`// <<< ${name(target)}`);
        return;
      }
      const def = DEFINE.exec(line);
      if (def?.[1] !== undefined && def[2] !== undefined) {
        defines.set(def[1], def[2]);
        out.push(`// ${line.trim()}`);
        return;
      }
      if (/^\s*#/.test(line)) {
        throw new WgslPreprocessError(`unsupported directive at ${where}: ${line.trim()}`);
      }
      let result = line;
      for (const [key, value] of defines) {
        result = result.replace(new RegExp(`\\b${key}\\b`, 'g'), value);
      }
      out.push(result);
    });
  };

  expand(resolve(entry), [], '<entry>');
  return out.join('\n');
}
