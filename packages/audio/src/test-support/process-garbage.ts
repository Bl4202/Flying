/**
 * Measures the garbage `FlightDsp.process` makes, in plain Node (run by the A7 test as a child
 * process, and printable by hand: `node packages/audio/src/test-support/process-garbage.ts`).
 *
 * The allocations are found with V8's sampling heap profiler, which attributes every sampled
 * allocation to the function that made it, so the figure is the bytes made by `process` and what it
 * calls, not the heap's growth (which moves in steps of a page and hides a small allocation made per
 * call). It runs outside the test runner because the runner's module transform reaches every import
 * through a property lookup, which stops V8 inlining a small function across modules: the doubles it
 * returns are then boxed on the heap, about 4.5 KB a block, which the shipped worklet (one bundle)
 * does not make. Not production code.
 */
import { SEEKER_MODE, WARNING } from '@flying/sim-bridge';
import { FlightDsp, P } from '../dsp/flight-dsp.ts';
import { kestrelProfile } from './fixtures.ts';
import { Session } from 'node:inspector/promises';

const dsp = new FlightDsp(kestrelProfile());
const left = new Float32Array(128);
const right = new Float32Array(128);

// States that exercise every voice and every tone (the duel's too), switching every 256 calls.
const states: ((p: Float64Array) => void)[] = [
  (p) => {
    p[P.ENGINE_BITS] = 3;
    p[P.CORE] = 1;
    p[P.AB_LEVEL] = 1;
    p[P.QBAR] = 8000;
    p[P.WARNINGS] = WARNING.STALL;
    p[P.ON_GROUND] = 1;
    p[P.GROUND_SPEED] = 40;
  },
  (p) => {
    p[P.ENGINE_BITS] = 1;
    p[P.CORE] = 0.3;
    p[P.AB_LEVEL] = 0;
    p[P.WARNINGS] = WARNING.PULLUP;
    p[P.ALPHA] = 0.5;
    p[P.ALPHA_WARN] = 0.45;
  },
  (p) => {
    p[P.ENGINE_BITS] = 1;
    p[P.CORE] = 0.06;
    p[P.AB_LEVEL] = 0;
    p[P.WARNINGS] = WARNING.GEAR;
    p[P.COCKPIT] = 0;
  },
  // The duel (AU4): both guns, the growl, and a launch, a hit and an explosion every block.
  (p) => {
    p[P.GUN_FIRING] = 1;
    p[P.ENEMY_GUN_DIST_M] = 400;
    p[P.SEEKER] = SEEKER_MODE.TONE;
    p[P.SIGNAL] = 2.2;
    p[P.LAUNCHES] = (p[P.LAUNCHES] as number) + 1;
    p[P.HITS_TAKEN] = (p[P.HITS_TAKEN] as number) + 1;
    p[P.EXPLOSIONS] = (p[P.EXPLOSIONS] as number) + 1;
    p[P.EXPLOSION_DIST_M] = 800;
  },
  (p) => {
    p[P.GUN_FIRING] = 0;
    p[P.ENEMY_GUN_DIST_M] = 0;
    p[P.SEEKER] = SEEKER_MODE.LOCKED;
  },
];

const step = (i: number): void => {
  (states[(i >> 8) % states.length] as (p: Float64Array) => void)(dsp.params);
  dsp.process(left, right, 128);
};

const WARMUP_CALLS = 30_000;
const SAMPLED_CALLS = 20_000;
/** Bytes between samples: small enough that a 48-byte array made every call is seen thousands of times. */
const SAMPLING_INTERVAL = 32;

for (let i = 0; i < WARMUP_CALLS; i++) step(i);
const session = new Session();
session.connect();
await session.post('HeapProfiler.enable');
// By default the profiler reports only what is still alive when it stops, which leaves out exactly
// the short-lived garbage this is looking for.
await session.post('HeapProfiler.startSampling', {
  samplingInterval: SAMPLING_INTERVAL,
  includeObjectsCollectedByMajorGC: true,
  includeObjectsCollectedByMinorGC: true,
});
for (let i = 0; i < SAMPLED_CALLS; i++) step(i);
const { profile } = await session.post('HeapProfiler.stopSampling');

interface ProfileNode {
  callFrame: { functionName: string; url: string; lineNumber: number };
  selfSize: number;
  children: ProfileNode[];
}
/** What is made inside `FlightDsp.process` and below it (the profiler's own and the loop's are not). */
let total = 0;
const sites = new Map<string, number>();
const walk = (node: ProfileNode, inside: boolean): void => {
  const here =
    inside ||
    (node.callFrame.functionName === 'process' && node.callFrame.url.endsWith('flight-dsp.ts'));
  if (here && node.selfSize > 0) {
    total += node.selfSize;
    const key = `${node.callFrame.functionName || '(anonymous)'} ${node.callFrame.url.split('/').pop() ?? ''}:${String(node.callFrame.lineNumber + 1)}`;
    sites.set(key, (sites.get(key) ?? 0) + node.selfSize);
  }
  for (const child of node.children) walk(child, here);
};
walk(profile.head, false);

console.log(
  JSON.stringify({
    bytesPerCall: total / SAMPLED_CALLS,
    sites: [...sites.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([site, bytes]) => ({ site, bytesPerCall: bytes / SAMPLED_CALLS })),
  }),
);
