/**
 * Keeps the four terrain levels under the aircraft (ADR-40). The Sim Worker holds one centre per
 * level; once the aircraft is further from a level's centre than the level's distance
 * (`TERRAIN_LEVELS`), that level is regenerated around it, snapped so the finer footprints keep
 * their edges on the coarser grid.
 *
 * At most one level is refreshed per check, nearest first. That keeps each check cheap (a fill is
 * about 17 000 terrain samples) and, because the re-centre distances leave room for it, a finer
 * footprint is still inside the coarser one while a refresh is waiting its turn.
 */
import {
  nextLevelCentre,
  TERRAIN_LEVEL_COUNT,
  type Centre,
} from '@flying/sim-bridge/terrain-levels';

/** A level to regenerate and the centre to regenerate it around. */
export interface LevelFill {
  level: number;
  centre: [number, number];
}

export class TerrainFollower {
  readonly #centres: (Centre | undefined)[] = Array.from(
    { length: TERRAIN_LEVEL_COUNT },
    () => undefined,
  );

  /** Forgets every centre: the next checks refill all the levels. */
  reset(): void {
    this.#centres.fill(undefined);
  }

  /** The centre a level was last filled around, or undefined if it has none. */
  centre(level: number): Centre | undefined {
    return this.#centres[level];
  }

  /** Records that `level` was filled around `centre`. */
  filled(level: number, centre: Centre): void {
    this.#centres[level] = centre;
  }

  /** The one level to refresh now for an aircraft at BF `pos`, if any. Does not record it. */
  next(pos: Centre): LevelFill | undefined {
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      const centre = nextLevelCentre(level, this.#centres[level], pos);
      if (centre !== undefined) return { level, centre };
    }
    return undefined;
  }
}
