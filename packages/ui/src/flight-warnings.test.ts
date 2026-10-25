// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import {
  ASSIST_LEVEL,
  newAircraftStatus,
  OWNSHIP_FLAG,
  OWNSHIP_FLAG2,
  type AircraftStatus,
} from '@flying/sim-bridge';
import { countNodes } from './dom.ts';
import { flightWarnings, stripSeverity, stripText, WarningStrip } from './flight-warnings.ts';

function status(over: Partial<AircraftStatus> = {}): AircraftStatus {
  return {
    ...newAircraftStatus(),
    assist: ASSIST_LEVEL.STANDARD,
    cas: 150,
    agl: 2000,
    vs: 0,
    ...over,
  };
}
const ids = (s: AircraftStatus): string[] => flightWarnings(s).map((w) => w.id);

describe('flightWarnings', () => {
  it('has none in ordinary flight', () => {
    expect(flightWarnings(status())).toEqual([]);
    expect(stripSeverity(status())).toBe('none');
  });

  it('has none without an aircraft, for the spectator, or for a wreck', () => {
    expect(flightWarnings(undefined)).toEqual([]);
    expect(flightWarnings(status({ assist: 0 }))).toEqual([]);
    expect(
      flightWarnings(status({ flags: OWNSHIP_FLAG.SPECTATOR | OWNSHIP_FLAG.STALL_WARN })),
    ).toEqual([]);
    expect(
      flightWarnings(status({ flags: OWNSHIP_FLAG.CRASHED | OWNSHIP_FLAG.STALL_WARN })),
    ).toEqual([]);
  });

  it('reads each flag', () => {
    expect(ids(status({ flags: OWNSHIP_FLAG.STALL_WARN }))).toEqual(['stall']);
    const cases: [number, string][] = [
      [OWNSHIP_FLAG2.ALPHA_LIMIT, 'alpha'],
      [OWNSHIP_FLAG2.G_LIMIT, 'g'],
      [OWNSHIP_FLAG2.OVERSPEED, 'overspeed'],
      [OWNSHIP_FLAG2.OVERSTRESS, 'overstress'],
      [OWNSHIP_FLAG2.GEAR_WARNING, 'gear'],
      [OWNSHIP_FLAG2.DEPARTED, 'departed'],
      [OWNSHIP_FLAG2.GEAR_OVERSPEED, 'gear-speed'],
      [OWNSHIP_FLAG2.FLAP_OVERSPEED, 'flap-speed'],
    ];
    for (const [bit, id] of cases) expect(ids(status({ flags2: bit })), id).toEqual([id]);
  });

  it('puts warnings before cautions, most urgent first', () => {
    const s = status({
      flags: OWNSHIP_FLAG.STALL_WARN,
      flags2:
        OWNSHIP_FLAG2.GEAR_WARNING |
        OWNSHIP_FLAG2.OVERSTRESS |
        OWNSHIP_FLAG2.ALPHA_LIMIT |
        OWNSHIP_FLAG2.DEPARTED,
    });
    expect(ids(s)).toEqual(['stall', 'departed', 'overstress', 'alpha', 'gear']);
    expect(flightWarnings(s).map((w) => w.severity)).toEqual([
      'warning',
      'warning',
      'warning',
      'caution',
      'caution',
    ]);
    expect(stripSeverity(s)).toBe('warning');
    expect(stripSeverity(status({ flags2: OWNSHIP_FLAG2.G_LIMIT }))).toBe('caution');
  });

  describe('pull up', () => {
    const diving = { agl: 300, vs: -100, cas: 200 };
    it('warns when the ground is under five seconds away at the present sink rate', () => {
      expect(ids(status(diving))).toEqual(['pullup']);
      expect(ids(status({ ...diving, agl: 600 }))).toEqual([]); // too high to matter yet
      expect(ids(status({ agl: 300, vs: -50 }))).toEqual([]); // six seconds
      expect(ids(status({ agl: 240, vs: -50 }))).toEqual(['pullup']); // under five
    });

    it('stays quiet when climbing, level, or sinking gently', () => {
      expect(ids(status({ agl: 50, vs: 5 }))).toEqual([]);
      expect(ids(status({ agl: 50, vs: 0 }))).toEqual([]);
      expect(ids(status({ agl: 5, vs: -0.5 }))).toEqual([]);
    });

    it('stays quiet on the wheels and on a landing approach with the gear down', () => {
      expect(ids(status({ ...diving, flags2: OWNSHIP_FLAG2.WOW }))).toEqual([]);
      expect(ids(status({ agl: 20, vs: -4, cas: 75, gearBits: 1 << 6 }))).toEqual([]);
      // Fast with the gear down is not a landing.
      expect(ids(status({ agl: 150, vs: -100, cas: 150, gearBits: 1 << 6 }))).toEqual(['pullup']);
    });
  });
});

