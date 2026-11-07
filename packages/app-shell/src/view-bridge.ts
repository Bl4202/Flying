/**
 * The page's side of the view channel (ADR-41): reads what the player is doing with the camera and
 * the HUD (the pump's `ViewInput`) once a frame, keeps the `ViewState` the renderer drives the camera
 * from, and hands it over: written into the shared 64-byte record when the page is isolated, sent as a
 * message when it changes when it is not. None of it reaches the simulation.
 */
import { ViewController, type ViewInput, type ViewSettings } from '@flying/input';
import { VIEW, ViewWriter, type ViewCamera, type ViewState } from '@flying/sim-bridge/view-channel';

export interface ViewBridgeDeps {
  input: ViewInput;
  /** The shared record the render worker reads, if the page is isolated. */
  buffer: SharedArrayBuffer | null;
  /** Without a shared record: tells the renderer the new state. */
  send: (state: ViewState) => void;
  requestFrame: (callback: () => void) => number;
  cancelFrame: (id: number) => void;
  /** The camera changed (a key press, or a setting). */
  onCamera?: (camera: ViewCamera) => void;
}

/** A shared view record, or undefined where there is no shared memory. */
export function makeViewBuffer(shared: boolean): SharedArrayBuffer | null {
  return shared ? new SharedArrayBuffer(VIEW.SIZE) : null;
}

export class ViewBridge {
  readonly controller: ViewController;
  readonly #d: ViewBridgeDeps;
  readonly #writer: ViewWriter | undefined;
  #frame = 0;
  #running = false;
  #camera: ViewCamera;

  constructor(deps: ViewBridgeDeps) {
    this.#d = deps;
    this.controller = new ViewController(deps.input);
    this.#writer = deps.buffer === null ? undefined : new ViewWriter(deps.buffer);
    this.#camera = this.controller.state.camera;
  }

  get state(): Readonly<ViewState> {
    return this.controller.state;
  }

  get camera(): ViewCamera {
    return this.controller.state.camera;
  }

  /** Applies the player's settings and publishes at once. */
  configure(settings: ViewSettings): void {
    this.controller.configure(settings);
    this.#publish(true);
  }

  /** Chooses the camera (a menu choice, `?camera=`), and publishes at once. */
  setCamera(camera: ViewCamera): void {
    this.controller.setCamera(camera);
    this.#publish(true);
  }

  /** Sets the HUD's detail (0 full to 2 minimum), and publishes at once. */
  setDeclutter(level: number): void {
    this.controller.state.hudDeclutter = Math.min(Math.max(Math.trunc(level), 0), 2);
    this.#publish(true);
  }

  start(): void {
    if (this.#running) return;
    this.#running = true;
    this.#publish(true);
    const tick = (): void => {
      if (!this.#running) return;
      this.#publish(false);
      this.#frame = this.#d.requestFrame(tick);
    };
    this.#frame = this.#d.requestFrame(tick);
  }

  stop(): void {
    this.#running = false;
    this.#d.cancelFrame(this.#frame);
  }

  /** Reads the pump's view input and, if the state changed (or `force`), hands it over. */
  #publish(force: boolean): void {
    const changed = this.controller.update();
    if (changed || force) {
      if (this.#writer) this.#writer.write(this.controller.state);
      else this.#d.send(this.controller.state);
    }
    const camera = this.controller.state.camera;
    if (camera !== this.#camera) {
      this.#camera = camera;
      this.#d.onCamera?.(camera);
    }
  }
}
