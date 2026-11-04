/**
 * The presentation bundle (`content-out/core.present.json`): HUD layout, cockpit, model, audio
 * profile and stroke font, baked beside the sim pack and never part of it. Fetched and parsed here;
 * each part of the game takes the slice it needs.
 */
import { parsePresentBundle, type PresentBundle } from '@flying/sim-bridge/present';
import presentBundleUrl from '../../../content-out/core.present.json?url';

export { presentBundleUrl };

/** Fetches and parses the bundle. Throws a `PresentError` (file + pointer + hint) if it is malformed. */
export async function loadPresentBundle(url: string = presentBundleUrl): Promise<PresentBundle> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`SYS-021: the presentation bundle returned ${String(response.status)}`);
  }
  return parsePresentBundle(await response.json());
}