describe('stripText', () => {
  it('names the assist level, then each warning behind a shape', () => {
    expect(stripText(status())).toBe('STD');
    expect(stripText(status({ assist: ASSIST_LEVEL.FULL }))).toBe('FULL');
    expect(stripText(status({ assist: ASSIST_LEVEL.REDUCED }))).toBe('RED');
    expect(stripText(status({ assist: ASSIST_LEVEL.MANUAL }))).toBe('MAN');
    const s = status({
      flags: OWNSHIP_FLAG.STALL_WARN,
      flags2: OWNSHIP_FLAG2.ALPHA_LIMIT | OWNSHIP_FLAG2.GEAR_WARNING,
    });
    expect(stripText(s)).toBe('STD  ▲ STALL  ◆ α-LIM  ◆ GEAR');
  });

  it('is empty with no aircraft', () => {
    expect(stripText(undefined)).toBe('');
    expect(stripText(status({ assist: 0 }))).toBe('');
  });
});

describe('WarningStrip', () => {
  function setup() {
    const root = document.createElement('div');
    document.body.append(root);
    const strip = new WarningStrip(root);
    return { root, strip, el: strip.element };
  }

  it('is a single element, hidden until there is something to say', () => {
    const { root, el } = setup();
    expect(root.children).toHaveLength(1);
    expect(el.hidden).toBe(true);
    expect(el.getAttribute('role')).toBe('status');
    expect(countNodes(root)).toBe(1);
  });

  it('shows the line with two nodes at most, and marks its severity', () => {
    const { root, strip, el } = setup();
    strip.update(status({ flags: OWNSHIP_FLAG.STALL_WARN }));
    expect(el.hidden).toBe(false);
    expect(el.textContent).toBe('STD  ▲ STALL');
    expect(el.dataset['severity']).toBe('warning');
    expect(countNodes(root)).toBe(2);
    strip.update(status({ flags2: OWNSHIP_FLAG2.G_LIMIT }));
    expect(el.dataset['severity']).toBe('caution');
    strip.update(status());
    expect(el.dataset['severity']).toBe('none');
    expect(el.textContent).toBe('STD');
  });

  it('hides again with no aircraft', () => {
    const { strip, el } = setup();
    strip.update(status());
    strip.update(undefined);
    expect(el.hidden).toBe(true);
    expect(el.textContent).toBe('');
  });

  it('rewrites the text only when it changes', () => {
    const { strip, el } = setup();
    strip.update(status());
    const node = el.firstChild;
    strip.update(status());
    strip.update(status({ cas: 160 }));
    expect(el.firstChild).toBe(node);
    strip.update(status({ flags: OWNSHIP_FLAG.STALL_WARN }));
    expect(el.firstChild).not.toBe(node);
  });

  it('can be taken away', () => {
    const { root, strip } = setup();
    strip.destroy();
    expect(root.children).toHaveLength(0);
  });
});
