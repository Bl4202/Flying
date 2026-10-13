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
  RendererUnavailable,
  type Renderer,
  type ShaderSources,
} from './renderer.ts';
import { FRAME_UNIFORM_BYTES, type FrameData } from './scene.ts';
import { VERTEX_BYTES, type TerrainMesh } from './terrain-mesh.ts';
import { SNAPSHOT } from '@flying/sim-bridge/layout';

/** The one function and two constants this module uses from `EXT_clip_control`. */
interface ClipControlExt {
  clipControlEXT(origin: number, depth: number): void;
  readonly LOWER_LEFT_EXT: number;
  readonly ZERO_TO_ONE_EXT: number;
}

const INSTANCE_BYTES = MARKER_INSTANCE_FLOATS * 4;

interface Resources {
  terrainProgram: WebGLProgram;
  markerProgram: WebGLProgram;
  ubo: WebGLBuffer;
  terrainVao: WebGLVertexArrayObject;
  terrainVbo: WebGLBuffer;
  terrainIbo: WebGLBuffer;
  markerVao: WebGLVertexArrayObject;
  markerVbo: WebGLBuffer;
  instanceVbo: WebGLBuffer;
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
  // Both stages' Frame blocks read uniform binding 0.
  for (const [stage, name] of [
    ['vertex', glsl.blocks.vertex],
    ['fragment', glsl.blocks.fragment],
  ] as const) {
    const index = gl.getUniformBlockIndex(program, name);
    if (index === gl.INVALID_INDEX) {
      throw new Error(`SYS-052: ${what} ${stage} shader has no uniform block named ${name}`);
    }
    gl.uniformBlockBinding(program, index, 0);
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
  #res: Resources | undefined;
  #terrain: TerrainMesh | undefined;
  #terrainIndexCount = 0;
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
      this.#build();
      if (this.#terrain) this.setTerrainPatch(this.#terrain);
      this.resize(this.#width, this.#height);
    });
  }

  /** Creates a renderer on `canvas`, or throws `RendererUnavailable`. */
  static create(canvas: CanvasLike, shaders: ShaderSources): WebGl2Renderer {
    const gl = canvas.getContext('webgl2', {
      antialias: false,
      alpha: false,
      depth: true,
      stencil: false,
      powerPreference: 'high-performance',
      preserveDrawingBuffer: false,
    }) as WebGL2RenderingContext | null;
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
    const terrainProgram = link(gl, this.#shaders.terrainGlsl, 'terrain');
    const markerProgram = link(gl, this.#shaders.markerGlsl, 'marker');

    const ubo = need(gl.createBuffer(), 'the uniform buffer');
    gl.bindBuffer(gl.UNIFORM_BUFFER, ubo);
    gl.bufferData(gl.UNIFORM_BUFFER, FRAME_UNIFORM_BYTES, gl.DYNAMIC_DRAW);
    gl.bindBufferBase(gl.UNIFORM_BUFFER, 0, ubo);

    // Terrain: filled by setTerrainPatch.
    const terrainVao = need(gl.createVertexArray(), 'the terrain VAO');
    const terrainVbo = need(gl.createBuffer(), 'the terrain vertex buffer');
    const terrainIbo = need(gl.createBuffer(), 'the terrain index buffer');
    gl.bindVertexArray(terrainVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, terrainVbo);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, VERTEX_BYTES, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, VERTEX_BYTES, 12);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 1, gl.FLOAT, false, VERTEX_BYTES, 24);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, terrainIbo);

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
    gl.bufferData(gl.ARRAY_BUFFER, SNAPSHOT.BODY_CAPACITY * INSTANCE_BYTES, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 4, gl.FLOAT, false, INSTANCE_BYTES, 0);
    gl.vertexAttribDivisor(2, 1);
    gl.enableVertexAttribArray(3);
    gl.vertexAttribPointer(3, 4, gl.FLOAT, false, INSTANCE_BYTES, 16);
    gl.vertexAttribDivisor(3, 1);
    gl.bindVertexArray(null);

    // State that never changes.
    if (this.#clip) {
      this.#clip.clipControlEXT(this.#clip.LOWER_LEFT_EXT, this.#clip.ZERO_TO_ONE_EXT);
    }
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.GREATER); // reverse-Z
    gl.clearDepth(0);
    gl.enable(gl.CULL_FACE);
    gl.cullFace(gl.BACK);
    gl.frontFace(gl.CCW);
    gl.clearColor(CLEAR_COLOUR[0], CLEAR_COLOUR[1], CLEAR_COLOUR[2], 1);

    this.#res = {
      terrainProgram,
      markerProgram,
      ubo,
      terrainVao,
      terrainVbo,
      terrainIbo,
      markerVao,
      markerVbo,
      instanceVbo,
    };
  }

  resize(widthPx: number, heightPx: number): void {
    this.#width = Math.max(1, Math.floor(widthPx));
    this.#height = Math.max(1, Math.floor(heightPx));
    this.#canvas.width = this.#width;
    this.#canvas.height = this.#height;
    this.#gl.viewport(0, 0, this.#width, this.#height);
  }

  setTerrainPatch(mesh: TerrainMesh): void {
    this.#terrain = mesh;
    const res = this.#res;
    if (!res || this.#contextLost) return;
    const gl = this.#gl;
    gl.bindVertexArray(res.terrainVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, res.terrainVbo);
    gl.bufferData(gl.ARRAY_BUFFER, mesh.vertices, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, res.terrainIbo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.indices, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    this.#terrainIndexCount = mesh.indices.length;
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
    gl.bindBuffer(gl.UNIFORM_BUFFER, res.ubo);
    gl.bufferSubData(gl.UNIFORM_BUFFER, 0, frame.uniform);
    gl.bindBufferBase(gl.UNIFORM_BUFFER, 0, res.ubo);

    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    if (this.#terrainIndexCount > 0) {
      gl.useProgram(res.terrainProgram);
      gl.bindVertexArray(res.terrainVao);
      gl.drawElementsInstanced(
        gl.TRIANGLES,
        this.#terrainIndexCount,
        gl.UNSIGNED_SHORT,
        0,
        frame.patchInstances,
      );
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
    gl.bindVertexArray(null);
  }

  finish(): Promise<void> {
    if (!this.#disposed && !this.#contextLost) {
      const gl = this.#gl;
      gl.finish();
      // `finish` alone does not always wait (it does not in Chrome's GPU process model); reading
      // one pixel back cannot return before every earlier draw has been executed.
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
      gl.deleteProgram(res.terrainProgram);
      gl.deleteProgram(res.markerProgram);
      for (const b of [res.ubo, res.terrainVbo, res.terrainIbo, res.markerVbo, res.instanceVbo]) {
        gl.deleteBuffer(b);
      }
      gl.deleteVertexArray(res.terrainVao);
      gl.deleteVertexArray(res.markerVao);
    }
    this.#res = undefined;
    this.#gl.getExtension('WEBGL_lose_context')?.loseContext();
  }
}
