import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { differences, uniformBlocks } from './wgsl-gen.ts';

describe('uniformBlocks', () => {
  it('finds the block name naga gave each stage', () => {
    const glsl = `#version 300 es
layout(std140) uniform Frame_block_0Vertex { Frame _group_0_binding_0_vs; };
uniform uint naga_vs_first_instance;
layout(std140) uniform Other_block { vec4 v; };`;
    expect(uniformBlocks(glsl)).toEqual(['Frame_block_0Vertex', 'Other_block']);
  });
  it('finds none in a shader without uniforms', () => {
    expect(uniformBlocks('void main() {}')).toEqual([]);
  });
});

describe('differences', () => {
  it('lists files that differ, are missing, or are extra, in sorted order', () => {
    const root = mkdtempSync(join(tmpdir(), 'fg-diff-'));
    try {
      const [a, b] = [join(root, 'a'), join(root, 'b')];
      mkdirSync(a);
      mkdirSync(b);
      writeFileSync(join(a, 'same'), 'x');
      writeFileSync(join(b, 'same'), 'x');
      writeFileSync(join(a, 'changed'), '1');
      writeFileSync(join(b, 'changed'), '2');
      writeFileSync(join(a, 'only-a'), '');
      writeFileSync(join(b, 'only-b'), '');
      expect(differences(a, b)).toEqual(['changed', 'only-a', 'only-b']);
      expect(differences(a, join(root, 'missing'))).toEqual(['changed', 'only-a', 'same']);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
