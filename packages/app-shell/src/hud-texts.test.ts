import { describe, expect, it } from 'vitest';
import { DEFAULT_HUD_TEXTS } from '@flying/render';
import { HUD_WARNING_IDS } from '@flying/render/hud-texts';
import { hudTexts } from './hud-texts.ts';

describe('hudTexts', () => {
  it('is the English the renderer defaults to, which the renderer checks against its font', () => {
    expect(hudTexts()).toEqual(DEFAULT_HUD_TEXTS);
  });

  it('has a text for every warning there is', () => {
    expect(Object.keys(hudTexts().warnings).sort()).toEqual([...HUD_WARNING_IDS].sort());
  });
});
