/**
 * The GPU resources a backend holds, counted as they are made and released, for the soak's leak
 * gate L5 (ADR-45): the live buffers and textures must be the same at every sortie start. Each
 * backend makes its buffers and textures through one `GpuTally`, which never counts one twice.
 */

/** Live counts (WebGL2's renderbuffers count as textures). */
export interface GpuResources {
  buffers: number;
  textures: number;
}

/** Something WebGPU releases with `destroy()`. */
interface Destroyable {
  destroy(): void;
}

export class GpuTally implements GpuResources {
  buffers = 0;
  textures = 0;

  /** A WebGPU buffer, counted until it is destroyed. */
  gpuBuffer(device: GPUDevice, desc: GPUBufferDescriptor): GPUBuffer {
    const b = device.createBuffer(desc);
    this.#track(b, 'buffers');
    return b;
  }

  /** A WebGPU texture, counted until it is destroyed. */
  gpuTexture(device: GPUDevice, desc: GPUTextureDescriptor): GPUTexture {
    const t = device.createTexture(desc);
    this.#track(t, 'textures');
    return t;
  }

  /** A WebGL object was made (`null`, from a lost context, counts as nothing). */
  glMade(kind: keyof GpuResources, obj: unknown): void {
    if (obj !== null && obj !== undefined) this[kind] += 1;
  }

  /** A WebGL object was deleted (`null` counts as nothing). */
  glDeleted(kind: keyof GpuResources, obj: unknown): void {
    if (obj !== null && obj !== undefined) this[kind] = Math.max(0, this[kind] - 1);
  }

  /** The counts, as a fresh object. */
  snapshot(): GpuResources {
    return { buffers: this.buffers, textures: this.textures };
  }

  #track(r: Destroyable, kind: keyof GpuResources): void {
    this[kind] += 1;
    const destroy = r.destroy.bind(r);
    let live = true;
    r.destroy = (): void => {
      if (live) {
        live = false;
        this[kind] -= 1;
      }
      destroy();
    };
  }
}
