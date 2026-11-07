import { describe, expect, it } from 'vitest';
import { HUD_PALETTES, QUALITY_PRESETS } from '@flying/render';
import { applySettingsPatch, DEFAULT_SETTINGS, HUD_PALETTE_SETTINGS } from '@flying/ui';
import {
  busesOf,
  presetFor,
  qualityOf,
  renderSettingsOf,
  viewSettingsOf,
} from './present-settings.ts';

describe('the settings the HUD can be coloured with', () => {
  it('are the palettes the renderer has, in the same order', () => {
    expect([...HUD_PALETTE_SETTINGS]).toEqual([...HUD_PALETTES]);
  });
});

describe('viewSettingsOf', () => {
  it('carries the field of view, the shake, the HUD brightness and reduced motion', () => {
    const s = applySettingsPatch(DEFAULT_SETTINGS, {
      fovDeg: 100,
      cameraShake: 40,
      hud: { brightness: 60 },
    });
    expect(viewSettingsOf(s, false)).toEqual({
      fovDeg: 100,
      shakePercent: 40,
      reducedMotion: false,
      hudBrightness: 60,
    });
  });

  it('follows the system for reduced motion unless the player says', () => {
    expect(viewSettingsOf(DEFAULT_SETTINGS, true).reducedMotion).toBe(true);
    expect(
      viewSettingsOf(applySettingsPatch(DEFAULT_SETTINGS, { reducedMotion: 'off' }), true)
        .reducedMotion,
    ).toBe(false);
    expect(
      viewSettingsOf(applySettingsPatch(DEFAULT_SETTINGS, { reducedMotion: 'on' }), false)
        .reducedMotion,
    ).toBe(true);
  });
});

describe('renderSettingsOf', () => {
  it('gives the screen effect strength, the HUD colour and the overlay size as a fraction', () => {
    const s = applySettingsPatch(DEFAULT_SETTINGS, {
      gfxStrength: 30,
      hud: { palette: 'amber', overlayScale: 150 },
    });
    expect(renderSettingsOf(s)).toEqual({ gfxStrength: 30, hudPalette: 'amber', hudScale: 1.5 });
  });
});

describe('presetFor', () => {
  it("is the player's choice when they made one", () => {
    expect(presetFor('low', 'high')).toBe('low');
    expect(presetFor('high', undefined)).toBe('high');
  });
  it('follows the benchmark on auto, with the top preset covering ultra', () => {
    expect(presetFor('auto', 'low')).toBe('low');
    expect(presetFor('auto', 'medium')).toBe('medium');
    expect(presetFor('auto', 'high')).toBe('high');
    expect(presetFor('auto', 'ultra')).toBe('high');
  });
  it('is Medium on auto before there is a benchmark', () => {
    expect(presetFor('auto', undefined)).toBe('medium');
  });
});

describe('qualityOf', () => {
  it("is the preset's, at 100 per cent", () => {
    expect(
      qualityOf(applySettingsPatch(DEFAULT_SETTINGS, { video: { preset: 'high' } }), undefined),
    ).toEqual(QUALITY_PRESETS.high);
    expect(qualityOf(DEFAULT_SETTINGS, 'low')).toEqual(QUALITY_PRESETS.low);
  });
  it("takes the player's resolution scale over the preset's, keeping its samples", () => {
    const s = applySettingsPatch(DEFAULT_SETTINGS, {
      video: { preset: 'high', resolutionScale: 150 },
    });
    expect(qualityOf(s, undefined)).toEqual({ resolutionScale: 1.5, sampleCount: 4 });
    const low = applySettingsPatch(DEFAULT_SETTINGS, {
      video: { preset: 'low', resolutionScale: 50 },
    });
    expect(qualityOf(low, undefined)).toEqual({ resolutionScale: 0.5, sampleCount: 1 });
  });
});

describe('busesOf', () => {
  it('turns per cent into 0 to 1', () => {
    const s = applySettingsPatch(DEFAULT_SETTINGS, {
      audio: { master: 50, aircraft: 25, warnings: 100, ui: 0 },
    });
    expect(busesOf(s)).toEqual({ master: 0.5, aircraft: 0.25, warnings: 1, ui: 0 });
  });
});
