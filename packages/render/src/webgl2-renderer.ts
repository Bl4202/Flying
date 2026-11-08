/**
 * The WebGL2 backend (GDD 61.6, ADR-24): the fallback for browsers or machines without WebGPU, and
 * the path the Minimum class is expected to run.
 *
 * It draws the same frame data as the WebGPU backend with shaders translated from the same WGSL.
 * Clip space is kept as WebGPU defines it (depth 0..1) by asking for `EXT_clip_control`; without
 * that extension the scene is built for -1..1 instead (`zRange`), which still works but spends
 * depth precision less evenly. Either way depth is reverse-Z: compare `GREATER`, clear 0.
 *
 * A lost context is reported and, if the browser restores it, every resource is rebuilt.
 */
import { TERRAIN_LEVEL_COUNT } from '@flying/sim-bridge/terrain-levels';
import type { ZRange } from './camera.ts';
import {
  buildMarkerMesh,
  MARKER_INSTANCE_FLOATS,
  MARKER_VERTEX_COUNT,
  MARKER_VERTEX_FLOATS,
} from './markers.ts';
import {
  CLEAR_COLOUR,
  type Backend,
  type CanvasLike,
  type GlslProgram,
  type MeshSlot,
  RendererUnavailable,
  type Renderer,
  type SampleCount,
  type ShaderSources,
} from './renderer.ts';
import {
  FRAME_UNIFORM_BYTES,
  type FrameData,
  LEVEL_UNIFORM_BYTES,
  LEVEL_UNIFORM_FLOATS,
  MARKER_INSTANCE_CAPACITY,
} from './scene.ts';
import { HUD_UNIFORM_BYTES } from './hud/hud.ts';
import { MAX_SEGMENTS, SEGMENT_FLOATS } from './hud/graph.ts';
import { MESH_VERTEX_BYTES, MODEL_UNIFORM_BYTES, type ModelMesh } from './model-mesh.ts';
import { VERTEX_BYTES, type TerrainMesh } from './terrain-mesh.ts';

/** The one function and two constants this module uses from `EXT_clip_control`. */
interface ClipControlExt {
  clipControlEXT(origin: number, depth: number): void;
  readonly LOWER_LEFT_EXT: number;
  readonly ZERO_TO_ONE_EXT: number;
}

const INSTANCE_BYTES = MARKER_INSTANCE_FLOATS * 4;

/** What a terrain level has on the GPU: its geometry, and its uniform block. */
interface LevelGl {
  vao: WebGLVertexArrayObject;
  vbo: WebGLBuffer;
  ibo: WebGLBuffer;
  ubo: WebGLBuffer;
}

/** What an aircraft mesh has on the GPU. */
interface MeshGl {
  vao: WebGLVertexArrayObject;
  vbo: WebGLBuffer;
  ibo: WebGLBuffer;
  indexCount: number;
  bodyIndexCount: number;
}

interface Resources {
  hudProgram: WebGLProgram;
  hudUbo: WebGLBuffer;
  hudVao: WebGLVertexArrayObject;
  hudVbo: WebGLBuffer;
  skyProgram: WebGLProgram;
  meshProgram: WebGLProgram;
  modelUbo: WebGLBuffer;
  terrainProgram: WebGLProgram;
  markerProgram: WebGLProgram;
  fxProgram: WebGLProgram;
  /** For the draws whose corners come from the vertex index (sky, screen effect). */
  emptyVao: WebGLVertexArrayObject;
  ubo: WebGLBuffer;
  levels: LevelGl[];
  markerVao: WebGLVertexArrayObject;
  markerVbo: WebGLBuffer;
  instanceVbo: WebGLBuffer;
}

/** The multisampled target a frame is drawn into before it is resolved to the canvas. */
interface MsaaTarget {
  fbo: WebGLFramebuffer;
  colour: WebGLRenderbuffer;
  depth: WebGLRenderbuffer;
  width: number;
  height: number;
  samples: number;
}

