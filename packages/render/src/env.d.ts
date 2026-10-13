/// <reference types="vite/client" />

/** Preprocessed WGSL (see `vite-plugin-wgsl`). */
declare module '*.wgsl' {
  const source: string;
  export default source;
}

// The WebGPU flag constants. TypeScript's DOM library declares the interfaces but not these
// objects, whose values are fixed by the specification and supplied by the browser at run time.
declare const GPUBufferUsage: {
  readonly MAP_READ: number;
  readonly COPY_DST: number;
  readonly INDEX: number;
  readonly VERTEX: number;
  readonly UNIFORM: number;
};
declare const GPUTextureUsage: {
  readonly COPY_SRC: number;
  readonly RENDER_ATTACHMENT: number;
};
declare const GPUShaderStage: {
  readonly VERTEX: number;
  readonly FRAGMENT: number;
};
declare const GPUMapMode: {
  readonly READ: number;
};
