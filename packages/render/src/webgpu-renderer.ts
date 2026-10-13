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
  type Renderer,
  type ShaderSources,
} from './renderer.ts';
import { FRAME_UNIFORM_BYTES, type FrameData } from './scene.ts';
import { VERTEX_BYTES, type TerrainMesh } from './terrain-mesh.ts';
import { SNAPSHOT } from '@flying/sim-bridge/layout';

const DEPTH_FORMAT = 'depth32float';
const INSTANCE_BYTES = MARKER_INSTANCE_FLOATS * 4;

export class WebGpuRenderer implements Renderer {
  readonly backend: Backend = 'webgpu';
  readonly zRange: ZRange = '0to1';
  readonly description: string;

  readonly #canvas: CanvasLike;
  readonly #device: GPUDevice;
  readonly #context: GPUCanvasContext;
  readonly #format: GPUTextureFormat;
  readonly #uniform: GPUBuffer;
  readonly #bindGroup: GPUBindGroup;
  readonly #terrainPipeline: GPURenderPipeline;
  readonly #markerPipeline: GPURenderPipeline;
  readonly #markerVertices: GPUBuffer;
  readonly #markerInstances: GPUBuffer;
  readonly #lost: ((reason: string) => void)[] = [];
  #depth: GPUTexture | undefined;
  #terrainVertices: GPUBuffer | undefined;
  #terrainIndices: GPUBuffer | undefined;
  #terrainIndexCount = 0;
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
    terrainPipeline: GPURenderPipeline;
    markerPipeline: GPURenderPipeline;
    layout: GPUBindGroupLayout;
  }) {
    this.#canvas = args.canvas;
    this.#device = args.device;
    this.#context = args.context;
    this.#format = args.format;
    this.description = args.description;
    this.#terrainPipeline = args.terrainPipeline;
    this.#markerPipeline = args.markerPipeline;

    const device = args.device;
    this.#uniform = device.createBuffer({
      label: 'frame uniform',
      size: FRAME_UNIFORM_BYTES,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.#bindGroup = device.createBindGroup({
      layout: args.layout,
      entries: [{ binding: 0, resource: { buffer: this.#uniform } }],
    });
    const mesh = buildMarkerMesh();
    this.#markerVertices = device.createBuffer({
      label: 'marker mesh',
      size: mesh.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(this.#markerVertices, 0, mesh);
    this.#markerInstances = device.createBuffer({
      label: 'marker instances',
      size: SNAPSHOT.BODY_CAPACITY * INSTANCE_BYTES,
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
    const context = canvas.getContext('webgpu') as GPUCanvasContext | null;
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

    const layout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: 'uniform' },
        },
      ],
    });
    const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout] });
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

    const terrainModule = device.createShaderModule({
      label: 'terrain patch',
      code: shaders.terrainWgsl,
    });
    const markerModule = device.createShaderModule({ label: 'marker', code: shaders.markerWgsl });
    const [terrainPipeline, markerPipeline] = await Promise.all([
      device.createRenderPipelineAsync({
        label: 'terrain patch',
        layout: pipelineLayout,
        vertex: {
          module: terrainModule,
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
        fragment: { module: terrainModule, entryPoint: 'fs_main', targets: [{ format }] },
        primitive,
        depthStencil,
      }),
      device.createRenderPipelineAsync({
        label: 'marker',
        layout: pipelineLayout,
        vertex: {
          module: markerModule,
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
              ],
            },
          ],
        },
        fragment: { module: markerModule, entryPoint: 'fs_main', targets: [{ format }] },
        primitive,
        depthStencil,
      }),
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
      terrainPipeline,
      markerPipeline,
      layout,
    });
  }

  resize(widthPx: number, heightPx: number): void {
    const w = Math.max(1, Math.floor(widthPx));
    const h = Math.max(1, Math.floor(heightPx));
    this.#width = w;
    this.#height = h;
    this.#canvas.width = w;
    this.#canvas.height = h;
    this.#depth?.destroy();
    this.#depth = this.#device.createTexture({
      label: 'depth',
      size: [w, h],
      format: DEPTH_FORMAT,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
  }

  setTerrainPatch(mesh: TerrainMesh): void {
    this.#terrainVertices?.destroy();
    this.#terrainIndices?.destroy();
    const device = this.#device;
    this.#terrainVertices = device.createBuffer({
      label: 'terrain vertices',
      size: mesh.vertices.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(this.#terrainVertices, 0, mesh.vertices);
    // Buffer writes must be a multiple of four bytes; 16-bit indices may not be.
    const indexBytes = Math.ceil(mesh.indices.byteLength / 4) * 4;
    this.#terrainIndices = device.createBuffer({
      label: 'terrain indices',
      size: indexBytes,
      usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
    });
    const padded = new Uint16Array(indexBytes / 2);
    padded.set(mesh.indices);
    device.queue.writeBuffer(this.#terrainIndices, 0, padded);
    this.#terrainIndexCount = mesh.indices.length;
  }

  render(frame: FrameData): void {
    if (this.#disposed) return;
    this.#last = frame;
    const encoder = this.#device.createCommandEncoder();
    this.#draw(encoder, this.#context.getCurrentTexture().createView(), frame);
    this.#device.queue.submit([encoder.finish()]);
  }

  #draw(encoder: GPUCommandEncoder, target: GPUTextureView, frame: FrameData): void {
    if (this.#depth === undefined) {
      this.resize(this.#width, this.#height);
    }
    const queue = this.#device.queue;
    queue.writeBuffer(this.#uniform, 0, frame.uniform);
    if (frame.instanceCount > 0) {
      queue.writeBuffer(
        this.#markerInstances,
        0,
        frame.instances,
        0,
        frame.instanceCount * MARKER_INSTANCE_FLOATS,
      );
    }
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: target,
          clearValue: { r: CLEAR_COLOUR[0], g: CLEAR_COLOUR[1], b: CLEAR_COLOUR[2], a: 1 },
          loadOp: 'clear',
          storeOp: 'store',
        },
      ],
      depthStencilAttachment: {
        view: (this.#depth as GPUTexture).createView(),
        depthClearValue: 0, // reverse-Z: far is 0
        depthLoadOp: 'clear',
        depthStoreOp: 'store',
      },
    });
    pass.setBindGroup(0, this.#bindGroup);
    if (this.#terrainVertices && this.#terrainIndices) {
      pass.setPipeline(this.#terrainPipeline);
      pass.setVertexBuffer(0, this.#terrainVertices);
      pass.setIndexBuffer(this.#terrainIndices, 'uint16');
      pass.drawIndexed(this.#terrainIndexCount);
    }
    if (frame.instanceCount > 0) {
      pass.setPipeline(this.#markerPipeline);
      pass.setVertexBuffer(0, this.#markerVertices);
      pass.setVertexBuffer(1, this.#markerInstances);
      pass.draw(MARKER_VERTEX_COUNT, frame.instanceCount);
    }
    pass.end();
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
    this.#terrainVertices?.destroy();
    this.#terrainIndices?.destroy();
    this.#markerVertices.destroy();
    this.#markerInstances.destroy();
    this.#uniform.destroy();
    this.#device.destroy();
  }
}