function compile(
  gl: WebGL2RenderingContext,
  type: number,
  source: string,
  what: string,
): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error(`SYS-052: cannot create a ${what} shader`);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!(gl.getShaderParameter(shader, gl.COMPILE_STATUS) as boolean)) {
    const log = gl.getShaderInfoLog(shader) ?? '';
    gl.deleteShader(shader);
    throw new Error(`SYS-052: ${what} shader did not compile: ${log}`);
  }
  return shader;
}

function link(gl: WebGL2RenderingContext, glsl: GlslProgram, what: string): WebGLProgram {
  const program = gl.createProgram();
  const vs = compile(gl, gl.VERTEX_SHADER, glsl.vertex, `${what} vertex`);
  const fs = compile(gl, gl.FRAGMENT_SHADER, glsl.fragment, `${what} fragment`);
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (!(gl.getProgramParameter(program, gl.LINK_STATUS) as boolean)) {
    const log = gl.getProgramInfoLog(program) ?? '';
    gl.deleteProgram(program);
    throw new Error(`SYS-052: ${what} program did not link: ${log}`);
  }
  // Each block reads the binding point its WGSL variable declared.
  for (const [stage, blocks] of [
    ['vertex', glsl.blocks.vertex],
    ['fragment', glsl.blocks.fragment],
  ] as const) {
    for (const { name, binding } of blocks) {
      const index = gl.getUniformBlockIndex(program, name);
      if (index === gl.INVALID_INDEX) {
        throw new Error(`SYS-052: ${what} ${stage} shader has no uniform block named ${name}`);
      }
      gl.uniformBlockBinding(program, index, binding);
    }
  }
  return program;
}

export class WebGl2Renderer implements Renderer {
  readonly backend: Backend = 'webgl2';
  readonly zRange: ZRange;
  readonly description: string;

