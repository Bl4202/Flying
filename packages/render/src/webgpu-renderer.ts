/**
 * The WebGPU backend (GDD 61.6, ADR-24): raw WebGPU, no library.
 *
 * Creation order matters. The adapter and device are obtained *before* the canvas is touched,
 * because a canvas can hold only one kind of context for its whole life: if WebGPU is unavailable
 * the canvas is still unspent and WebGL2 can use it. Depth is `depth32float`, reverse-Z (compare
 * `greater`, clear 0), and positions arrive eye-relative from the CPU, so float precision is spent
 * near the camera.
 *
 * Pipelines are built with `createRenderPipelineAsync`, so a slow shader compile does not stall
 * the thread.
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
  RendererUnavailable,
  type Backend,
  type CanvasLike,
  type MeshSlot,
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
import { MESH_VERTEX_BYTES, MODEL_UNIFORM_BYTES, type ModelMesh } from './model-mesh.ts';
import { VERTEX_BYTES, type TerrainMesh } from './terrain-mesh.ts';

const DEPTH_FORMAT = 'depth32float';
const INSTANCE_BYTES = MARKER_INSTANCE_FLOATS * 4;

/** The pipelines for one sample count (a pipeline is made for the sample count it draws into). */
interface Pipelines {
  sky: GPURenderPipeline;
  mesh: GPURenderPipeline;
  terrain: GPURenderPipeline;
  marker: GPURenderPipeline;
  fx: GPURenderPipeline;
}

/** What an aircraft mesh has on the GPU. */
interface MeshGpu {
  vertices: GPUBuffer;
  indices: GPUBuffer;
  indexCount: number;
  bodyIndexCount: number;
}

/** What a terrain level has on the GPU. */
interface LevelGpu {
  vertices: GPUBuffer;
  indices: GPUBuffer;
  indexCount: number;
}

export class WebGpuRenderer implements Renderer {
  readonly backend: Backend = 'webgpu';
  readonly zRange: ZRange = '0to1';
  readonly description: string;

