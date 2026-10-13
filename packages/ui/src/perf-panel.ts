/**
 * The panel for an in-app performance run (`?perf=ps0`): progress while it runs, then the gates
 * and the whole report as JSON in a `<pre>` with Copy and Download, which is how a report gets
 * off a reference machine and into the repository.
 */
import type { PerfReport, Progress } from '@flying/perf';
import { clear, h } from './dom.ts';
import { BROWSER_REPORT_IO, reportFileName, type ReportIo } from './report-io.ts';
import { t } from './strings.ts';

export interface PerfPanelOptions {
  scenario: string;
  io?: ReportIo;
  /** Called by the Back button; the button is not shown without it. */
  onBack?: () => void;
}

export interface PerfPanelHandle {
  progress(p: Progress): void;
  done(report: PerfReport): void;
  fail(message: string): void;
}

export function mountPerfPanel(root: HTMLElement, options: PerfPanelOptions): PerfPanelHandle {
  const doc = root.ownerDocument;
  const io = options.io ?? BROWSER_REPORT_IO;

  const status = h(doc, 'p', {
    class: 'status',
    testid: 'perf-status',
    data: { state: 'running' },
    attrs: { role: 'status' },
    text: t('perf.waiting'),
  });
  const verdict = h(doc, 'p', { class: 'verdict', testid: 'perf-verdict' });
  const gates = h(doc, 'ul', { class: 'gates', testid: 'perf-gates' });
  const pre = h(doc, 'pre', { class: 'report', testid: 'perf-report' });
  const feedback = h(doc, 'p', { class: 'hint', testid: 'perf-feedback' });
  const buttons = h(doc, 'div', { class: 'buttons' });

  let json = '';
  let filename = 'perf-report.json';
  const copy = h(doc, 'button', {
    class: 'button',
    testid: 'perf-copy',
    text: t('perf.copy'),
    attrs: { type: 'button' },
  });
  copy.disabled = true;
  copy.addEventListener('click', () => {
    void io.copy(json).then((ok) => {
      feedback.textContent = t(ok ? 'perf.copied' : 'perf.copy-failed');
    });
  });
  const download = h(doc, 'button', {
    class: 'button',
    testid: 'perf-download',
    text: t('perf.download'),
    attrs: { type: 'button' },
  });
  download.disabled = true;
  download.addEventListener('click', () => {
    io.download(filename, json);
  });
  buttons.append(copy, download);
  if (options.onBack) {
    const back = h(doc, 'button', {
      class: 'button',
      testid: 'perf-back',
      text: t('perf.back'),
      attrs: { type: 'button' },
    });
    back.addEventListener('click', options.onBack);
    buttons.append(back);
  }

  const section = h(
    doc,
    'section',
    { class: 'panel perf', testid: 'perf-panel' },
    h(doc, 'h2', { text: t('perf.title', { scenario: options.scenario }) }),
    status,
    verdict,
    gates,
    pre,
    buttons,
    feedback,
  );
  clear(root);
  root.append(section);

  return {
    progress: (p) => {
      status.dataset['state'] = 'running';
      status.textContent =
        p.phase === 'waiting'
          ? t('perf.waiting')
          : p.phase === 'warmup'
            ? t('perf.warmup', { elapsed: p.elapsedS, total: Math.round(p.totalS) })
            : p.phase === 'measure'
              ? t('perf.measure', { elapsed: p.elapsedS, total: Math.round(p.totalS) })
              : t('perf.done');
    },
    done: (report) => {
      status.dataset['state'] = 'done';
      status.textContent = t('perf.done');
      const failed = report.gates.filter((g) => !g.pass).length;
      verdict.dataset['verdict'] = report.verdict;
      verdict.textContent =
        report.verdict === 'pass'
          ? t('perf.verdict.pass')
          : t('perf.verdict.fail', { failed, total: report.gates.length });
      clear(gates);
      for (const g of report.gates) {
        gates.append(
          h(doc, 'li', {
            data: { gate: g.id, pass: String(g.pass) },
            text: `${g.rule}: ${g.value.toFixed(3)} (${t(g.pass ? 'perf.gate.pass' : 'perf.gate.fail')})`,
          }),
        );
      }
      json = JSON.stringify(report, null, 2);
      filename = reportFileName(`perf-${report.scenario}`, report.startedAt);
      pre.textContent = json;
      copy.disabled = false;
      download.disabled = false;
    },
    fail: (message) => {
      status.dataset['state'] = 'error';
      status.textContent = t('perf.error', { message });
    },
  };
}
