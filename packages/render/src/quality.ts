/**
 * Graphics presets (ADR-40). A preset is a resolution scale and a sample count; the player can
 * override the scale (50 to 200 per cent), and a benchmark preset or a URL flag picks the starting
 * point. The drawing buffer is the canvas's device-pixel size times the scale; the browser scales
 * it to the canvas, so a lower scale is a softer picture, not a smaller one.
 *
 * | preset | resolution scale | MSAA |
 * |--------|------------------|------|
 * | low    | 66 %             | off  |
 * | medium | 85 %             | off  |
 * | high   | 100 %            | 4x   |
 */
import type { QualityChoice } from '@flying/platform-web/flags';
import type { SampleCount } from './renderer.ts';

export interface Quality {
  /** The drawing buffer's size as a fraction of the canvas's size in device pixels. */
  resolutionScale: number;
  sampleCount: SampleCount;
}

export const QUALITY_PRESETS: Readonly<Record<QualityChoice, Quality>> = {
  low: { resolutionScale: 0.66, sampleCount: 1 },
  medium: { resolutionScale: 0.85, sampleCount: 1 },
  high: { resolutionScale: 1, sampleCount: 4 },
};

/** What a renderer does until it is told a preset: the picture as v0.1b drew it. */
export const QUALITY_UNSET: Quality = { resolutionScale: 1, sampleCount: 1 };

/** The player's range for the resolution scale. */
export const RESOLUTION_SCALE_MIN = 0.5;
export const RESOLUTION_SCALE_MAX = 2;

/** The largest drawing buffer along one side, pixels (a limit every GPU we target has). */
export const MAX_DRAWING_SIDE = 8192;

/** The preset's quality, with the scale replaced if one is given (kept in the player's range). */
export function qualityFor(preset: QualityChoice, resolutionScale?: number): Quality {
  const base = QUALITY_PRESETS[preset];
  if (resolutionScale === undefined || !Number.isFinite(resolutionScale)) return base;
  return {
    ...base,
    resolutionScale: Math.min(
      Math.max(resolutionScale, RESOLUTION_SCALE_MIN),
      RESOLUTION_SCALE_MAX,
    ),
  };
}

export interface Size {
  width: number;
  height: number;
}

/**
 * The drawing buffer for a canvas `device` pixels in size: a fixed size if one is given (a perf or
 * screenshot run), else the device size times `scale`. Never under a pixel or over the limit.
 */
export function drawingSize(device: Size, scale: number, fixed?: Size): Size {
  const clamp = (v: number): number => Math.min(Math.max(Math.round(v), 1), MAX_DRAWING_SIDE);
  if (fixed) return { width: clamp(fixed.width), height: clamp(fixed.height) };
  return { width: clamp(device.width * scale), height: clamp(device.height * scale) };
}
