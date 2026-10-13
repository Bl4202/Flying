/**
 * Vite plugin: `import src from './shader.wgsl'` gives the preprocessed WGSL (includes expanded,
 * defines substituted) as a string, so the same expansion that `wgsl-check` validates and
 * `wgsl-gen` translates is what ships.
 */
import type { Plugin } from 'vite';
import { preprocess } from './wgsl-pp.ts';

export function wgsl(): Plugin {
  return {
    name: 'fg-wgsl',
    enforce: 'pre',
    load(id) {
      const file = id.split('?')[0] ?? id;
      if (!file.endsWith('.wgsl')) {
        return undefined;
      }
      return `export default ${JSON.stringify(preprocess(file))};`;
    },
  };
}
