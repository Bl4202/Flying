/**
 * The screens outside the flight: the first-run flow, the main menu and the settings (ADR-44).
 * `ScreenController` shows the one the application state names, turns what the player does in them
 * into flow events, and leaves the doing (starting a sortie, applying a setting to the controls,
 * the renderer and the audio) to the demo through `ScreenDeps`.
 *
 * Nothing here knows how a sortie starts: Fly is `onFly`. The controls editor opens over the menu
 * and returns to the screen that opened it.
 */
import { builtinProfile, type BuiltinId, type ControlsProfile } from '@flying/input';
import {
  type ControlsStore,
  type DetectedDevice,
  type DeviceKind,
  FirstRun,
  type FlowEvent,
  type FlowState,
  MainMenu,
  type OptionalRow,
  type Settings,
  type SettingsPatch,
  SettingsScreen,
  type SettingsTab,
} from '@flying/ui';

const PROFILE_FOR: Record<DeviceKind, BuiltinId> = {
  km: 'default-km',
  gamepad: 'gamepad-standard',
  joystick: 'generic-joystick',
};

/** The device a controls profile is for (a custom profile counts as the keyboard unless it says otherwise). */
export function deviceOfProfile(profile: Pick<ControlsProfile, 'deviceClass'>): DeviceKind {
  if (profile.deviceClass === 'gamepad') return 'gamepad';
  if (profile.deviceClass === 'joystick') return 'joystick';
  return 'km';
}

/** What a connected pad is, by its mapping: a standard-mapping pad is a gamepad, anything else a joystick. */
export function kindOfMapping(mapping: string): Exclude<DeviceKind, 'km'> {
  return mapping === 'standard' ? 'gamepad' : 'joystick';
}

export interface ScreenDeps {
  root: HTMLElement;
  /** The settings now (with this visit's URL settings over them). */
  settings: () => Readonly<Settings>;
  /** The player changed settings in a screen: store and apply them. */
  onChange: (patch: SettingsPatch) => void;
  /** An imported set of settings replaces every setting: store and apply them. */
  onReplace: (settings: Settings) => void;
  /** Every setting back to its default: store and apply them. */
  onReset: () => void;
  /** The first-run flow ended (finished or skipped): the setup is done. */
  onSetupDone: () => void;
  flow: { dispatch(event: FlowEvent): FlowState };
  store: ControlsStore;
  /** The connected pads and joysticks. */
  devices: () => readonly DetectedDevice[];
  /** The controls profile in use. */
  profile: () => Pick<ControlsProfile, 'id' | 'name' | 'deviceClass'>;
  /** Fly with this profile (the player chose a device). */
  useProfile: (profile: ControlsProfile) => void;
  /** Open the controls editor over the screens; `back` shows them again. */
  openControls: (back: () => void) => void;
  /** Start a sortie. */
  onFly: () => void;
  save: (name: string, text: string) => void;
  readFile?: (file: File) => Promise<string>;
  info: { storage: string; build: string };
  install?: OptionalRow;
  update?: OptionalRow;
  audio?: OptionalRow;
}

export class ScreenController {
  readonly #d: ScreenDeps;
  readonly menu: MainMenu;
  readonly firstRun: FirstRun;
  readonly settings: SettingsScreen;

  constructor(deps: ScreenDeps) {
    this.#d = deps;
    const root = deps.root;
    this.menu = new MainMenu(root, {
      settings: deps.settings,
      onChange: deps.onChange,
      onFly: deps.onFly,
      onSettings: () => {
        this.#go({ type: 'menu-screen', screen: 'settings' });
      },
      build: deps.info.build,
      ...(deps.install === undefined ? {} : { install: deps.install }),
      ...(deps.update === undefined ? {} : { update: deps.update }),
      ...(deps.audio === undefined ? {} : { audio: deps.audio }),
    });
    this.firstRun = new FirstRun(root, {
      settings: deps.settings,
      onChange: deps.onChange,
      onStep: (step) => {
        this.#go({ type: 'first-run-step', step });
      },
      onFinish: () => {
        deps.onSetupDone();
        this.#go({ type: 'first-run-done' });
      },
      devices: deps.devices,
      device: () => deviceOfProfile(deps.profile()),
      onDevice: (kind) => {
        const profile = builtinProfile(PROFILE_FOR[kind]);
        void deps.store.setActive(profile.id);
        deps.useProfile(profile);
      },
      onCalibrate: () => {
        this.#openControls();
      },
    });
    this.settings = new SettingsScreen(root, {
      settings: deps.settings,
      onChange: deps.onChange,
      onReplace: deps.onReplace,
      onReset: () => {
        deps.onReset();
        this.settings.refresh();
      },
      onClose: () => {
        this.#go({ type: 'menu-screen', screen: 'home' });
      },
      onControls: () => {
        this.#openControls();
      },
      onRunSetup: () => {
        this.#go({ type: 'run-setup' });
      },
      save: deps.save,
      profileName: () => deps.profile().name,
      info: deps.info,
      ...(deps.readFile === undefined ? {} : { readFile: deps.readFile }),
    });
  }

  /** Whether a screen is on show. */
  get active(): boolean {
    return this.menu.isOpen || this.firstRun.isOpen || this.settings.isOpen;
  }

  /** Shows what the application state names, and nothing else. */
  show(state: FlowState): void {
    if (state.name === 'FIRST_RUN') {
      this.menu.close();
      this.settings.close();
      this.firstRun.show(state.step);
    } else if (state.name === 'MAIN_MENU' && state.screen === 'home') {
      this.firstRun.close();
      this.settings.close();
      this.menu.open();
      this.menu.refresh();
    } else if (state.name === 'MAIN_MENU') {
      this.firstRun.close();
      this.menu.close();
      this.settings.open();
    } else {
      this.closeAll();
    }
  }

  closeAll(): void {
    this.firstRun.close();
    this.menu.close();
    this.settings.close();
  }

  /** Esc: back out of the settings; the other screens stay (Esc must not skip a setup by accident). */
  escape(): void {
    if (this.settings.isOpen) this.#go({ type: 'menu-screen', screen: 'home' });
  }

  /** Re-reads what the screens show, for a change made elsewhere (a pad plugged in, an update ready). */
  refresh(): void {
    this.menu.refresh();
    this.firstRun.refresh();
    this.settings.refresh();
  }

  destroy(): void {
    this.firstRun.destroy();
    this.menu.destroy();
    this.settings.destroy();
  }

  #go(event: FlowEvent): void {
    this.show(this.#d.flow.dispatch(event));
  }

  /** Opens the controls editor over the open screen, and shows the screen again when it closes. */
  #openControls(): void {
    const tab: SettingsTab | undefined = this.settings.isOpen ? this.settings.tab : undefined;
    const step = this.firstRun.isOpen ? this.firstRun.step : undefined;
    this.#d.openControls(() => {
      if (step !== undefined) this.firstRun.show(step);
      else if (tab !== undefined) this.settings.open(tab);
    });
    // The editor takes the place of the screen that opened it.
    if (step !== undefined) this.firstRun.close();
    else this.settings.close();
  }
}
