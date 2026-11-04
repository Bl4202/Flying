import { describe, expect, it } from 'vitest';
import {
  HUD_BRIGHTNESS_MAX,
  HUD_BRIGHTNESS_MIN,
  HUD_BRIGHTNESS_STEP,
  ViewController,
  VIEW_DEFAULTS,
} from './view-controller.ts';
import { ViewInput, VI } from './view-input.ts';

function rig() {
  const input = new ViewInput();
  const controller = new ViewController(input);
  const press = (index: number, by = 1): void => {
    input.v[index] = (input.v[index] ?? 0) + by;
  };
  return { input, controller, press };
}

describe('ViewController', () => {
  it('starts with the cockpit camera, the field of view at 75 and the HUD full and bright', () => {
    const { controller } = rig();
    expect(controller.state.camera).toBe('cockpit');
    expect(controller.state.fovDeg).toBe(75);
    expect(controller.state.hudDeclutter).toBe(0);
    expect(controller.state.hudBrightness).toBe(100);
    expect(VIEW_DEFAULTS.fovDeg).toBe(75);
  });

  it('cycles the camera on each press, and is not fooled by presses from before it existed', () => {
    const input = new ViewInput();
    input.v[VI.CAMERA_PRESSES] = 5;
    const c = new ViewController(input);
    c.update();
    expect(c.state.camera).toBe('cockpit');
    input.v[VI.CAMERA_PRESSES] = 6;
    c.update();
    expect(c.state.camera).toBe('chase');
    input.v[VI.CAMERA_PRESSES] = 7;
    c.update();
    expect(c.state.camera).toBe('cockpit');
    // Two presses between two frames: back where it was.
    input.v[VI.CAMERA_PRESSES] = 9;
    c.update();
    expect(c.state.camera).toBe('cockpit');
  });

  it('lets a menu choose the camera, and the next press cycles from there', () => {
    const { controller, press } = rig();
    controller.setCamera('chase');
    controller.update();
    expect(controller.state.camera).toBe('chase');
    press(VI.CAMERA_PRESSES);
    controller.update();
    expect(controller.state.camera).toBe('cockpit');
  });

  it('copies the held keys, the rates and the mouse look', () => {
    const { controller, input } = rig();
    input.v[VI.LOOK_YAW_RATE] = 1.5;
    input.v[VI.LOOK_PITCH_RATE] = -1.5;
    input.v[VI.MOUSE_YAW_TOTAL] = 0.25;
    input.v[VI.MOUSE_PITCH_TOTAL] = -0.5;
    input.v[VI.ZOOM] = 1;
    input.v[VI.FREE_LOOK] = 1;
    controller.update();
    const s = controller.state;
    expect([s.lookYawRate, s.lookPitchRate, s.mouseYawTotal, s.mousePitchTotal]).toEqual([
      1.5, -1.5, 0.25, -0.5,
    ]);
    expect([s.zoom, s.freeLook]).toEqual([true, true]);
  });

  it('counts recentre requests', () => {
    const { controller, press } = rig();
    press(VI.RECENTRE_PRESSES);
    press(VI.RECENTRE_PRESSES);
    controller.update();
    expect(controller.state.recentreCount).toBe(2);
    press(VI.RECENTRE_PRESSES);
    controller.update();
    expect(controller.state.recentreCount).toBe(3);
  });

  it('steps the HUD declutter through full, reduced, minimum and round again', () => {
    const { controller, press } = rig();
    const seen: number[] = [];
    for (let i = 0; i < 4; i++) {
      press(VI.DECLUTTER_PRESSES);
      controller.update();
      seen.push(controller.state.hudDeclutter);
    }
    expect(seen).toEqual([1, 2, 0, 1]);
  });

  it('moves the HUD brightness in tens, between 20 and 100 per cent', () => {
    const { controller, press } = rig();
    press(VI.BRIGHTNESS_STEPS, -1);
    controller.update();
    expect(controller.state.hudBrightness).toBe(100 - HUD_BRIGHTNESS_STEP);
    press(VI.BRIGHTNESS_STEPS, -20);
    controller.update();
    expect(controller.state.hudBrightness).toBe(HUD_BRIGHTNESS_MIN);
    press(VI.BRIGHTNESS_STEPS, 50);
    controller.update();
    expect(controller.state.hudBrightness).toBe(HUD_BRIGHTNESS_MAX);
  });

  it('keeps the settings within their ranges', () => {
    const { controller } = rig();
    controller.configure({
      fovDeg: 200,
      shakePercent: 150,
      chaseDistanceM: 5,
      hudBrightness: 3,
      reducedMotion: true,
    });
    const s = controller.state;
    expect([s.fovDeg, s.shake, s.chaseDistanceM, s.hudBrightness, s.reducedMotion]).toEqual([
      120,
      1,
      40,
      20,
      true,
    ]);
    controller.configure({ fovDeg: 50, shakePercent: 0, chaseDistanceM: 150, hudBrightness: 55 });
    expect([s.fovDeg, s.shake, s.chaseDistanceM, s.hudBrightness]).toEqual([50, 0, 150, 55]);
    // Nonsense is ignored.
    controller.configure({ fovDeg: Number.NaN, shakePercent: Number.POSITIVE_INFINITY });
    expect([s.fovDeg, s.shake]).toEqual([50, 0]);
  });

  it('says when the state changed, once, so a page without a shared channel knows when to send', () => {
    const { controller, input, press } = rig();
    expect(controller.update()).toBe(true); // the first one is always news
    expect(controller.update()).toBe(false);
    input.v[VI.LOOK_YAW_RATE] = 1;
    expect(controller.update()).toBe(true);
    expect(controller.update()).toBe(false);
    press(VI.DECLUTTER_PRESSES);
    expect(controller.update()).toBe(true);
    controller.configure({ fovDeg: 90 });
    expect(controller.update()).toBe(true);
    expect(controller.update()).toBe(false);
  });
});
