/**
 * The pilot's controls on the page: the keyboard, the mouse under Pointer Lock, the gamepads and
 * joysticks, the profile that maps them, and the pump that turns them into input frames
 * (ADR-37). Nothing here decides what a control does; `InputPump` does, and the sim acts on it.
 *
 * This is the one place the settings reach the pump: the mouse mode and feel, the assist the
 * player asks for, where a respawn goes, and the profile that follows the devices that are
 * connected (a device plugged in or out re-binds within a quarter of a second, AC-58.6).
 */
import {
  type ActionId,
  attachInput,
  type ControlsProfile,
  GamepadHub,
  InputPump,
  KeyboardState,
  MouseDevice,
  type MouseWarning,
  chooseProfile,
} from '@flying/input';
import type { InputSink } from '@flying/sim-bridge';
import { monotonicMs, sharedNowMs } from '@flying/platform-web';
import { isOverlayChord, type ControlsStore, type Settings } from '@flying/ui';

export interface FlightControlsDeps {
  window: Window;
  document: Document;
  /** Where the mouse is captured: pointer events on it, and the element Pointer Lock is taken on. */
  stage: HTMLElement;
  sink: InputSink;
  store: ControlsStore;
  /** Longest gap between frames when nothing changes: 8 ms through the shared ring, else 33. */
  minIntervalMs: number;
  /** An action only the page can carry out: pause, assist cycle, mouse mode, respawn. */
  onAction: (id: ActionId) => void;
  onMouseWarning: (warning: MouseWarning) => void;
  /** Pointer Lock was lost while the mouse was flying the aircraft (Esc, a click away). */
  onPointerLost: () => void;
}

/** How often the connected devices are looked at for a profile change, milliseconds. */
const DEVICE_WATCH_MS = 250;

export class FlightControls {
  readonly keyboard = new KeyboardState();
  readonly pads = new GamepadHub();
  readonly mouse: MouseDevice;
  readonly pump: InputPump;
  readonly #d: FlightControlsDeps;
  readonly #stops: (() => void)[] = [];
  #profileRevision = -1;
  #wasLocked = false;

  constructor(deps: FlightControlsDeps) {
    this.#d = deps;
    // The mouse pokes the pump with each event's time, so the frame it causes is stamped with
    // the event and not with the poll that found it (AC-58.3).
    this.mouse = new MouseDevice({
      target: deps.stage,
      document: deps.document,
      onEvent: (timeStampMs) => {
        this.pump.poke(timeStampMs);
      },
      onWarn: deps.onMouseWarning,
    });
    this.pump = new InputPump({
      sink: deps.sink,
      keyboard: this.keyboard,
      pads: this.pads,
      mouse: this.mouse,
      profile: chooseProfile(deps.store.all(), [], deps.store.activeId),
      nowMs: monotonicMs,
      sharedNowMs,
      minIntervalMs: deps.minIntervalMs,
      onAction: deps.onAction,
    });
  }

  /** Starts listening to the devices. */
  start(): void {
    const { window, document, stage } = this.#d;
    this.followDevices();
    const watch = window.setInterval(() => {
      this.followDevices();
    }, DEVICE_WATCH_MS);
    this.#stops.push(() => {
      window.clearInterval(watch);
    });
    this.#stops.push(
      attachInput(
        { window, document },
        {
          keyboard: this.keyboard,
          pump: this.pump,
          // The overlay's Ctrl+Shift+D chord is the page's, not a roll command.
          swallow: (e) =>
            isOverlayChord({
              code: e.code,
              ctrlKey: e.ctrlKey === true,
              shiftKey: e.shiftKey === true,
              altKey: e.altKey === true,
              metaKey: e.metaKey === true,
              repeat: e.repeat,
            }),
        },
      ),
    );
    this.#stops.push(this.mouse.attach());

    const onLockChange = (): void => {
      const locked = document.pointerLockElement === stage;
      const lost = this.#wasLocked && !locked;
      this.#wasLocked = locked;
      if (lost) this.#d.onPointerLost();
    };
    document.addEventListener('pointerlockchange', onLockChange);
    this.#stops.push(() => {
      document.removeEventListener('pointerlockchange', onLockChange);
    });
    // A click on the picture captures the mouse for aim or stick (it needs a gesture).
    const onClick = (): void => {
      this.syncMouseLock();
    };
    stage.addEventListener('click', onClick);
    this.#stops.push(() => {
      stage.removeEventListener('click', onClick);
    });
  }

  /** Re-binds when a device has come or gone: the player's profile if its devices are there. */
  followDevices(): void {
    this.pads.poll();
    if (this.pads.revision === this.#profileRevision) return;
    this.#profileRevision = this.pads.revision;
    const chosen = chooseProfile(
      this.#d.store.all(),
      this.pads.deviceNames(),
      this.#d.store.activeId,
    );
    if (chosen.id !== this.pump.profile.id) this.pump.setProfile(chosen);
  }

  /** Flies with this profile (the player chose or edited it in the editor). */
  useProfile(profile: ControlsProfile): void {
    this.pump.setProfile(profile);
  }

  /**
   * Takes Pointer Lock when the mouse flies the aircraft and gives it back when it does not. Call
   * it from a click or a key press: the browser refuses a lock without a gesture.
   */
  syncMouseLock(): void {
    const wanted = this.pump.mouseMode !== 'off' && this.pump.context === 'flight';
    if (wanted && !this.mouse.locked) this.mouse.lock();
    else if (!wanted && this.mouse.locked) this.mouse.unlock();
  }

  /** Gives the pointer back (a menu opens over the flight). */
  releaseMouse(): void {
    if (this.mouse.locked) this.mouse.unlock();
  }

  /** Applies all of the player's settings that the controls own. */
  applySettings(s: Readonly<Settings>): void {
    this.pump.setMouseMode(s.mouseMode);
    this.pump.setMouseTuning(s.mouseSensitivity, s.mouseSpring);
    this.pump.setAutoGcas(s.autoGcas);
  }

  /** Applies a change the player made: `patch` is what changed, `next` all the settings now. */
  applyChange(patch: Partial<Omit<Settings, 'version'>>, next: Readonly<Settings>): void {
    if (patch.assist !== undefined) this.pump.requestAssist(patch.assist);
    // The respawn kinds are 0 the session's, 1 the runway, 2 the air; a setting of the spawn is
    // 0 the runway, 1 the air.
    if (patch.spawn !== undefined) this.pump.setRespawnKind(patch.spawn + 1);
    if (patch.mouseMode !== undefined) {
      this.pump.setMouseMode(patch.mouseMode);
      if (patch.mouseMode === 'off') this.releaseMouse();
    }
    if (patch.mouseSensitivity !== undefined || patch.mouseSpring !== undefined) {
      this.pump.setMouseTuning(next.mouseSensitivity, next.mouseSpring);
    }
    if (patch.autoGcas !== undefined) this.pump.setAutoGcas(patch.autoGcas);
  }

  destroy(): void {
    for (const stop of this.#stops.splice(0)) stop();
  }
}
