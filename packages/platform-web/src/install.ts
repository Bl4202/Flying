/**
 * Installing the game as an app (ADR-44). Chromium-based browsers fire `beforeinstallprompt` when the
 * page qualifies and expect it to be held until the person asks; this holds it for the main menu's
 * Install button, and lets go of it when the app is installed or already runs as one.
 */

/** The event the browser fires; not in TypeScript's DOM library. */
export interface BeforeInstallPromptEventLike extends Event {
  prompt(): Promise<void>;
  readonly userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>;
}

export interface InstallTarget {
  addEventListener(
    type: 'beforeinstallprompt' | 'appinstalled',
    listener: (e: Event) => void,
  ): void;
}

export type InstallOutcome = 'accepted' | 'dismissed' | 'unavailable';

export class InstallPrompt {
  readonly #standalone: () => boolean;
  readonly #listeners = new Set<() => void>();
  #event: BeforeInstallPromptEventLike | undefined;

  /** `standalone()` is true when the page already runs as an installed app. */
  constructor(target: InstallTarget, standalone: () => boolean) {
    this.#standalone = standalone;
    target.addEventListener('beforeinstallprompt', (e) => {
      // Keep the browser's own mini-bar away: the menu asks when the person does.
      e.preventDefault();
      this.#event = e as BeforeInstallPromptEventLike;
      this.#changed();
    });
    target.addEventListener('appinstalled', () => {
      this.#event = undefined;
      this.#changed();
    });
  }

  /** An install is on offer: the browser said so, and the app is not already installed. */
  get available(): boolean {
    return this.#event !== undefined && !this.#standalone();
  }

  /** Shows the browser's install dialog. The offer is spent whatever the person answers. */
  async prompt(): Promise<InstallOutcome> {
    const event = this.#event;
    if (event === undefined || this.#standalone()) return 'unavailable';
    this.#event = undefined;
    this.#changed();
    try {
      await event.prompt();
      return (await event.userChoice).outcome;
    } catch {
      return 'dismissed';
    }
  }

  onChange(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  #changed(): void {
    for (const fn of this.#listeners) fn();
  }
}
