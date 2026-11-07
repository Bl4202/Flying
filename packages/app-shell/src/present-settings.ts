/**
 * What each of the player's settings means to the parts that draw and sound the flight: pure
 * functions from `Settings`, so the mapping is tested without a browser and `demo.ts` only applies it.
 */
import type { BusLevels } from '@flying/audio';
import type { QualityChoice } from '@flying/platform-web';
import type { ViewSettings } from '@flying/input';
import { qualityFor, type Quality, type RenderSettings } from '@flying/render';
import { reducedMotionOn, type PresetSetting, type Settings } from '@flying/ui';

/** The benchmark's presets (`perf`'s `Preset`), which include one the renderer has no more to give to. */
export type BenchmarkPreset = 'low' | 'medium' | 'high' | 'ultra';

/** The settings the view (camera and HUD brightness) is driven by. */
export function viewSettingsOf(
  s: Readonly<Settings>,
  systemPrefersReducedMotion: boolean,
): Required<Omit<ViewSettings, 'chaseDistanceM'>> {
  return {
    fovDeg: s.fovDeg,
    shakePercent: s.cameraShake,
    reducedMotion: reducedMotionOn(s.reducedMotion, systemPrefersReducedMotion),
    hudBrightness: s.hud.brightness,
  };
}

/** The settings the renderer takes directly. */
export function renderSettingsOf(s: Readonly<Settings>): RenderSettings {
  return {
    gfxStrength: s.gfxStrength,
    hudPalette: s.hud.palette,
    hudScale: s.hud.overlayScale / 100,
  };
}

/**
 * The preset in force: the player's, or for `auto` the one the first-run benchmark chose (the
 * renderer's top preset covers the benchmark's `ultra`), or Medium where there is no benchmark yet.
 */
export function presetFor(
  setting: PresetSetting,
  benchmark: BenchmarkPreset | undefined,
): QualityChoice {
  if (setting !== 'auto') return setting;
  if (benchmark === undefined) return 'medium';
  return benchmark === 'ultra' ? 'high' : benchmark;
}

/** The graphics quality: the preset's, with the player's resolution scale over it when it is not 100 %. */
export function qualityOf(s: Readonly<Settings>, benchmark: BenchmarkPreset | undefined): Quality {
  const scale = s.video.resolutionScale;
  const preset = presetFor(s.video.preset, benchmark);
  return qualityFor(preset, scale === 100 ? undefined : scale / 100);
}

/** The volumes as 0 to 1. */
export function busesOf(s: Readonly<Settings>): BusLevels {
  const a = s.audio;
  return {
    master: a.master / 100,
    aircraft: a.aircraft / 100,
    warnings: a.warnings / 100,
    ui: a.ui / 100,
  };
}
