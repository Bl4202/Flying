/**
 * The shaders as the bundler delivers them: WGSL preprocessed by the Vite plugin, GLSL generated
 * from it and committed. Importing this file needs that plugin, so tests that do not draw avoid it.
 */
import markerWgsl from '../shaders/marker.wgsl';
import terrainWgsl from '../shaders/terrain_patch.wgsl';
import markerFrag from '../shaders/gen/marker.frag.glsl?raw';
import markerVert from '../shaders/gen/marker.vert.glsl?raw';
import glslManifest from '../shaders/gen/manifest.json';
import terrainFrag from '../shaders/gen/terrain_patch.frag.glsl?raw';
import terrainVert from '../shaders/gen/terrain_patch.vert.glsl?raw';
import type { ShaderSources } from './renderer.ts';

export const SHADERS: ShaderSources = {
  terrainWgsl,
  markerWgsl,
  terrainGlsl: {
    vertex: terrainVert,
    fragment: terrainFrag,
    blocks: glslManifest.terrain_patch.blocks,
  },
  markerGlsl: { vertex: markerVert, fragment: markerFrag, blocks: glslManifest.marker.blocks },
};
