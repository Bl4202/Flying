/**
 * The menus over the flight: the pause menu and, from it, the binding editor (GDD 53, 58.5).
 *
 * Opening a menu holds the simulation, silences the flight controls (the pump's `menu` context:
 * the sticks centre and the throttle stays where it was), and gives the pointer back. Closing the
 * last one undoes all three. A change made in a menu is applied at once and stored.
 *
 * What the URL set for this visit (`?assist=`, `?spawn=`, `?g=`) is shown in the menu in place of
 * the stored setting, so the menu says what is true; once the player changes that setting, the
 * player's choice stands.
 */
import type { ControlsProfile, GamepadHub, InputPump } from '@flying/input';
import { BUTTON } from '@flying/sim-bridge';
import {
  BindingEditor,
  type ControlsStore,
  type DemoMenu,
  EditorModel,
  PauseMenu,
  type Settings,
  type SettingsPatch,
  type SettingsStore,
} from '@flying/ui';

/** The settings a URL flag can set for one visit. */
export type SettingsOverrides = Partial<Pick<Settings, 'spawn' | 'assist' | 'gEffects'>>;

export interface MenuControllerDeps {
  root: HTMLElement;
  settings: SettingsStore;
  store: ControlsStore;
  pads: GamepadHub;
  pump: InputPump;
  /** What the URL set for this visit. The controller drops an entry once the player changes it. */
  overrides: SettingsOverrides;
  /** The assist level the aircraft is flying under now (0 if none). */
  assistInForce: () => number;
  /** Holds the simulation while a menu is open, and releases it. */
  hold: (on: boolean) => void;
  /** Tells the application state which menu is open. */
  setMenu: (menu: DemoMenu) => void;
  /** Gives the pointer back when a menu opens, takes it again (if the mouse flies) when it closes. */
  releaseMouse: () => void;
  syncMouseLock: () => void;
  /** The player chose or edited a profile: fly with it. */
  onProfile: (profile: ControlsProfile) => void;
  /** The player changed settings in a menu: apply them. `next` is every setting now. */
  onSettings: (patch: SettingsPatch, next: Readonly<Settings>) => void;
  nowMs: () => number;
}

export class MenuController {
  readonly #d: MenuControllerDeps;
  readonly #pause: PauseMenu;
  #editor: BindingEditor | undefined;
  #menu: DemoMenu = null;

  constructor(deps: MenuControllerDeps) {
    this.#d = deps;
    this.#pause = new PauseMenu(deps.root, {
      settings: () => this.settings(),
      assistInForce: deps.assistInForce,
      onChange: (patch) => {
        this.change(patch);
      },
      onResume: () => {
        this.close();
      },
      onControls: () => {
        this.openControls();
      },
      onRespawn: () => {
        this.respawn();
      },
    });
  }

  /** Which menu is open. */
  get current(): DemoMenu {
    return this.#menu;
  }

  get isOpen(): boolean {
    return this.#menu !== null;
  }

  /** The settings now: the stored ones, with this visit's URL settings over them. */
  settings(): Readonly<Settings> {
    return { ...this.#d.settings.value, ...this.#d.overrides };
  }

  /** The pause key: opens the pause menu, steps back from the editor, or resumes. */
  toggle(): void {
    if (this.#menu === null) this.openPause();
    else if (this.#menu === 'controls') this.openPause();
    else this.close();
  }

  openPause(): void {
    if (this.#menu === 'pause') return;
    this.#closeEditor();
    this.#enter();
    this.#pause.open();
    this.#set('pause');
  }

  openControls(): void {
    if (this.#menu === 'controls') return;
    this.#enter();
    this.#pause.close();
    const d = this.#d;
    const model = new EditorModel(
      {
        store: d.store,
        hub: d.pads,
        onProfile: d.onProfile,
        nowMs: d.nowMs,
      },
      d.pump.profile.id,
    );
    this.#editor = new BindingEditor(d.root, {
      model,
      hub: d.pads,
      onClose: () => {
        this.openPause();
      },
    });
    this.#editor.open();
    this.#set('controls');
  }

  /** Closes whatever is open and flies on. */
  close(): void {
    this.#closeEditor();
    this.#pause.close();
    if (this.#menu === null) return;
    this.#d.pump.setContext('flight');
    this.#d.hold(false);
    this.#set(null);
    this.#d.syncMouseLock();
  }

  /** Flies on from a fresh spawn (the menu's respawn). */
  respawn(): void {
    this.close();
    this.#d.pump.pressOnce(BUTTON.RESET);
  }

  /** The player changed a setting in a menu: store it and apply it. */
  change(patch: SettingsPatch): void {
    this.persist(patch);
    this.#d.onSettings(patch, this.settings());
  }

  /** Stores a change the controls already made (the assist or mouse key). */
  persist(patch: SettingsPatch): void {
    const overrides = this.#d.overrides;
    if (patch.spawn !== undefined) delete overrides.spawn;
    if (patch.assist !== undefined) delete overrides.assist;
    if (patch.gEffects !== undefined) delete overrides.gEffects;
    void this.#d.settings.update(patch);
  }

  destroy(): void {
    this.#closeEditor();
    this.#pause.destroy();
  }

  #enter(): void {
    if (this.#menu !== null) return;
    this.#d.pump.setContext('menu');
    this.#d.releaseMouse();
    this.#d.hold(true);
  }

  #closeEditor(): void {
    this.#editor?.destroy();
    this.#editor = undefined;
  }

  #set(menu: DemoMenu): void {
    this.#menu = menu;
    this.#d.setMenu(menu);
  }
}
