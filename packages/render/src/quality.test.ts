import { describe, expect, it } from 'vitest';
import {
  drawingSize,
  MAX_DRAWING_SIDE,
  QUALITY_PRESETS,
  QUALITY_UNSET,
  qualityFor,
} from './quality.ts';

describe('the presets', () => {
  it('are Low 66 % without MSAA, Medium 85 % without, High 100 % with 4x', () => {
    expect(QUALITY_PRESETS.low).toEqual({ resolutionScale: 0.66, sampleCount: 1 });
    expect(QUALITY_PRESETS.medium).toEqual({ resolutionScale: 0.85, sampleCount: 1 });
    expect(QUALITY_PRESETS.high).toEqual({ resolutionScale: 1, sampleCount: 4 });
  });

  it('start from the picture v0.1b drew, until one is chosen', () => {
    expect(QUALITY_UNSET).toEqual({ resolutionScale: 1, sampleCount: 1 });
  });
});

describe('qualityFor', () => {
  it('is the preset, or the preset with the player scale kept between 50 and 200 per cent', () => {
    expect(qualityFor('high')).toBe(QUALITY_PRESETS.high);
    expect(qualityFor('low', 1.5)).toEqual({ resolutionScale: 1.5, sampleCount: 1 });
    expect(qualityFor('medium', 0.1).resolutionScale).toBe(0.5);
    expect(qualityFor('medium', 9).resolutionScale).toBe(2);
    expect(qualityFor('medium', Number.NaN)).toBe(QUALITY_PRESETS.medium);
  });
});

describe('drawingSize', () => {
  const device = { width: 2560, height: 1440 };
  it('scales the canvas size and rounds to whole pixels', () => {
    expect(drawingSize(device, 1)).toEqual(device);
    expect(drawingSize(device, 0.66)).toEqual({ width: 1690, height: 950 });
    expect(drawingSize(device, 2)).toEqual({ width: 5120, height: 2880 });
  });

  it('takes a fixed size over everything else', () => {
    expect(drawingSize(device, 0.5, { width: 1920, height: 1080 })).toEqual({
      width: 1920,
      height: 1080,
    });
  });

  it('never goes under a pixel or over the limit', () => {
    expect(drawingSize({ width: 1, height: 1 }, 0.5)).toEqual({ width: 1, height: 1 });
    expect(drawingSize({ width: 9000, height: 9000 }, 2)).toEqual({
      width: MAX_DRAWING_SIDE,
      height: MAX_DRAWING_SIDE,
    });
  });
});
