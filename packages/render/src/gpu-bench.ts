/**
 * GPU frame time at several load levels (GDD 60.8). A fixed scene (one terrain patch seen from
 * 1.5 km, with the markers) is drawn with 1, 16 and 64 copies of the patch, back to back, and the
 * time per frame is taken after the GPU has finished them all. The camera never moves, so the
 * only thing that changes between levels is how much there is to draw.
 *
 * Submitting several frames and waiting once measures GPU throughput; waiting after every frame
 * would measure the round trip to the GPU instead. The CPU cost of encoding is part of what is
 * timed, which is the right thing when the GPU is the bottleneck and visible when it is not.
 */
import { SnapshotInterpolator } from './interp.ts';
import type { Renderer } from './renderer.ts';
import { SceneBuilder } from './scene.ts';
import { makeSnapshot, NORTH } from './snapshot-fixture.ts';
import { buildTerrainMesh } from './terrain-mesh.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

export interface GpuBenchLevel {
  /** Copies of the terrain patch drawn per frame. */
  patches: number;
  /** Frames timed. */
  frames: number;
  /** Milliseconds per frame, GPU finished. */
  msPerFrame: number;
}

export interface GpuBenchResult {
  backend: string;
  description: string;
  width: number;
  height: number;
  levels: GpuBenchLevel[];
}

export interface GpuBenchOptions {
  renderer: Renderer;
  width: number;
  height: number;
  /** Patch counts, lightest first. */
  levels: readonly number[];
  /** About how long to spend timing each level, milliseconds. */
  budgetMs: number;
  /** Milliseconds, monotonic. */
  nowMs(): number;
  /** Called after each level, for progress. */
  onLevel?: (level: GpuBenchLevel) => void;
}

/** Frames drawn and discarded before timing, so shader compilation and first-use costs do not count. */
const WARMUP_FRAMES = 3;
const MIN_FRAMES = 3;
const MAX_FRAMES = 240;

export async function measureGpu(opts: GpuBenchOptions): Promise<GpuBenchResult> {
  const { renderer, width, height } = opts;
  renderer.resize(width, height);
  const mesh = buildTerrainMesh(makeTestPatch());
  renderer.setTerrainPatch(mesh);

  const interp = new SnapshotInterpolator();
  interp.push(
    makeSnapshot({
      publishMs: 1000,
      ownship: { pos: [0, -6000, 1500], quat: NORTH },
      bodies: Array.from({ length: 32 }, (_, i) => ({
        pos: [((i % 8) - 3.5) * 700, -2500 + Math.floor(i / 8) * 900, 1400 + (i % 5) * 80] as [
          number,
          number,
          number,
        ],
      })),
    }),
  );
  const state = interp.sample(2000);
  if (!state) throw new Error('SYS-109: the benchmark scene could not be built');
  const scene = new SceneBuilder();

  const timeFrames = async (patches: number, frames: number): Promise<number> => {
    const frame = scene.build(state, {
      aspect: width / height,
      zRange: renderer.zRange,
      timeS: 0,
      patchOriginBf: mesh.originBf,
      spacingM: mesh.spacingM,
      patchInstances: patches,
    });
    const t0 = opts.nowMs();
    for (let i = 0; i < frames; i++) renderer.render(frame);
    await renderer.finish();
    return opts.nowMs() - t0;
  };

  const levels: GpuBenchLevel[] = [];
  for (const patches of opts.levels) {
    await timeFrames(patches, WARMUP_FRAMES);
    // One frame to learn the cost, then as many as fit the budget.
    const probe = await timeFrames(patches, 1);
    const frames = Math.min(
      MAX_FRAMES,
      Math.max(MIN_FRAMES, Math.floor(opts.budgetMs / Math.max(probe, 0.01))),
    );
    const elapsed = await timeFrames(patches, frames);
    const level: GpuBenchLevel = { patches, frames, msPerFrame: elapsed / frames };
    levels.push(level);
    opts.onLevel?.(level);
  }
  return {
    backend: renderer.backend,
    description: renderer.description,
    width,
    height,
    levels,
  };
}
