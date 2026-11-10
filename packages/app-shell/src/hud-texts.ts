/**
 * The words the HUD writes, from the string table (GDD 53.10), so a translation changes them. The
 * HUD's font has to cover every character of every one of them: a test compares these with the
 * renderer's English defaults, which a test over there checks against the font.
 */
import type { HudTexts } from '@flying/render';
import { t } from '@flying/ui';

export function hudTexts(): HudTexts {
  return {
    warnings: {
      pullup: t('hud.warning.pullup'),
      stall: t('hud.warning.stall'),
      departed: t('hud.warning.departed'),
      overstress: t('hud.warning.overstress'),
      alpha: t('hud.warning.alpha'),
      g: t('hud.warning.g'),
      overspeed: t('hud.warning.overspeed'),
      'gear-speed': t('hud.warning.gear-speed'),
      'flap-speed': t('hud.warning.flap-speed'),
      gear: t('hud.warning.gear'),
    },
    assist: [
      t('hud.assist.full'),
      t('hud.assist.standard'),
      t('hud.assist.reduced'),
      t('hud.assist.manual'),
    ],
    gear: t('hud.gear'),
    flapsTakeOff: t('hud.flaps-takeoff'),
    flapsLanding: t('hud.flaps-landing'),
    brakes: t('hud.brakes'),
    parking: t('hud.parking'),
    max: t('hud.max'),
    radar: t('hud.radar'),
    throttle: t('hud.throttle'),
    afterburner: t('hud.afterburner'),
    fuel: t('hud.fuel'),
    fuelFlow: t('hud.fuel-flow'),
    endurance: t('hud.endurance'),
    gun: t('hud.gun'),
    srm: t('hud.srm'),
    arm: t('hud.arm'),
    safe: t('hud.safe'),
    tone: t('hud.tone'),
    lock: t('hud.lock'),
    shoot: t('hud.shoot'),
    inRange: t('hud.in-range'),
    hit: t('hud.hit'),
    splash: t('hud.splash'),
    inhibit: [
      t('hud.inhibit.safe'),
      t('hud.inhibit.empty'),
      t('hud.inhibit.no-lock'),
      t('hud.inhibit.too-close'),
      t('hud.inhibit.g-limit'),
      t('hud.inhibit.alpha-limit'),
      t('hud.inhibit.slow'),
    ],
    rangeLabel: t('hud.range'),
    closureLabel: t('hud.closure'),
  };
}
