/**
 * Tools to measure rendered audio in a test: an FFT peak finder and the ITU-R BS.1770-4 loudness
 * meter (K-weighting, gated integrated loudness, true peak). Not production code.
 */
import type { FlightDsp } from '../dsp/flight-dsp.ts';

/** In-place radix-2 FFT of `re` and `im` (length a power of two). */
export function fft(re: Float64Array, im: Float64Array): void {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) {
      [re[i], re[j]] = [re[j] as number, re[i] as number];
      [im[i], im[j]] = [im[j] as number, im[i] as number];
    }
  }
  for (let len = 2; len <= n; len <<= 1) {
    const ang = (-2 * Math.PI) / len;
    const wr = Math.cos(ang);
    const wi = Math.sin(ang);
    for (let i = 0; i < n; i += len) {
      let cr = 1;
      let ci = 0;
      for (let k = 0; k < len / 2; k++) {
        const a = i + k;
        const b = i + k + len / 2;
        const tr = (re[b] as number) * cr - (im[b] as number) * ci;
        const ti = (re[b] as number) * ci + (im[b] as number) * cr;
        re[b] = (re[a] as number) - tr;
        im[b] = (im[a] as number) - ti;
        re[a] = (re[a] as number) + tr;
        im[a] = (im[a] as number) + ti;
        const t = cr * wr - ci * wi;
        ci = cr * wi + ci * wr;
        cr = t;
      }
    }
  }
}

/** The frequency of the strongest peak of `x` between `fromHz` and `toHz` (Hann window, parabolic interpolation). */
export function peakFrequency(
  x: Float32Array,
  sampleRate: number,
  fromHz: number,
  toHz: number,
): number {
  let n = 1;
  while (n * 2 <= x.length) n *= 2;
  const re = new Float64Array(n);
  const im = new Float64Array(n);
  for (let i = 0; i < n; i++)
    re[i] = (x[i] ?? 0) * (0.5 - 0.5 * Math.cos((2 * Math.PI * i) / (n - 1)));
  fft(re, im);
  const mag = (k: number): number => Math.hypot(re[k] as number, im[k] as number);
  const lo = Math.max(Math.floor((fromHz * n) / sampleRate), 1);
  const hi = Math.min(Math.ceil((toHz * n) / sampleRate), n / 2 - 1);
  let best = lo;
  for (let k = lo; k <= hi; k++) if (mag(k) > mag(best)) best = k;
  // Parabolic interpolation on the log magnitude.
  const a = Math.log(mag(best - 1) + 1e-30);
  const b = Math.log(mag(best) + 1e-30);
  const c = Math.log(mag(best + 1) + 1e-30);
  const shift = (0.5 * (a - c)) / (a - 2 * b + c);
  return ((best + (Number.isFinite(shift) ? shift : 0)) * sampleRate) / n;
}

/** Renders `seconds` of audio mono (the left channel) in 128-frame quanta, calling `each` before every block. */
export function render(
  dsp: FlightDsp,
  seconds: number,
  each: (block: number, timeS: number) => void = () => undefined,
  sampleRate = 48_000,
): { left: Float32Array; right: Float32Array } {
  const total = Math.round(seconds * sampleRate);
  const left = new Float32Array(total);
  const right = new Float32Array(total);
  const bl = new Float32Array(128);
  const br = new Float32Array(128);
  for (let at = 0, block = 0; at < total; at += 128, block++) {
    each(block, at / sampleRate);
    const n = Math.min(128, total - at);
    dsp.process(bl, br, n);
    left.set(bl.subarray(0, n), at);
    right.set(br.subarray(0, n), at);
  }
  return { left, right };
}

/** The RMS level of `x[from..to)` in dB relative to full scale. */
export function levelDb(x: ArrayLike<number>, from = 0, to = x.length): number {
  let sum = 0;
  for (let i = from; i < to; i++) sum += (x[i] ?? 0) ** 2;
  return 10 * Math.log10(Math.max(sum / Math.max(to - from, 1), 1e-20));
}

