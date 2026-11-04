import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { blocksWithBindings, differences, uniformBlocks, wgslUniformBindings } from './wgsl-gen.ts';

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

describe('wgslUniformBindings', () => {
  it('maps each uniform struct type to its binding', () => {
    const wgsl = `
struct Frame { v : vec4<f32>, };
struct TerrainLevel { v : vec4<f32>, };
@group(0) @binding(0) var<uniform> frame : Frame;
@group(0)
@binding(1) var<uniform>   level : TerrainLevel;
@group(1) @binding(0) var<storage> not_a_uniform : Frame;`;
    expect([...wgslUniformBindings(wgsl)]).toEqual([
      ['Frame', 0],
      ['TerrainLevel', 1],
    ]);
  });
  it('refuses a type that is bound twice', () => {
    expect(() =>
      wgslUniformBindings(
        '@group(0) @binding(0) var<uniform> a : Frame;\n@group(0) @binding(1) var<uniform> b : Frame;',
      ),
    ).toThrow(/bound twice/);
  });
});

describe('blocksWithBindings', () => {
  const bindings = new Map([
    ['Frame', 0],
    ['TerrainLevel', 1],
  ]);
  it('gives every block of a stage the binding of its WGSL variable', () => {
    const glsl = `layout(std140) uniform Frame_block_0Vertex { Frame _group_0_binding_0_vs; };
layout(std140) uniform TerrainLevel_block_1Vertex { TerrainLevel _group_0_binding_1_vs; };`;
    expect(blocksWithBindings(glsl, bindings)).toEqual([
      { name: 'Frame_block_0Vertex', binding: 0 },
      { name: 'TerrainLevel_block_1Vertex', binding: 1 },
    ]);
  });
  it('is empty for a stage without uniforms', () => {
    expect(blocksWithBindings('void main() {}', bindings)).toEqual([]);
  });
  it('names the block it cannot place', () => {
    expect(() =>
      blocksWithBindings('uniform Mystery_block_0Fragment { int a; };', bindings),
    ).toThrow(/Mystery_block_0Fragment/);
    expect(() => blocksWithBindings('uniform Odd { int a; };', bindings)).toThrow(/Odd/);
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
