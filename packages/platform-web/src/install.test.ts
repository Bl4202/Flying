import { describe, expect, it } from 'vitest';
import { InstallPrompt, type BeforeInstallPromptEventLike, type InstallTarget } from './install.ts';

class FakeTarget implements InstallTarget {
  #listeners = new Map<string, ((e: Event) => void)[]>();
  addEventListener(
    type: 'beforeinstallprompt' | 'appinstalled',
    listener: (e: Event) => void,
  ): void {
    this.#listeners.set(type, [...(this.#listeners.get(type) ?? []), listener]);
  }
  fire(type: string, e: Event): void {
    for (const fn of this.#listeners.get(type) ?? []) fn(e);
  }
}

function offer(outcome: 'accepted' | 'dismissed' = 'accepted', fail = false) {
  const e = new Event('beforeinstallprompt', {
    cancelable: true,
  }) as BeforeInstallPromptEventLike & {
    prompts: number;
  };
  let prompts = 0;
  Object.defineProperties(e, {
    prompt: {
      value: () => {
        prompts += 1;
        return fail ? Promise.reject(new Error('blocked')) : Promise.resolve();
      },
    },
    userChoice: { value: Promise.resolve({ outcome }) },
    prompts: { get: () => prompts },
  });
  return e;
}

describe('InstallPrompt', () => {
  it('has nothing to offer until the browser says the page can be installed', () => {
    const target = new FakeTarget();
    const install = new InstallPrompt(target, () => false);
    expect(install.available).toBe(false);
    target.fire('beforeinstallprompt', offer());
    expect(install.available).toBe(true);
  });

  it("holds the browser's own prompt back, for the menu to ask", () => {
    const target = new FakeTarget();
    new InstallPrompt(target, () => false);
    const e = offer();
    target.fire('beforeinstallprompt', e);
    expect(e.defaultPrevented).toBe(true);
  });

  it('shows the dialog once and reports what the person chose', async () => {
    const target = new FakeTarget();
    const install = new InstallPrompt(target, () => false);
    const e = offer('accepted');
    target.fire('beforeinstallprompt', e);
    expect(await install.prompt()).toBe('accepted');
    expect(e.prompts).toBe(1);
    // The offer is spent: the browser will not honour a second prompt() on the same event.
    expect(install.available).toBe(false);
    expect(await install.prompt()).toBe('unavailable');
  });

  it('reports a dismissal, and a dialog the browser refused', async () => {
    const target = new FakeTarget();
    const a = new InstallPrompt(target, () => false);
    target.fire('beforeinstallprompt', offer('dismissed'));
    expect(await a.prompt()).toBe('dismissed');
    const b = new InstallPrompt(target, () => false);
    target.fire('beforeinstallprompt', offer('accepted', true));
    expect(await b.prompt()).toBe('dismissed');
  });

  it('goes away once the app is installed, and is never on offer to an app that already runs as one', () => {
    const target = new FakeTarget();
    const install = new InstallPrompt(target, () => false);
    target.fire('beforeinstallprompt', offer());
    target.fire('appinstalled', new Event('appinstalled'));
    expect(install.available).toBe(false);

    const standalone = new InstallPrompt(target, () => true);
    target.fire('beforeinstallprompt', offer());
    expect(standalone.available).toBe(false);
  });

  it('tells a listener when the offer comes and goes', async () => {
    const target = new FakeTarget();
    const install = new InstallPrompt(target, () => false);
    let n = 0;
    const off = install.onChange(() => {
      n += 1;
    });
    target.fire('beforeinstallprompt', offer());
    await install.prompt();
    expect(n).toBe(2);
    off();
    target.fire('beforeinstallprompt', offer());
    expect(n).toBe(2);
  });
});