// ---------------------------------------------------------------------------------------------
// ITU-R BS.1770-4.

/** K-weighting at 48 kHz: a high shelf then the RLB high-pass (BS.1770-4 tables 1 and 2). */
const SHELF = {
  b: [1.53512485958697, -2.69169618940638, 1.19839281085285],
  a: [-1.69065929318241, 0.73248077421585],
};
const RLB = { b: [1, -2, 1], a: [-1.99004745483398, 0.99007225036621] };

function biquad(x: Float32Array, f: { b: number[]; a: number[] }): Float64Array {
  const out = new Float64Array(x.length);
  let z1 = 0;
  let z2 = 0;
  const [b0, b1, b2] = f.b as [number, number, number];
  const [a1, a2] = f.a as [number, number];
  for (let i = 0; i < x.length; i++) {
    const v = x[i] ?? 0;
    const y = b0 * v + z1;
    z1 = b1 * v - a1 * y + z2;
    z2 = b2 * v - a2 * y;
    out[i] = y;
  }
  return out;
}

/** K-weights a channel. */
function kWeight(x: Float32Array): Float32Array {
  const a = biquad(x, SHELF);
  const b = biquad(Float32Array.from(a), RLB);
  return Float32Array.from(b);
}

/**
 * Integrated loudness, LUFS, of a stereo signal at 48 kHz: mean square of K-weighted 400 ms blocks
 * overlapping by 75 per cent, an absolute gate at -70 LUFS and a relative gate 10 LU below the mean
 * of the blocks that pass it.
 */
export function integratedLoudness(left: Float32Array, right: Float32Array): number {
  const kl = kWeight(left);
  const kr = kWeight(right);
  const block = 19_200; // 400 ms
  const step = 4_800; // 100 ms
  const powers: number[] = [];
  for (let at = 0; at + block <= kl.length; at += step) {
    let sl = 0;
    let sr = 0;
    for (let i = at; i < at + block; i++) {
      sl += (kl[i] as number) ** 2;
      sr += (kr[i] as number) ** 2;
    }
    powers.push((sl + sr) / block);
  }
  const lufs = (z: number): number => -0.691 + 10 * Math.log10(Math.max(z, 1e-20));
  const passing = powers.filter((z) => lufs(z) > -70);
  if (passing.length === 0) return -Infinity;
  const mean = passing.reduce((s, z) => s + z, 0) / passing.length;
  const relative = lufs(mean) - 10;
  const gated = passing.filter((z) => lufs(z) > relative);
  if (gated.length === 0) return -Infinity;
  return lufs(gated.reduce((s, z) => s + z, 0) / gated.length);
}

/** The true peak, dBTP: the largest sample after 4x oversampling with a windowed-sinc interpolator. */
export function truePeakDb(x: Float32Array): number {
  const taps = 24; // per side, at the oversampled rate's input spacing
  const factor = 4;
  let peak = 0;
  for (let i = taps; i < x.length - taps; i++) {
    for (let phase = 0; phase < factor; phase++) {
      let y = 0;
      for (let k = -taps + 1; k <= taps; k++) {
        const t = k - phase / factor;
        const sinc = t === 0 ? 1 : Math.sin(Math.PI * t) / (Math.PI * t);
        // A Blackman window over the taps.
        const w =
          0.42 -
          0.5 * Math.cos((2 * Math.PI * (t + taps)) / (2 * taps)) +
          0.08 * Math.cos((4 * Math.PI * (t + taps)) / (2 * taps));
        y += (x[i + k - 1] ?? 0) * sinc * w;
      }
      peak = Math.max(peak, Math.abs(y));
    }
  }
  for (let i = 0; i < x.length; i++) peak = Math.max(peak, Math.abs(x[i] ?? 0));
  return 20 * Math.log10(Math.max(peak, 1e-10));
}
