/** The screen for a failure while running: what stopped, the code, the log tail, and Reload. */
import { clear, h } from './dom.ts';
import { t, type StringKey } from './strings.ts';
import type { MountedScreen } from './compat-screen.ts';

export type FatalSource = 'sim-worker' | 'render-worker' | 'unknown';

const MESSAGE: Record<FatalSource, StringKey> = {
  'sim-worker': 'fatal.sim-worker',
  'render-worker': 'fatal.render-worker',
  unknown: 'fatal.unknown',
};

export interface FatalScreenOptions {
  source: FatalSource;
  code: string;
  /** Technical detail, shown under a heading and selectable so it can be pasted into a report. */
  detail: string;
  onReload: () => void;
}

export function mountFatalScreen(root: HTMLElement, options: FatalScreenOptions): MountedScreen {
  const doc = root.ownerDocument;
  const heading = h(doc, 'h1', { text: t('fatal.title'), attrs: { tabindex: '-1' } });
  const reload = h(doc, 'button', {
    class: 'button primary',
    testid: 'fatal-reload',
    text: t('fatal.reload'),
    attrs: { type: 'button' },
  });
  reload.addEventListener('click', options.onReload);

  const section = h(
    doc,
    'section',
    {
      class: 'screen fatal',
      testid: 'fatal-screen',
      attrs: { role: 'alertdialog', 'aria-modal': 'true', 'aria-label': t('fatal.title') },
      data: { source: options.source },
    },
    heading,
    h(doc, 'p', { testid: 'fatal-message', text: t(MESSAGE[options.source]) }),
    h(doc, 'p', { testid: 'fatal-code', text: t('fatal.code', { code: options.code }) }),
    h(doc, 'h2', { text: t('fatal.detail') }),
    h(doc, 'pre', { class: 'detail', testid: 'fatal-detail', text: options.detail }),
    reload,
  );
  clear(root);
  root.append(section);
  reload.focus();
  return {
    element: section,
    destroy: () => {
      section.remove();
    },
  };
}
