/**
 * The end of a sortie (ADR-49): the debrief, and from it the feel questionnaire, over a held
 * simulation. It works without the menus (a scripted pilot has none), so a test or the soak can
 * fly the bot's sortie again and again through the debrief's Fly again.
 *
 * Opening holds the simulation and silences the flight controls; nothing here lets it run again,
 * because leaving the debrief is Fly again (a fresh sortie) or Main menu. Rate this flight swaps the
 * debrief for the questionnaire; Save or Skip (or Esc) brings the debrief back. Esc on the debrief
 * itself does nothing: the sortie is over, and the player chooses what comes next.
 */
import {
  Debrief,
  FeelSurvey,
  type DebriefNumbers,
  type DemoMenu,
  type FeelRatings,
  type MissionResult,
} from '@flying/ui';

export interface DebriefFlowDeps {
  root: HTMLElement;
  /** Holds the simulation, silences the flight controls and gives the pointer back. */
  hold: () => void;
  /** Tells the application state which menu is open. */
  setMenu: (menu: DemoMenu) => void;
  onFlyAgain: () => void;
  onMainMenu: () => void;
  /** The player rated the flight: keep the answers. */
  onRated: (ratings: FeelRatings, result: MissionResult) => void;
}

export class DebriefFlow {
  readonly #d: DebriefFlowDeps;
  readonly #debrief: Debrief;
  readonly #feel: FeelSurvey;
  #result: MissionResult | undefined;
  #menu: 'debrief' | 'feel' | null = null;
  /** The debrief's numbers while the questionnaire is over it. */
  #keep: DebriefNumbers | null = null;
  /** Debriefs opened since the page loaded (the soak counts its sorties by them). */
  opened = 0;

  constructor(deps: DebriefFlowDeps) {
    this.#d = deps;
    this.#debrief = new Debrief(deps.root, {
      onFlyAgain: () => {
        deps.onFlyAgain();
      },
      onRate: () => {
        this.#rate();
      },
      onMenu: () => {
        deps.onMainMenu();
      },
    });
    this.#feel = new FeelSurvey(deps.root, {
      onSave: (ratings) => {
        if (this.#result !== undefined) deps.onRated(ratings, this.#result);
        this.#back();
      },
      onSkip: () => {
        this.#back();
      },
    });
  }

  get isOpen(): boolean {
    return this.#menu !== null;
  }

  get current(): 'debrief' | 'feel' | null {
    return this.#menu;
  }

  /** What the debrief shows (while it, or the questionnaire over it, is open), or null. */
  get numbers(): DebriefNumbers | null {
    return this.#menu === 'debrief'
      ? this.#debrief.numbers
      : this.#menu === 'feel'
        ? this.#keep
        : null;
  }

  /** Shows the debrief of `result`, holding the simulation. */
  open(result: MissionResult): void {
    this.#result = result;
    this.#d.hold();
    this.#feel.close();
    this.#debrief.open(result);
    this.#set('debrief');
    this.opened += 1;
  }

  /** Esc: from the questionnaire back to the debrief; on the debrief, nothing. */
  escape(): void {
    if (this.#menu === 'feel') this.#back();
  }

  /** Removes both (the sortie is leaving: Fly again or Main menu). */
  close(): void {
    this.#debrief.close();
    this.#feel.close();
    this.#keep = null;
    if (this.#menu !== null) {
      this.#menu = null;
      this.#d.setMenu(null);
    }
  }

  destroy(): void {
    this.#debrief.destroy();
    this.#feel.destroy();
  }

  #rate(): void {
    this.#keep = this.#debrief.numbers;
    this.#debrief.close();
    this.#feel.open();
    this.#set('feel');
  }

  #back(): void {
    this.#feel.close();
    this.#keep = null;
    if (this.#result !== undefined) this.#debrief.open(this.#result);
    this.#set('debrief');
  }

  #set(menu: 'debrief' | 'feel'): void {
    this.#menu = menu;
    this.#d.setMenu(menu);
  }
}
