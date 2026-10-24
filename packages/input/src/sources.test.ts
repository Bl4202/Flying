import { describe, expect, it } from 'vitest';
import {
  describeSource,
  deviceMatches,
  deviceName,
  parsePadId,
  sameSource,
  sourceDevice,
  sourceKey,
  type Source,
} from './sources.ts';

const samples: Source[] = [
  { kind: 'key', code: 'KeyW' },
  { kind: 'key', code: 'KeyS' },
  { kind: 'mouse-button', button: 0 },
  { kind: 'mouse-button', button: 2 },
  { kind: 'wheel', dir: 'up' },
  { kind: 'wheel', dir: 'down' },
  { kind: 'pad-button', device: 'pad', index: 0 },
  { kind: 'pad-button', device: 'joy', index: 0 },
  { kind: 'pad-axis', device: 'pad', index: 1 },
  { kind: 'pad-axis', device: 'pad', index: 1, half: 'positive' },
  { kind: 'pad-axis', device: 'pad', index: 1, half: 'negative' },
  { kind: 'hat', device: 'pad', hat: 0, dir: 'up' },
  { kind: 'hat', device: 'pad', hat: 0, dir: 'down' },
  { kind: 'hat', device: 'pad', hat: 1, dir: 'up' },
];

describe('source keys', () => {
  it('differ for every different control and agree for the same one', () => {
    const keys = samples.map(sourceKey);
    expect(new Set(keys).size).toBe(samples.length);
    for (const s of samples) expect(sameSource(s, { ...s })).toBe(true);
  });

  it('tell the two halves of one axis apart from each other and from the whole axis', () => {
    const whole: Source = { kind: 'pad-axis', device: 'pad', index: 3 };
    const pos: Source = { ...whole, half: 'positive' };
    const neg: Source = { ...whole, half: 'negative' };
    expect(sameSource(whole, pos)).toBe(false);
    expect(sameSource(pos, neg)).toBe(false);
  });

  it('separate the same index on different devices', () => {
    expect(
      sameSource(
        { kind: 'pad-button', device: 'pad', index: 4 },
        { kind: 'pad-button', device: 'joy:046d:c215', index: 4 },
      ),
    ).toBe(false);
  });
});

describe('source devices', () => {
  it('names the keyboard, the mouse, and a pad by its class', () => {
    expect(sourceDevice({ kind: 'key', code: 'KeyW' })).toBe('keyboard');
    expect(sourceDevice({ kind: 'mouse-button', button: 0 })).toBe('mouse');
    expect(sourceDevice({ kind: 'wheel', dir: 'up' })).toBe('mouse');
    expect(sourceDevice({ kind: 'hat', device: 'joy:046d:c215', hat: 0, dir: 'up' })).toBe(
      'joy:046d:c215',
    );
  });
});

describe('describeSource', () => {
  it('writes a short label for each kind', () => {
    expect(describeSource({ kind: 'key', code: 'KeyW' })).toBe('W');
    expect(describeSource({ kind: 'key', code: 'Digit4' })).toBe('4');
    expect(describeSource({ kind: 'key', code: 'ShiftLeft' })).toBe('ShiftLeft');
    expect(describeSource({ kind: 'mouse-button', button: 0 })).toBe('Mouse 1');
    expect(describeSource({ kind: 'wheel', dir: 'down' })).toBe('Wheel down');
    expect(describeSource({ kind: 'pad-button', device: 'pad', index: 7 })).toBe('Button 7');
    expect(describeSource({ kind: 'pad-axis', device: 'pad', index: 2, half: 'negative' })).toBe(
      'Axis 2-',
    );
    expect(describeSource({ kind: 'pad-axis', device: 'pad', index: 2 })).toBe('Axis 2');
    expect(describeSource({ kind: 'hat', device: 'pad', hat: 0, dir: 'left' })).toBe('Hat 0 left');
  });
});

describe('gamepad identity', () => {
  it('reads vendor and product from a Chrome id', () => {
    expect(parsePadId('Thrustmaster T.16000M (Vendor: 044f Product: b10a)')).toEqual({
      vendor: '044f',
      product: 'b10a',
      name: 'Thrustmaster T.16000M',
    });
    expect(parsePadId('Xbox 360 Controller (STANDARD GAMEPAD Vendor: 045e Product: 028e)')).toEqual(
      { vendor: '045e', product: '028e', name: 'Xbox 360 Controller' },
    );
  });

  it('pads short hex numbers and lowercases them', () => {
    expect(parsePadId('Acme Stick (Vendor: 46D Product: C215)')).toMatchObject({
      vendor: '046d',
      product: 'c215',
    });
  });

  it('reads vendor and product from a Firefox id', () => {
    expect(parsePadId('046d-c215-Logitech Extreme 3D Pro')).toEqual({
      vendor: '046d',
      product: 'c215',
      name: 'Logitech Extreme 3D Pro',
    });
  });

  it('gives up on an id without them', () => {
    expect(parsePadId('Mystery Stick')).toBeUndefined();
    expect(parsePadId('')).toBeUndefined();
  });

  it('names a standard pad "pad", a known raw device by identity, and an unknown one "joy"', () => {
    expect(deviceName('standard', 'anything')).toBe('pad');
    expect(deviceName('', 'T.16000M (Vendor: 044f Product: b10a)')).toBe('joy:044f:b10a');
    expect(deviceName('', 'Mystery Stick')).toBe('joy');
  });

  it('lets "joy" cover any raw device and nothing else', () => {
    expect(deviceMatches('pad', 'pad')).toBe(true);
    expect(deviceMatches('joy', 'joy:044f:b10a')).toBe(true);
    expect(deviceMatches('joy', 'joy')).toBe(true);
    expect(deviceMatches('joy:044f:b10a', 'joy:044f:b10a')).toBe(true);
    expect(deviceMatches('joy:044f:b10a', 'joy:046d:c215')).toBe(false);
    expect(deviceMatches('joy:044f:b10a', 'joy')).toBe(false);
    expect(deviceMatches('joy', 'pad')).toBe(false);
    expect(deviceMatches('pad', 'joy')).toBe(false);
  });
});