  readonly #canvas: CanvasLike;
  readonly #gl: WebGL2RenderingContext;
  readonly #shaders: ShaderSources;
  readonly #clip: ClipControlExt | null;
  readonly #lost: ((reason: string) => void)[] = [];
  readonly #maxSamples: number;
  #res: Resources | undefined;
  /** The terrain levels as last uploaded, to upload again if the context is restored. */
  readonly #meshes: (TerrainMesh | undefined)[] = Array.from(
    { length: TERRAIN_LEVEL_COUNT },
    () => undefined,
  );
  readonly #indexCounts: number[] = Array.from({ length: TERRAIN_LEVEL_COUNT }, () => 0);
  /** The indices of each level's surface alone, without its skirts. */
  readonly #surfaceCounts: number[] = Array.from({ length: TERRAIN_LEVEL_COUNT }, () => 0);
  /** The aircraft meshes as last given, to upload again if the context is restored. */
  readonly #meshSources = new Map<MeshSlot, ModelMesh>();
  readonly #meshGl = new Map<MeshSlot, MeshGl>();
  #samples: SampleCount = 1;
  #msaa: MsaaTarget | undefined;
  #last: FrameData | undefined;
  #width = 1;
  #height = 1;
  #contextLost = false;
  readonly #syncPixel = new Uint8Array(4);
  #disposed = false;

  private constructor(canvas: CanvasLike, gl: WebGL2RenderingContext, shaders: ShaderSources) {
    this.#canvas = canvas;
    this.#gl = gl;
    this.#shaders = shaders;
    this.#clip = gl.getExtension('EXT_clip_control') as ClipControlExt | null;
    this.zRange = this.#clip ? '0to1' : '-1to1';
    this.#maxSamples = Number(gl.getParameter(gl.MAX_SAMPLES));
    const debug = gl.getExtension('WEBGL_debug_renderer_info');
    this.description = String(gl.getParameter(debug ? debug.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
    this.#build();

    canvas.addEventListener('webglcontextlost', (event) => {
      event.preventDefault(); // ask the browser to try to restore it
      this.#contextLost = true;
      for (const l of this.#lost) l('WebGL2 context lost');
    });
    canvas.addEventListener('webglcontextrestored', () => {
      this.#contextLost = false;
      this.#msaa = undefined; // its objects died with the context
      this.#meshGl.clear();
      this.#build();
      for (const [slot, mesh] of this.#meshSources) this.setMesh(slot, mesh);
      this.#meshes.forEach((mesh, level) => {
        if (mesh) this.setTerrainLevel(level, mesh);
      });
      this.resize(this.#width, this.#height);
    });
  }

  /** Creates a renderer on `canvas`, or throws `RendererUnavailable`. */
  static create(canvas: CanvasLike, shaders: ShaderSources): WebGl2Renderer {
    // Typed as `unknown` first: what `getContext` is declared to return differs between the DOM
    // and worker typings (and between programs that include one or both), so any direct
    // assertion is "unnecessary" in some of them and necessary in others.
    const context: unknown = canvas.getContext('webgl2', {
      antialias: false,
      alpha: false,
      depth: true,
      stencil: false,
      powerPreference: 'high-performance',
      preserveDrawingBuffer: false,
    });
    const gl = context as WebGL2RenderingContext | null;
    if (!gl) {
      throw new RendererUnavailable('SYS-053: the canvas would not give a WebGL2 context', false);
    }
    try {
      return new WebGl2Renderer(canvas, gl, shaders);
    } catch (e) {
      throw new RendererUnavailable(`SYS-052: ${e instanceof Error ? e.message : String(e)}`, true);
    }
  }

  #build(): void {
    const gl = this.#gl;
    const need = <T>(v: T | null, what: string): T => {
      if (v === null) throw new Error(`SYS-052: cannot create ${what}`);
      return v;
    };
    const skyProgram = link(gl, this.#shaders.skyGlsl, 'sky');
    const hudProgram = link(gl, this.#shaders.hudGlsl, 'hud');
    const hudUbo = need(gl.createBuffer(), 'the HUD uniform buffer');
    gl.bindBuffer(gl.UNIFORM_BUFFER, hudUbo);
    gl.bufferData(gl.UNIFORM_BUFFER, HUD_UNIFORM_BYTES, gl.DYNAMIC_DRAW);
    // The HUD's segments: one instance each, two vec4s.
    const hudVao = need(gl.createVertexArray(), 'the HUD VAO');
    const hudVbo = need(gl.createBuffer(), 'the HUD segment buffer');
    gl.bindVertexArray(hudVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, hudVbo);
    gl.bufferData(gl.ARRAY_BUFFER, MAX_SEGMENTS * SEGMENT_FLOATS * 4, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 4, gl.FLOAT, false, SEGMENT_FLOATS * 4, 0);
    gl.vertexAttribDivisor(0, 1);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 4, gl.FLOAT, false, SEGMENT_FLOATS * 4, 16);
    gl.vertexAttribDivisor(1, 1);
    gl.bindVertexArray(null);
    const meshProgram = link(gl, this.#shaders.meshGlsl, 'aircraft mesh');
    const modelUbo = need(gl.createBuffer(), 'the model uniform buffer');
    gl.bindBuffer(gl.UNIFORM_BUFFER, modelUbo);
    gl.bufferData(gl.UNIFORM_BUFFER, MODEL_UNIFORM_BYTES, gl.DYNAMIC_DRAW);
    const terrainProgram = link(gl, this.#shaders.terrainGlsl, 'terrain');
    const markerProgram = link(gl, this.#shaders.markerGlsl, 'marker');
    const fxProgram = link(gl, this.#shaders.fxGlsl, 'screen fx');

    const ubo = need(gl.createBuffer(), 'the uniform buffer');
    gl.bindBuffer(gl.UNIFORM_BUFFER, ubo);
    gl.bufferData(gl.UNIFORM_BUFFER, FRAME_UNIFORM_BYTES, gl.DYNAMIC_DRAW);
    gl.bindBufferBase(gl.UNIFORM_BUFFER, 0, ubo);

    // Terrain levels: geometry filled by setTerrainLevel, one uniform block each.
    const levels: LevelGl[] = [];
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      const vao = need(gl.createVertexArray(), 'a terrain VAO');
      const vbo = need(gl.createBuffer(), 'a terrain vertex buffer');
      const ibo = need(gl.createBuffer(), 'a terrain index buffer');
      const levelUbo = need(gl.createBuffer(), 'a terrain level uniform buffer');
      gl.bindVertexArray(vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 3, gl.FLOAT, false, VERTEX_BYTES, 0);
      gl.enableVertexAttribArray(1);
      gl.vertexAttribPointer(1, 3, gl.FLOAT, false, VERTEX_BYTES, 12);
      gl.enableVertexAttribArray(2);
      gl.vertexAttribPointer(2, 1, gl.FLOAT, false, VERTEX_BYTES, 24);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ibo);
      gl.bindBuffer(gl.UNIFORM_BUFFER, levelUbo);
      gl.bufferData(gl.UNIFORM_BUFFER, LEVEL_UNIFORM_BYTES, gl.DYNAMIC_DRAW);
      levels.push({ vao, vbo, ibo, ubo: levelUbo });
    }

    // Markers: a static mesh and a per-instance buffer.
    const markerVao = need(gl.createVertexArray(), 'the marker VAO');
    const markerVbo = need(gl.createBuffer(), 'the marker vertex buffer');
    const instanceVbo = need(gl.createBuffer(), 'the marker instance buffer');
    gl.bindVertexArray(markerVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, markerVbo);
    gl.bufferData(gl.ARRAY_BUFFER, buildMarkerMesh(), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, MARKER_VERTEX_FLOATS * 4, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, MARKER_VERTEX_FLOATS * 4, 12);
    gl.bindBuffer(gl.ARRAY_BUFFER, instanceVbo);
    gl.bufferData(gl.ARRAY_BUFFER, MARKER_INSTANCE_CAPACITY * INSTANCE_BYTES, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 4, gl.FLOAT, false, INSTANCE_BYTES, 0);
    gl.vertexAttribDivisor(2, 1);
    gl.enableVertexAttribArray(3);
    gl.vertexAttribPointer(3, 4, gl.FLOAT, false, INSTANCE_BYTES, 16);
    gl.vertexAttribDivisor(3, 1);
    gl.enableVertexAttribArray(4);
    gl.vertexAttribPointer(4, 4, gl.FLOAT, false, INSTANCE_BYTES, 32);
    gl.vertexAttribDivisor(4, 1);
    gl.bindVertexArray(null);

    // The sky and screen-effect triangles have no vertex data: their corners come from the vertex index.
    const emptyVao = need(gl.createVertexArray(), 'the empty VAO');

    // State that never changes.
    if (this.#clip) {
      this.#clip.clipControlEXT(this.#clip.LOWER_LEFT_EXT, this.#clip.ZERO_TO_ONE_EXT);
    }
    gl.clearDepth(0); // reverse-Z
    gl.frontFace(gl.CCW);
    gl.clearColor(CLEAR_COLOUR[0], CLEAR_COLOUR[1], CLEAR_COLOUR[2], 1);

    this.#res = {
      hudProgram,
      hudUbo,
      hudVao,
      hudVbo,
      skyProgram,
      meshProgram,
      modelUbo,
      terrainProgram,
      markerProgram,
      fxProgram,
      emptyVao,
      ubo,
      levels,
      markerVao,
      markerVbo,
      instanceVbo,
    };
  }

  get sampleCount(): SampleCount {
    return this.#samples;
  }

  setSampleCount(count: SampleCount): SampleCount {
    this.#samples = count === 4 && this.#maxSamples >= 4 ? 4 : 1;
    return this.#samples;
  }

  resize(widthPx: number, heightPx: number): void {
    this.#width = Math.max(1, Math.floor(widthPx));
    this.#height = Math.max(1, Math.floor(heightPx));
    this.#canvas.width = this.#width;
    this.#canvas.height = this.#height;
    this.#gl.viewport(0, 0, this.#width, this.#height);
  }

  setMesh(slot: MeshSlot, mesh: ModelMesh | undefined): void {
    if (mesh === undefined) this.#meshSources.delete(slot);
    else this.#meshSources.set(slot, mesh);
    const gl = this.#gl;
    const old = this.#meshGl.get(slot);
    if (old && !this.#contextLost) {
      gl.deleteBuffer(old.vbo);
      gl.deleteBuffer(old.ibo);
      gl.deleteVertexArray(old.vao);
    }
    this.#meshGl.delete(slot);
    if (mesh === undefined || !this.#res || this.#contextLost) return;
    const vao = gl.createVertexArray();
    const vbo = gl.createBuffer();
    const ibo = gl.createBuffer();
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    gl.bufferData(gl.ARRAY_BUFFER, mesh.vertices, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, MESH_VERTEX_BYTES, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, MESH_VERTEX_BYTES, 12);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 2, gl.FLOAT, false, MESH_VERTEX_BYTES, 24);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ibo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.indices, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    this.#meshGl.set(slot, {
      vao,
      vbo,
      ibo,
      indexCount: mesh.indices.length,
      bodyIndexCount: mesh.bodyIndexCount,
    });
  }

  setTerrainLevel(level: number, mesh: TerrainMesh): void {
    this.#meshes[level] = mesh;
    const res = this.#res;
    const slot = res?.levels[level];
    if (!res || !slot || this.#contextLost) return;
    const gl = this.#gl;
    gl.bindVertexArray(slot.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, slot.vbo);
    gl.bufferData(gl.ARRAY_BUFFER, mesh.vertices, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, slot.ibo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.indices, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    this.#indexCounts[level] = mesh.indices.length;
    this.#surfaceCounts[level] = mesh.surfaceTriangleCount * 3;
  }

  /** The multisampled framebuffer for the current size and sample count, made when needed. */
  #target(): MsaaTarget {
    const gl = this.#gl;
    const m = this.#msaa;
    if (m && m.width === this.#width && m.height === this.#height && m.samples === this.#samples) {
      return m;
    }
    if (m) {
      gl.deleteFramebuffer(m.fbo);
      gl.deleteRenderbuffer(m.colour);
      gl.deleteRenderbuffer(m.depth);
    }
    const fbo = gl.createFramebuffer();
    const colour = gl.createRenderbuffer();
    const depth = gl.createRenderbuffer();
    gl.bindRenderbuffer(gl.RENDERBUFFER, colour);
    gl.renderbufferStorageMultisample(
      gl.RENDERBUFFER,
      this.#samples,
      gl.RGBA8,
      this.#width,
      this.#height,
    );
    gl.bindRenderbuffer(gl.RENDERBUFFER, depth);
    gl.renderbufferStorageMultisample(
      gl.RENDERBUFFER,
      this.#samples,
      gl.DEPTH_COMPONENT32F,
      this.#width,
      this.#height,
    );
    gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.RENDERBUFFER, colour);
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, depth);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    this.#msaa = {
      fbo,
      colour,
      depth,
      width: this.#width,
      height: this.#height,
      samples: this.#samples,
    };
    return this.#msaa;
  }

  render(frame: FrameData): void {
    if (this.#disposed || this.#contextLost) return;
    this.#last = frame;
    this.#draw(frame);
  }

  #draw(frame: FrameData): void {
    const res = this.#res;
    if (!res) return;
    const gl = this.#gl;
    const target = this.#samples > 1 ? this.#target() : undefined;
    gl.bindFramebuffer(gl.FRAMEBUFFER, target?.fbo ?? null);
    gl.viewport(0, 0, this.#width, this.#height);

    gl.bindBuffer(gl.UNIFORM_BUFFER, res.ubo);
    gl.bufferSubData(gl.UNIFORM_BUFFER, 0, frame.uniform);
    gl.bindBufferBase(gl.UNIFORM_BUFFER, 0, res.ubo);
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      if ((frame.levelMask & (1 << level)) === 0) continue;
      gl.bindBuffer(gl.UNIFORM_BUFFER, (res.levels[level] as LevelGl).ubo);
      gl.bufferSubData(
        gl.UNIFORM_BUFFER,
        0,
        frame.levelUniform,
        level * LEVEL_UNIFORM_FLOATS,
        LEVEL_UNIFORM_FLOATS,
      );
    }

    gl.depthMask(true);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    if (frame.sky) {
      // One triangle over the whole screen, with no depth test or write, drawn first.
      gl.disable(gl.DEPTH_TEST);
      gl.disable(gl.CULL_FACE);
      gl.useProgram(res.skyProgram);
      gl.bindVertexArray(res.emptyVao);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.GREATER); // reverse-Z
    gl.enable(gl.CULL_FACE);
    gl.cullFace(gl.BACK);
    // Finest level first: its fragments are the nearest, so the depth test rejects the coarser
    // levels' hidden ones early.
    gl.useProgram(res.terrainProgram);
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      const slot = res.levels[level] as LevelGl;
      const count = (frame.noSkirts ? this.#surfaceCounts[level] : this.#indexCounts[level]) ?? 0;
      if ((frame.levelMask & (1 << level)) === 0 || count === 0) continue;
      gl.bindBufferBase(gl.UNIFORM_BUFFER, 1, slot.ubo);
      gl.bindVertexArray(slot.vao);
      gl.drawElementsInstanced(
        gl.TRIANGLES,
        count,
        gl.UNSIGNED_SHORT,
        0,
        level === 0 ? frame.patchInstances : 1,
      );
    }
    if (frame.model) {
      gl.bindBuffer(gl.UNIFORM_BUFFER, res.modelUbo);
      gl.bufferSubData(gl.UNIFORM_BUFFER, 0, frame.modelUniform);
      gl.bindBufferBase(gl.UNIFORM_BUFFER, 2, res.modelUbo);
      gl.useProgram(res.meshProgram);
      const exterior = this.#meshGl.get('exterior');
      const interior = this.#meshGl.get('cockpit');
      if (exterior) {
        gl.bindVertexArray(exterior.vao);
        gl.drawElements(
          gl.TRIANGLES,
          frame.hideCanopy ? exterior.bodyIndexCount : exterior.indexCount,
          gl.UNSIGNED_SHORT,
          0,
        );
      }
      if (interior && frame.cockpit) {
        gl.bindVertexArray(interior.vao);
        gl.drawElements(gl.TRIANGLES, interior.indexCount, gl.UNSIGNED_SHORT, 0);
      }
    }
    if (frame.instanceCount > 0) {
      gl.bindBuffer(gl.ARRAY_BUFFER, res.instanceVbo);
      gl.bufferSubData(
        gl.ARRAY_BUFFER,
        0,
        frame.instances,
        0,
        frame.instanceCount * MARKER_INSTANCE_FLOATS,
      );
      gl.useProgram(res.markerProgram);
      gl.bindVertexArray(res.markerVao);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, MARKER_VERTEX_COUNT, frame.instanceCount);
    }
    if (frame.hud && frame.hudCount > 0) {
      // The HUD: capsules of four vertices, one instance a segment, alpha blended, no depth.
      gl.bindBuffer(gl.UNIFORM_BUFFER, res.hudUbo);
      gl.bufferSubData(gl.UNIFORM_BUFFER, 0, frame.hudUniform);
      gl.bindBufferBase(gl.UNIFORM_BUFFER, 3, res.hudUbo);
      gl.bindBuffer(gl.ARRAY_BUFFER, res.hudVbo);
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, frame.hudSegments, 0, frame.hudCount * SEGMENT_FLOATS);
      gl.disable(gl.DEPTH_TEST);
      gl.disable(gl.CULL_FACE);
      gl.enable(gl.BLEND);
      gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.useProgram(res.hudProgram);
      gl.bindVertexArray(res.hudVao);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, frame.hudCount);
      gl.disable(gl.BLEND);
      gl.enable(gl.DEPTH_TEST);
      gl.enable(gl.CULL_FACE);
    }
    if (frame.fx) {
      // Over the finished picture, alpha blended, with no depth test and no culling.
      gl.disable(gl.DEPTH_TEST);
      gl.disable(gl.CULL_FACE);
      gl.enable(gl.BLEND);
      gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.useProgram(res.fxProgram);
      gl.bindVertexArray(res.emptyVao);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      gl.disable(gl.BLEND);
    }
    gl.bindVertexArray(null);
    if (target) {
      // Resolve the samples into the canvas.
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, target.fbo);
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
      gl.blitFramebuffer(
        0,
        0,
        this.#width,
        this.#height,
        0,
        0,
        this.#width,
        this.#height,
        gl.COLOR_BUFFER_BIT,
        gl.NEAREST,
      );
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  }

  finish(): Promise<void> {
    if (!this.#disposed && !this.#contextLost) {
      const gl = this.#gl;
      gl.finish();
      // `finish` alone does not always wait (it does not in Chrome's GPU process model); reading
      // one pixel back cannot return before every earlier draw has been executed.
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, this.#syncPixel);
    }
    return Promise.resolve();
  }

  debugReadPixels(): Promise<Uint8Array> {
    const w = this.#width;
    const h = this.#height;
    const out = new Uint8Array(w * h * 4);
    if (this.#disposed || this.#contextLost || this.#last === undefined) {
      return Promise.resolve(out);
    }
    // The drawing buffer is only guaranteed to hold the picture until the task ends, so draw it
    // again and read it back immediately.
    this.#draw(this.#last);
    const gl = this.#gl;
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    const raw = new Uint8Array(w * h * 4);
    gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, raw);
    // GL rows run bottom to top; the contract is top row first.
    for (let y = 0; y < h; y++) {
      out.set(raw.subarray((h - 1 - y) * w * 4, (h - y) * w * 4), y * w * 4);
    }
    return Promise.resolve(out);
  }

  onLost(listener: (reason: string) => void): void {
    this.#lost.push(listener);
  }

  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    const res = this.#res;
    if (res && !this.#contextLost) {
      const gl = this.#gl;
      gl.deleteBuffer(res.hudUbo);
      gl.deleteBuffer(res.hudVbo);
      gl.deleteVertexArray(res.hudVao);
      for (const p of [
        res.hudProgram,
        res.skyProgram,
        res.meshProgram,
        res.terrainProgram,
        res.markerProgram,
        res.fxProgram,
      ]) {
        gl.deleteProgram(p);
      }
      gl.deleteBuffer(res.modelUbo);
      for (const m of this.#meshGl.values()) {
        gl.deleteBuffer(m.vbo);
        gl.deleteBuffer(m.ibo);
        gl.deleteVertexArray(m.vao);
      }
      gl.deleteVertexArray(res.emptyVao);
      for (const b of [res.ubo, res.markerVbo, res.instanceVbo]) gl.deleteBuffer(b);
      for (const slot of res.levels) {
        gl.deleteBuffer(slot.vbo);
        gl.deleteBuffer(slot.ibo);
        gl.deleteBuffer(slot.ubo);
        gl.deleteVertexArray(slot.vao);
      }
      gl.deleteVertexArray(res.markerVao);
      const m = this.#msaa;
      if (m) {
        gl.deleteFramebuffer(m.fbo);
        gl.deleteRenderbuffer(m.colour);
        gl.deleteRenderbuffer(m.depth);
      }
    }
    this.#res = undefined;
    this.#msaa = undefined;
    this.#gl.getExtension('WEBGL_lose_context')?.loseContext();
  }
}
