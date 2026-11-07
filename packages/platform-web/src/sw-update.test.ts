import { describe, expect, it } from 'vitest';
import {
  SKIP_WAITING,
  UpdateWatcher,
  type ContainerLike,
  type InstallingWorkerLike,
  type RegistrationLike,
  type WorkerLike,
} from './sw-update.ts';

class FakeWorker implements WorkerLike, InstallingWorkerLike {
  state = 'installing';
  readonly messages: unknown[] = [];
  #listeners: (() => void)[] = [];
  postMessage(message: unknown): void {
    this.messages.push(message);
  }
  addEventListener(_type: 'statechange', listener: () => void): void {
    this.#listeners.push(listener);
  }
  become(state: string): void {
    this.state = state;
    for (const fn of this.#listeners) fn();
  }
}

class FakeRegistration implements RegistrationLike {
  waiting: FakeWorker | null = null;
  installing: FakeWorker | null = null;
  updates = 0;
  failUpdate = false;
  #found: (() => void)[] = [];
  addEventListener(_type: 'updatefound', listener: () => void): void {
    this.#found.push(listener);
  }
  update(): Promise<void> {
    this.updates += 1;
    return this.failUpdate ? Promise.reject(new Error('offline')) : Promise.resolve();
  }
  /** A newer build is found, installs, and waits. */
  install(): FakeWorker {
    const worker = new FakeWorker();
    this.installing = worker;
    for (const fn of this.#found) fn();
    worker.become('installed');
    this.installing = null;
    this.waiting = worker;
    worker.become('installed');
    return worker;
  }
}

class FakeContainer implements ContainerLike {
  controller: unknown = {};
  #listeners: (() => void)[] = [];
  addEventListener(_type: 'controllerchange', listener: () => void): void {
    this.#listeners.push(listener);
  }
  takeOver(): void {
    for (const fn of this.#listeners) fn();
  }
}

function setup(controlled = true) {
  const reg = new FakeRegistration();
  const container = new FakeContainer();
  container.controller = controlled ? {} : null;
  let reloads = 0;
  const watcher = new UpdateWatcher(reg, container, () => {
    reloads += 1;
  });
  let changes = 0;
  watcher.onChange(() => {
    changes += 1;
  });
  return { reg, container, watcher, reloads: () => reloads, changes: () => changes };
}

describe('UpdateWatcher', () => {
  it('has nothing to offer until a newer build is waiting', () => {
    const r = setup();
    expect(r.watcher.available).toBe(false);
    r.reg.install();
    expect(r.watcher.available).toBe(true);
    expect(r.changes()).toBeGreaterThan(0);
  });

  it('offers nothing on a first visit: the worker that installs is the first, not an update', () => {
    const r = setup(false);
    r.reg.install();
    expect(r.watcher.available).toBe(false);
  });

  it('asks the server on request, and a failure (offline) changes nothing', async () => {
    const r = setup();
    await r.watcher.check();
    expect(r.reg.updates).toBe(1);
    r.reg.failUpdate = true;
    await expect(r.watcher.check()).resolves.toBeUndefined();
    expect(r.watcher.available).toBe(false);
  });

  it('notices a build that was already waiting when the page opened', async () => {
    const reg = new FakeRegistration();
    reg.waiting = new FakeWorker();
    const watcher = new UpdateWatcher(reg, new FakeContainer(), () => undefined);
    await watcher.check();
    expect(watcher.available).toBe(true);
  });

  it('applies only when asked: tells the waiting worker, and reloads once it has taken over', () => {
    const r = setup();
    const worker = r.reg.install();
    expect(worker.messages).toEqual([]);
    expect(r.reloads()).toBe(0);
    r.watcher.apply();
    expect(worker.messages).toEqual([SKIP_WAITING]);
    expect(r.reloads()).toBe(0);
    r.container.takeOver();
    expect(r.reloads()).toBe(1);
    r.container.takeOver(); // never twice
    expect(r.reloads()).toBe(1);
  });

  it('does not reload when a worker takes over that the player did not ask for', () => {
    const r = setup();
    r.reg.install();
    r.container.takeOver();
    expect(r.reloads()).toBe(0);
  });

  it('does nothing when there is nothing to apply', () => {
    const r = setup();
    r.watcher.apply();
    r.container.takeOver();
    expect(r.reloads()).toBe(0);
  });

  it('reloads at once if the new worker took over between the offer and the answer', () => {
    const r = setup();
    r.reg.install();
    const stale = r.reg.waiting;
    expect(stale).not.toBeNull();
    r.reg.waiting = null;
    // `available` is held by the installed flag until the next check, so the answer still goes through.
    r.watcher.apply();
    expect(r.reloads()).toBe(1);
  });

  it('stops telling a listener that left', () => {
    const r = setup();
    let heard = 0;
    const off = r.watcher.onChange(() => {
      heard += 1;
    });
    r.reg.install();
    const before = heard;
    off();
    r.reg.install();
    expect(heard).toBe(before);
  });
});