  readonly #canvas: CanvasLike;
  readonly #device: GPUDevice;
  readonly #context: GPUCanvasContext;
  readonly #format: GPUTextureFormat;
  readonly #uniform: GPUBuffer;
  /** The aircraft's part matrices and glow (binding 2 of every bind group). */
  readonly #modelUniform: GPUBuffer;
  readonly #meshes = new Map<MeshSlot, MeshGpu>();
  /** Per terrain level: the level's uniform buffer, and a bind group of it with the frame's. */
  readonly #levelUniforms: GPUBuffer[] = [];
  readonly #bindGroups: GPUBindGroup[] = [];
  readonly #pipelines: Record<SampleCount, Pipelines>;
  readonly #markerVertices: GPUBuffer;
  readonly #markerInstances: GPUBuffer;
  readonly #lost: ((reason: string) => void)[] = [];
  readonly #levels: (LevelGpu | undefined)[] = Array.from(
    { length: TERRAIN_LEVEL_COUNT },
    () => undefined,
  );
  #samples: SampleCount = 1;
  #depth: GPUTexture | undefined;
  /** The multisampled colour target, resolved into the swapchain (only with 4x MSAA). */
  #msaa: GPUTexture | undefined;
  #last: FrameData | undefined;
  #width = 1;
  #height = 1;
  #disposed = false;

  private constructor(args: {
    canvas: CanvasLike;
    device: GPUDevice;
    context: GPUCanvasContext;
    format: GPUTextureFormat;
    description: string;
    pipelines: Record<SampleCount, Pipelines>;
    layout: GPUBindGroupLayout;
  }) {
    this.#canvas = args.canvas;
    this.#device = args.device;
    this.#context = args.context;
    this.#format = args.format;
    this.description = args.description;
    this.#pipelines = args.pipelines;

    const device = args.device;
    this.#uniform = device.createBuffer({
      label: 'frame uniform',
      size: FRAME_UNIFORM_BYTES,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.#modelUniform = device.createBuffer({
      label: 'model uniform',
      size: MODEL_UNIFORM_BYTES,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      const buffer = device.createBuffer({
        label: `terrain level ${String(level)} uniform`,
        size: LEVEL_UNIFORM_BYTES,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
      this.#levelUniforms.push(buffer);
      this.#bindGroups.push(
        device.createBindGroup({
          layout: args.layout,
          entries: [
            { binding: 0, resource: { buffer: this.#uniform } },
            { binding: 1, resource: { buffer } },
            { binding: 2, resource: { buffer: this.#modelUniform } },
          ],
        }),
      );
    }
    const mesh = buildMarkerMesh();
    this.#markerVertices = device.createBuffer({
      label: 'marker mesh',
      size: mesh.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(this.#markerVertices, 0, mesh);
    this.#markerInstances = device.createBuffer({
      label: 'marker instances',
      size: MARKER_INSTANCE_CAPACITY * INSTANCE_BYTES,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });

    void device.lost.then((info) => {
      if (!this.#disposed) {
        for (const l of this.#lost) l(`WebGPU device lost (${info.reason}): ${info.message}`);
      }
    });
    device.addEventListener('uncapturederror', (event) => {
      console.error('WebGPU error:', event.error.message);
    });
  }

  /**
   * Creates a renderer on `canvas`. Rejects with `RendererUnavailable`; if `canvasTouched` is
   * false the canvas is still unspent.
   */
  static async create(
    canvas: CanvasLike,
    shaders: ShaderSources,
    gpu: GPU | undefined = (globalThis as { navigator?: { gpu?: GPU } }).navigator?.gpu,
  ): Promise<WebGpuRenderer> {
    if (!gpu) {
      throw new RendererUnavailable('SYS-050: WebGPU is not available', false);
    }
    // The adapter and the device come first and leave the canvas alone, so failing here costs
    // nothing: the same canvas can still go to WebGL2.
    let device: GPUDevice;
    let adapter: GPUAdapter;
    try {
      const found = await gpu.requestAdapter({ powerPreference: 'high-performance' });
      if (!found) {
        throw new Error('no WebGPU adapter');
      }
      adapter = found;
      device = await adapter.requestDevice();
    } catch (e) {
      throw new RendererUnavailable(
        `SYS-050: ${e instanceof Error ? e.message : String(e)}`,
        false,
      );
    }
    // Only now does the canvas commit to a WebGPU context.
    const raw: unknown = canvas.getContext('webgpu');
    const context = raw as GPUCanvasContext | null;
    if (!context) {
      device.destroy();
      throw new RendererUnavailable('SYS-051: the canvas would not give a WebGPU context', false);
    }
    try {
      return await WebGpuRenderer.#finish(canvas, context, device, adapter, shaders, gpu);
    } catch (e) {
      device.destroy();
      throw new RendererUnavailable(`SYS-051: ${e instanceof Error ? e.message : String(e)}`, true);
    }
  }

  static async #finish(
    canvas: CanvasLike,
    context: GPUCanvasContext,
    device: GPUDevice,
    adapter: GPUAdapter,
    shaders: ShaderSources,
    gpu: GPU,
  ): Promise<WebGpuRenderer> {
    const format = gpu.getPreferredCanvasFormat();
    context.configure({
      device,
      format,
      alphaMode: 'opaque',
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });

    // Binding 0 is the frame; binding 1 is the terrain level and binding 2 the aircraft's part
    // matrices (only those shaders read them, but one layout serves every pipeline).
    const layout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: 'uniform' },
        },
        {
          binding: 1,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: 'uniform' },
        },
        {
          binding: 2,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: 'uniform' },
        },
      ],
    });
    const modules = {
      mesh: device.createShaderModule({ label: 'aircraft mesh', code: shaders.meshWgsl }),
      sky: device.createShaderModule({ label: 'sky', code: shaders.skyWgsl }),
      terrain: device.createShaderModule({ label: 'terrain patch', code: shaders.terrainWgsl }),
      marker: device.createShaderModule({ label: 'marker', code: shaders.markerWgsl }),
      fx: device.createShaderModule({ label: 'screen fx', code: shaders.fxWgsl }),
    };
    const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout] });
    // Pipelines are made for both sample counts up front, so switching MSAA never stalls a frame.
    const [one, four] = await Promise.all([
      buildPipelines(device, format, modules, pipelineLayout, 1),
      buildPipelines(device, format, modules, pipelineLayout, 4),
    ]);

    const info = adapter.info;
    const description =
      [info.vendor, info.architecture, info.device, info.description]
        .filter((s) => s !== '')
        .join(' ') || 'WebGPU adapter';
    return new WebGpuRenderer({
      canvas,
      device,
      context,
      format,
      description,
      pipelines: { 1: one, 4: four },
      layout,
    });
  }

  get sampleCount(): SampleCount {
    return this.#samples;
  }

  setSampleCount(count: SampleCount): SampleCount {
    if (count !== this.#samples) {
      this.#samples = count;
      this.#depth?.destroy();
      this.#msaa?.destroy();
      this.#depth = undefined;
      this.#msaa = undefined;
    }
    return this.#samples;
  }

  resize(widthPx: number, heightPx: number): void {
    const w = Math.max(1, Math.floor(widthPx));
    const h = Math.max(1, Math.floor(heightPx));
    this.#width = w;
    this.#height = h;
    this.#canvas.width = w;
    this.#canvas.height = h;
    this.#makeTargets();
  }

  /** The depth buffer, and the multisampled colour target with 4x MSAA, for the current size. */
  #makeTargets(): void {
    this.#depth?.destroy();
    this.#msaa?.destroy();
    this.#depth = this.#device.createTexture({
      label: 'depth',
      size: [this.#width, this.#height],
      format: DEPTH_FORMAT,
      sampleCount: this.#samples,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
    this.#msaa =
      this.#samples > 1
        ? this.#device.createTexture({
            label: 'colour (multisampled)',
            size: [this.#width, this.#height],
            format: this.#format,
            sampleCount: this.#samples,
            usage: GPUTextureUsage.RENDER_ATTACHMENT,
          })
        : undefined;
  }

  setMesh(slot: MeshSlot, mesh: ModelMesh | undefined): void {
    const old = this.#meshes.get(slot);
    old?.vertices.destroy();
    old?.indices.destroy();
    this.#meshes.delete(slot);
    if (mesh === undefined) return;
    const device = this.#device;
    const vertices = device.createBuffer({
      label: `${slot} vertices`,
      size: mesh.vertices.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(vertices, 0, mesh.vertices);
    const indexBytes = Math.ceil(mesh.indices.byteLength / 4) * 4;
    const indices = device.createBuffer({
      label: `${slot} indices`,
      size: indexBytes,
      usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
    });
    const padded = new Uint16Array(indexBytes / 2);
    padded.set(mesh.indices);
    device.queue.writeBuffer(indices, 0, padded);
    this.#meshes.set(slot, {
      vertices,
      indices,
      indexCount: mesh.indices.length,
      bodyIndexCount: mesh.bodyIndexCount,
    });
  }

  setTerrainLevel(level: number, mesh: TerrainMesh): void {
    const device = this.#device;
    const old = this.#levels[level];
    old?.vertices.destroy();
    old?.indices.destroy();
    const vertices = device.createBuffer({
      label: `terrain level ${String(level)} vertices`,
      size: mesh.vertices.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(vertices, 0, mesh.vertices);
    // Buffer writes must be a multiple of four bytes; 16-bit indices may not be.
    const indexBytes = Math.ceil(mesh.indices.byteLength / 4) * 4;
    const indices = device.createBuffer({
      label: `terrain level ${String(level)} indices`,
      size: indexBytes,
      usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
    });
    const padded = new Uint16Array(indexBytes / 2);
    padded.set(mesh.indices);
    device.queue.writeBuffer(indices, 0, padded);
    this.#levels[level] = { vertices, indices, indexCount: mesh.indices.length };
  }

  render(frame: FrameData): void {
    if (this.#disposed) return;
    this.#last = frame;
    const encoder = this.#device.createCommandEncoder();
    this.#draw(encoder, this.#context.getCurrentTexture().createView(), frame);
    this.#device.queue.submit([encoder.finish()]);
  }

  #draw(encoder: GPUCommandEncoder, target: GPUTextureView, frame: FrameData): void {
    if (this.#depth === undefined || (this.#samples > 1 && this.#msaa === undefined)) {
      this.#makeTargets();
    }
    const queue = this.#device.queue;
    queue.writeBuffer(this.#uniform, 0, frame.uniform);
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      if ((frame.levelMask & (1 << level)) !== 0) {
        queue.writeBuffer(
          this.#levelUniforms[level] as GPUBuffer,
          0,
          frame.levelUniform,
          level * LEVEL_UNIFORM_FLOATS,
          LEVEL_UNIFORM_FLOATS,
        );
      }
    }
    if (frame.model) queue.writeBuffer(this.#modelUniform, 0, frame.modelUniform);
    if (frame.instanceCount > 0) {
      queue.writeBuffer(
        this.#markerInstances,
        0,
        frame.instances,
        0,
        frame.instanceCount * MARKER_INSTANCE_FLOATS,
      );
    }
    const msaa = this.#msaa;
    const clearValue = { r: CLEAR_COLOUR[0], g: CLEAR_COLOUR[1], b: CLEAR_COLOUR[2], a: 1 };
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        msaa
          ? {
              view: msaa.createView(),
              resolveTarget: target,
              clearValue,
              loadOp: 'clear',
              storeOp: 'discard',
            }
          : { view: target, clearValue, loadOp: 'clear', storeOp: 'store' },
      ],
      depthStencilAttachment: {
        view: (this.#depth as GPUTexture).createView(),
        depthClearValue: 0, // reverse-Z: far is 0
        depthLoadOp: 'clear',
        depthStoreOp: 'discard',
      },
    });
    const pipelines = this.#pipelines[this.#samples];
    pass.setBindGroup(0, this.#bindGroups[0] as GPUBindGroup);
    if (frame.sky) {
      pass.setPipeline(pipelines.sky);
      pass.draw(3);
    }
    // Finest level first: its fragments are the nearest, so the depth test rejects the coarser
    // levels' hidden ones early.
    pass.setPipeline(pipelines.terrain);
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      const gpu = this.#levels[level];
      if (gpu === undefined || (frame.levelMask & (1 << level)) === 0) continue;
      pass.setBindGroup(0, this.#bindGroups[level] as GPUBindGroup);
      pass.setVertexBuffer(0, gpu.vertices);
      pass.setIndexBuffer(gpu.indices, 'uint16');
      pass.drawIndexed(gpu.indexCount, level === 0 ? frame.patchInstances : 1);
    }
    pass.setBindGroup(0, this.#bindGroups[0] as GPUBindGroup);
    if (frame.model) {
      pass.setPipeline(pipelines.mesh);
      const exterior = this.#meshes.get('exterior');
      const interior = this.#meshes.get('cockpit');
      if (exterior) {
        pass.setVertexBuffer(0, exterior.vertices);
        pass.setIndexBuffer(exterior.indices, 'uint16');
        pass.drawIndexed(frame.hideCanopy ? exterior.bodyIndexCount : exterior.indexCount);
      }
      if (interior && frame.cockpit) {
        pass.setVertexBuffer(0, interior.vertices);
        pass.setIndexBuffer(interior.indices, 'uint16');
        pass.drawIndexed(interior.indexCount);
      }
    }
    if (frame.instanceCount > 0) {
      pass.setPipeline(pipelines.marker);
      pass.setVertexBuffer(0, this.#markerVertices);
      pass.setVertexBuffer(1, this.#markerInstances);
      pass.draw(MARKER_VERTEX_COUNT, frame.instanceCount);
    }
    if (frame.fx) {
      pass.setPipeline(pipelines.fx);
      pass.draw(3);
    }
    pass.end();
  }

  async finish(): Promise<void> {
    if (!this.#disposed) await this.#device.queue.onSubmittedWorkDone();
  }

  async debugReadPixels(): Promise<Uint8Array> {
    const w = this.#width;
    const h = this.#height;
    if (this.#disposed || this.#last === undefined) {
      return new Uint8Array(w * h * 4);
    }
    const device = this.#device;
    const target = device.createTexture({
      size: [w, h],
      format: this.#format,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
    const bytesPerRow = Math.ceil((w * 4) / 256) * 256;
    const readback = device.createBuffer({
      size: bytesPerRow * h,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    const encoder = device.createCommandEncoder();
    this.#draw(encoder, target.createView(), this.#last);
    encoder.copyTextureToBuffer({ texture: target }, { buffer: readback, bytesPerRow }, [w, h]);
    device.queue.submit([encoder.finish()]);
    await readback.mapAsync(GPUMapMode.READ);
    const mapped = new Uint8Array(readback.getMappedRange());
    const out = new Uint8Array(w * h * 4);
    const bgra = this.#format.startsWith('bgra');
    for (let y = 0; y < h; y++) {
      const row = mapped.subarray(y * bytesPerRow, y * bytesPerRow + w * 4);
      if (bgra) {
        for (let x = 0; x < w * 4; x += 4) {
          out[y * w * 4 + x] = row[x + 2] ?? 0;
          out[y * w * 4 + x + 1] = row[x + 1] ?? 0;
          out[y * w * 4 + x + 2] = row[x] ?? 0;
          out[y * w * 4 + x + 3] = row[x + 3] ?? 0;
        }
      } else {
        out.set(row, y * w * 4);
      }
    }
    readback.unmap();
    readback.destroy();
    target.destroy();
    return out;
  }

  onLost(listener: (reason: string) => void): void {
    this.#lost.push(listener);
  }

  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#depth?.destroy();
    this.#msaa?.destroy();
    for (const level of this.#levels) {
      level?.vertices.destroy();
      level?.indices.destroy();
    }
    for (const buffer of this.#levelUniforms) buffer.destroy();
    for (const m of this.#meshes.values()) {
      m.vertices.destroy();
      m.indices.destroy();
    }
    this.#modelUniform.destroy();
    this.#markerVertices.destroy();
    this.#markerInstances.destroy();
    this.#uniform.destroy();
    this.#device.destroy();
  }
}

/** The four render pipelines for drawing into targets of `samples` samples per pixel. */
async function buildPipelines(
  device: GPUDevice,
  format: GPUTextureFormat,
  modules: Record<'sky' | 'mesh' | 'terrain' | 'marker' | 'fx', GPUShaderModule>,
  layout: GPUPipelineLayout,
  samples: SampleCount,
): Promise<Pipelines> {
  const multisample: GPUMultisampleState = { count: samples };
  const depthStencil: GPUDepthStencilState = {
    format: DEPTH_FORMAT,
    depthWriteEnabled: true,
    depthCompare: 'greater', // reverse-Z
  };
  const primitive: GPUPrimitiveState = {
    topology: 'triangle-list',
    cullMode: 'back',
    frontFace: 'ccw',
  };
  // The sky and the screen effect are one triangle each, with no depth test or write. A pipeline
  // used in a pass with a depth attachment has to name it.
  const overlayDepth: GPUDepthStencilState = {
    format: DEPTH_FORMAT,
    depthWriteEnabled: false,
    depthCompare: 'always',
  };
  const triangle: GPUPrimitiveState = { topology: 'triangle-list', cullMode: 'none' };
  const [sky, mesh, terrain, marker, fx] = await Promise.all([
    device.createRenderPipelineAsync({
      label: `sky x${String(samples)}`,
      layout,
      vertex: { module: modules.sky, entryPoint: 'vs_main' },
      fragment: { module: modules.sky, entryPoint: 'fs_main', targets: [{ format }] },
      primitive: triangle,
      depthStencil: overlayDepth,
      multisample,
    }),
    device.createRenderPipelineAsync({
      label: `aircraft mesh x${String(samples)}`,
      layout,
      vertex: {
        module: modules.mesh,
        entryPoint: 'vs_main',
        buffers: [
          {
            arrayStride: MESH_VERTEX_BYTES,
            attributes: [
              { shaderLocation: 0, offset: 0, format: 'float32x3' },
              { shaderLocation: 1, offset: 12, format: 'float32x3' },
              { shaderLocation: 2, offset: 24, format: 'float32x2' },
            ],
          },
        ],
      },
      fragment: { module: modules.mesh, entryPoint: 'fs_main', targets: [{ format }] },
      primitive,
      depthStencil,
      multisample,
    }),
    device.createRenderPipelineAsync({
      label: `terrain patch x${String(samples)}`,
      layout,
      vertex: {
        module: modules.terrain,
        entryPoint: 'vs_main',
        buffers: [
          {
            arrayStride: VERTEX_BYTES,
            attributes: [
              { shaderLocation: 0, offset: 0, format: 'float32x3' },
              { shaderLocation: 1, offset: 12, format: 'float32x3' },
              { shaderLocation: 2, offset: 24, format: 'float32' },
            ],
          },
        ],
      },
      fragment: { module: modules.terrain, entryPoint: 'fs_main', targets: [{ format }] },
      primitive,
      depthStencil,
      multisample,
    }),
    device.createRenderPipelineAsync({
      label: `marker x${String(samples)}`,
      layout,
      vertex: {
        module: modules.marker,
        entryPoint: 'vs_main',
        buffers: [
          {
            arrayStride: MARKER_VERTEX_FLOATS * 4,
            attributes: [
              { shaderLocation: 0, offset: 0, format: 'float32x3' },
              { shaderLocation: 1, offset: 12, format: 'float32x3' },
            ],
          },
          {
            arrayStride: INSTANCE_BYTES,
            stepMode: 'instance',
            attributes: [
              { shaderLocation: 2, offset: 0, format: 'float32x4' },
              { shaderLocation: 3, offset: 16, format: 'float32x4' },
              { shaderLocation: 4, offset: 32, format: 'float32x4' },
            ],
          },
        ],
      },
      fragment: { module: modules.marker, entryPoint: 'fs_main', targets: [{ format }] },
      primitive,
      depthStencil,
      multisample,
    }),
    // The pilot's g and red-out, over the finished picture: alpha blended.
    device.createRenderPipelineAsync({
      label: `screen fx x${String(samples)}`,
      layout,
      vertex: { module: modules.fx, entryPoint: 'vs_main' },
      fragment: {
        module: modules.fx,
        entryPoint: 'fs_main',
        targets: [
          {
            format,
            blend: {
              color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha', operation: 'add' },
              alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            },
          },
        ],
      },
      primitive: triangle,
      depthStencil: overlayDepth,
      multisample,
    }),
  ]);
  return { sky, mesh, terrain, marker, fx };
}
