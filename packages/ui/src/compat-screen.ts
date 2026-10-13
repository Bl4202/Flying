/**
 * The compat screen: one row per feature with its state in words and a mark (never colour alone),
 * what it means, what to do about it, the modes the game will run in, and a Continue button that
 * has focus when the screen opens. A fatal verdict has no Continue: there is nothing to continue to.
 */
import type { CompatModes, CompatReport, CompatRow, CompatState } from './compat.ts';
import { clear, h } from './dom.ts';
import { t, type StringKey } from './strings.ts';

const MARK: Record<CompatState, string> = { ok: '✓', degraded: '!', missing: '✗' };
const STATE_KEY: Record<CompatState, StringKey> = {
  ok: 'compat.state.ok',
  degraded: 'compat.state.degraded',
  missing: 'compat.state.missing',
};

export interface CompatScreenOptions {
  onContinue: () => void;
  /** Reloads the page (fatal screens offer to check again). */
  onReload?: () => void;
}

export interface MountedScreen {
  readonly element: HTMLElement;
  destroy(): void;
}

function rowElement(doc: Document, row: CompatRow): HTMLElement {
  const need = t(row.required ? 'compat.required' : 'compat.optional');
  const detail = h(doc, 'td', { class: 'detail' }, t(row.detailKey, row.detailParams));
  if (row.remedyKey !== undefined) {
    detail.append(
      h(doc, 'div', {
        class: 'remedy',
        testid: `compat-remedy-${row.id}`,
        text: t('compat.remedy', { text: t(row.remedyKey) }),
      }),
    );
  }
  return h(
    doc,
    'tr',
    { testid: `compat-row-${row.id}`, data: { state: row.state, required: String(row.required) } },
    h(doc, 'th', { attrs: { scope: 'row' }, text: t(row.labelKey) }),
    h(doc, 'td', { class: 'need', text: need }),
    h(doc, 'td', {
      class: 'state',
      testid: `compat-state-${row.id}`,
      text: `${MARK[row.state]} ${t(STATE_KEY[row.state])}`,
    }),
    detail,
  );
}

function modeLines(m: CompatModes): [string, string][] {
  const variant = m.variant === undefined ? 'none' : m.variant;
  return [
    [t('compat.mode.simulation'), t(`compat.mode.variant.${variant}`)],
    [t('compat.mode.snapshots'), t(`compat.mode.channel.${m.snapshotChannel}`)],
    [t('compat.mode.input'), t(`compat.mode.channel.${m.inputChannel}`)],
    [t('compat.mode.render-host'), t(`compat.mode.host.${m.renderHost}`)],
    [t('compat.mode.renderer'), t(`compat.mode.renderer.${m.renderer}`)],
    [t('compat.mode.sleeper'), t(`compat.mode.sleeper.${m.sleeper}`)],
  ];
}

export function mountCompatScreen(
  root: HTMLElement,
  report: CompatReport,
  options: CompatScreenOptions,
): MountedScreen {
  const doc = root.ownerDocument;
  const fatal = report.verdict === 'fatal';

  const heading = h(doc, 'h1', { text: t('compat.title'), attrs: { tabindex: '-1' } });
  const verdict = h(doc, 'p', {
    class: 'verdict',
    testid: 'compat-verdict',
    data: { verdict: report.verdict },
    text: t(`compat.verdict.${report.verdict}`),
  });

  const table = h(
    doc,
    'table',
    { class: 'compat-table', attrs: { 'aria-label': t('compat.title') } },
    h(
      doc,
      'thead',
      {},
      h(
        doc,
        'tr',
        {},
        h(doc, 'th', { attrs: { scope: 'col' }, text: t('compat.column.feature') }),
        h(doc, 'th', { attrs: { scope: 'col' }, text: t('compat.column.need') }),
        h(doc, 'th', { attrs: { scope: 'col' }, text: t('compat.column.state') }),
        h(doc, 'th', { attrs: { scope: 'col' }, text: t('compat.column.detail') }),
      ),
    ),
    h(doc, 'tbody', {}, ...report.rows.map((r) => rowElement(doc, r))),
  );

  const modes = h(
    doc,
    'dl',
    { class: 'modes', testid: 'compat-modes' },
    ...modeLines(report.modes).flatMap(([k, v]) => [
      h(doc, 'dt', { text: k }),
      h(doc, 'dd', { text: v }),
    ]),
  );

  const section = h(
    doc,
    'section',
    {
      class: 'screen compat',
      testid: 'compat-screen',
      attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-label': t('compat.title') },
      data: { verdict: report.verdict },
    },
    heading,
    verdict,
    table,
    h(doc, 'h2', { text: t('compat.modes.title') }),
    modes,
  );

  if (report.codes.length > 0) {
    section.append(
      h(doc, 'h2', { text: t('compat.codes.title') }),
      h(
        doc,
        'ul',
        { class: 'codes', testid: 'compat-codes' },
        ...report.codes.map((c) => h(doc, 'li', { text: c, data: { code: c } })),
      ),
    );
  }

  let focusTarget: HTMLElement = heading;
  if (fatal) {
    if (options.onReload !== undefined) {
      const reload = h(doc, 'button', {
        class: 'button',
        testid: 'compat-reload',
        text: t('compat.reload'),
        attrs: { type: 'button' },
      });
      reload.addEventListener('click', options.onReload);
      section.append(reload);
    }
  } else {
    const go = h(doc, 'button', {
      class: 'button primary',
      testid: 'compat-continue',
      text: t('compat.continue'),
      attrs: { type: 'button' },
    });
    go.addEventListener('click', options.onContinue);
    section.append(go, h(doc, 'p', { class: 'hint', text: t('compat.hint.keyboard') }));
    focusTarget = go;
  }

  clear(root);
  root.append(section);
  focusTarget.focus();

  return {
    element: section,
    destroy: () => {
      section.remove();
    },
  };
}
