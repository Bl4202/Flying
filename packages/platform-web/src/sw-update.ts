/**
 * The page's side of an update (ADR-22 as amended by ADR-44). A new build installs in the background
 * and waits. This watches for that, tells the main menu so ("Update ready — reload"), and, only when
 * the player asks, tells the waiting worker to take over and reloads the page when it has. It never
 * does either by itself, and the menu is the only place that offers it, so a flight is never
 * interrupted.
 */

export interface WorkerLike {
  postMessage(message: unknown): void;
}

export interface InstallingWorkerLike {
  readonly state: string;
  addEventListener(type: 'statechange', listener: () => void): void;
}

export interface RegistrationLike {
  readonly waiting: WorkerLike | null;
  readonly installing: InstallingWorkerLike | null;
  addEventListener(type: 'updatefound', listener: () => void): void;
  update(): Promise<unknown>;
}

export interface ContainerLike {
  /** The worker controlling this page, or null on a first visit. */
  readonly controller: unknown;
  addEventListener(type: 'controllerchange', listener: () => void): void;
}

/** What the page tells the waiting worker. */
export const SKIP_WAITING = { type: 'skip-waiting' } as const;

export class UpdateWatcher {
  readonly #reg: RegistrationLike;
  readonly #container: ContainerLike;
  readonly #reload: () => void;
  readonly #listeners = new Set<() => void>();
  #applying = false;
  #reloaded = false;
  #installed = false;

  constructor(reg: RegistrationLike, container: ContainerLike, reload: () => void) {
    this.#reg = reg;
    this.#container = container;
    this.#reload = reload;
    reg.addEventListener('updatefound', () => {
      const worker = reg.installing;
      worker?.addEventListener('statechange', () => {
        if (worker.state === 'installed') this.#changed();
      });
    });
    container.addEventListener('controllerchange', () => {
      // The first install claims the page too; that is not an update, and nothing reloads for it.
      if (this.#applying && !this.#reloaded) {
        this.#reloaded = true;
        reload();
      }
    });
  }

  /** A newer build is installed and waiting, and this page runs under an older one. */
  get available(): boolean {
    return this.#container.controller !== null && (this.#reg.waiting !== null || this.#installed);
  }

  /** Asks the server whether there is a newer build (it installs in the background if so). */
  async check(): Promise<void> {
    try {
      await this.#reg.update();
    } catch {
      // Offline, or the host is down: the build in use is the one that works.
    }
    this.#changed();
  }

  /** The player chose to update: the waiting worker takes over, and the page reloads under it. */
  apply(): void {
    if (!this.available) return;
    this.#applying = true;
    this.#reg.waiting?.postMessage(SKIP_WAITING);
    // If it took over between the check and now, there is no `controllerchange` still to come.
    if (this.#reg.waiting === null && !this.#reloaded) {
      this.#reloaded = true;
      this.#reload();
    }
  }

  onChange(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  #changed(): void {
    this.#installed = this.#reg.waiting !== null;
    for (const fn of this.#listeners) fn();
  }
}
