import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { STRINGS, t, type StringKey } from './strings.ts';

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === 'node_modules' ? [] : sourceFiles(path);
    return /\.ts$/.test(name) && !/\.(test|contract)\.ts$|\.d\.ts$|strings\.ts$/.test(name)
      ? [path]
      : [];
  });
}

const ROOT = new URL('../../', import.meta.url).pathname;
const sources = [...sourceFiles(join(ROOT, 'ui/src')), ...sourceFiles(join(ROOT, 'app-shell/src'))]
  .map((path) => readFileSync(path, 'utf8'))
  .join('\n');

const keys = Object.keys(STRINGS) as StringKey[];
const namespaces = new Set(keys.map((k) => k.split('.')[0]));

describe('strings', () => {
  it('has English text for every key, and none of it empty', () => {
    for (const key of keys) {
      expect(STRINGS[key].trim(), key).not.toBe('');
    }
  });

  it('every key-shaped literal in the sources is a key of the table', () => {
    const literal = /'([a-z][a-z0-9-]*(?:\.[a-z0-9-]+)+)'/g;
    const unknown: string[] = [];
    for (const [, text] of sources.matchAll(literal)) {
      const ns = text?.split('.')[0];
      if (text !== undefined && ns !== undefined && namespaces.has(ns) && !(text in STRINGS)) {
        unknown.push(text);
      }
    }
    expect(unknown).toEqual([]);
  });

  it('every key is used, directly or through a template such as `compat.verdict.${v}`', () => {
    const prefixes = [...sources.matchAll(/`([a-z][a-z0-9.-]*\.)\$\{/g)].map((m) => m[1] ?? '');
    const unused = keys.filter(
      (k) => !sources.includes(`'${k}'`) && !prefixes.some((p) => k.startsWith(p)),
    );
    expect(unused).toEqual([]);
  });
});

describe('t', () => {
  it('returns the text as is when there are no slots', () => {
    expect(t('compat.continue')).toBe('Continue');
  });

  it('fills slots and leaves unknown ones visible', () => {
    expect(t('fatal.code', { code: 'SYS-103' })).toBe('Code SYS-103');
    expect(t('compat.remedy', {})).toBe('What to do: {text}');
  });

  it('fills numbers and repeated slots', () => {
    expect(t('toast.sim-silent', { seconds: 7 })).toBe(
      'The simulation has not reported for 7 seconds.',
    );
  });
});
