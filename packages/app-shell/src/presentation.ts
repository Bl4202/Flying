/**
 * Loads the aircraft's presentation (ADR-43): the HUD, cockpit, exterior model and stroke font for the
 * renderer and the audio profile for the sound, from the bundle baked beside the sim pack. The bundle
 * names the sim pack it was baked with; it is checked against the pack that was loaded, so presentation
 * data from another build is refused rather than drawn.
 *
 * Nothing here is fatal. If the bundle is missing, malformed or from another build, the game still
 * flies, without a HUD, a cockpit, a model or engine sound, and says why (`error`).
 */
import type { AudioProfile } from '@flying/audio';
import { audioProfileFor } from '@flying/audio';
import {
  cockpitView,
  renderPresentation,
  type CockpitView,
  type RenderPresentation,
} from '@flying/render';
import { checkBundleMatchesPack, PresentError, type PresentBundle } from '@flying/sim-bridge';

/** The only aircraft there is in v0.1c. */
export const AIRCRAFT_ID = 'core:ac/kestrel';

export interface LoadedPresentation {
  render: RenderPresentation | undefined;
  cockpit: CockpitView | undefined;
  audio: AudioProfile | undefined;
  /** Why there is none, if there is none. */
  error: string | undefined;
}

export interface PresentationSources {
  /** Fetches and parses the presentation bundle. */
  bundle: () => Promise<PresentBundle>;
  /** The sim pack's bytes, as the Sim Worker loaded them. */
  pack: () => Promise<Uint8Array>;
}

export async function loadPresentation(
  sources: PresentationSources,
  aircraftId: string = AIRCRAFT_ID,
): Promise<LoadedPresentation> {
  try {
    const [bundle, pack] = await Promise.all([sources.bundle(), sources.pack()]);
    checkBundleMatchesPack(bundle, pack);
    const render = renderPresentation(bundle, aircraftId);
    return {
      render,
      cockpit: render === undefined ? undefined : cockpitView(render.cockpit),
      audio: audioProfileFor(bundle, aircraftId),
      error: render === undefined ? `no presentation for ${aircraftId}` : undefined,
    };
  } catch (e) {
    const error =
      e instanceof PresentError
        ? `${e.pointer}: ${e.message}`
        : e instanceof Error
          ? e.message
          : String(e);
    return { render: undefined, cockpit: undefined, audio: undefined, error };
  }
}
