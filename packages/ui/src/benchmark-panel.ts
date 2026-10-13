/**
 * The panel for the first-run benchmark: which part is running, then the verdict (profile and
 * preset), why, the figures behind it, and the whole report with Copy and Download.
 */
import type { BenchmarkReport, BenchmarkStep, Classification, Tier } from '@flying/perf';
import { clear, h } from './dom.ts';
import { BROWSER_REPORT_IO, reportFileName, type ReportIo } from './report-io.ts';
import { t, type StringKey } from './strings.ts';

/** The sentences that say why the profile is what it is, from the classification alone. */
export function explainClassification(c: Classification): string[] {
  const f = c.facts;
  const params = {
    preset: t(`bench.preset.${c.preset}`),
    profile: t(`bench.profile.${c.profile}`),
    gpuMs: f.gpuHeavyMs === null ? '–' : f.gpuHeavyMs.toFixed(1),
    kticks: f.cpuTicksPerSecond === null ? '–' : Math.round(f.cpuTicksPerSecond / 1000),
    cores: f.cores ?? '–',
    memoryGb: f.memoryGb ?? '–',
  };
  const lines = [t('bench.explain.summary', params), t(`bench.explain.${c.limitedBy}`, params)];
  if (!c.complete) lines.push(t('bench.explain.incomplete'));
  return lines;
}

const PART_KEYS: Record<'cpu' | 'gpu' | 'cores' | 'memory', StringKey> = {
  cpu: 'bench.part.cpu',
  gpu: 'bench.part.gpu',
  cores: 'bench.part.cores',
  memory: 'bench.part.memory',
};

function tierText(tier: Tier | null): string {
  return tier === null ? t('bench.part.unmeasured') : t(`bench.profile.${tier}`);
}

export interface BenchmarkPanelOptions {
  /** Roughly how long the run takes, for the intro line. */
  seconds: number;
  io?: ReportIo;
  onAgain: () => void;
  onBack: () => void;
}

export interface BenchmarkPanelHandle {
  step(step: BenchmarkStep, detail: string): void;
  /** Shows a finished report; `storedAt` marks it as an earlier run loaded from storage. */
  done(report: BenchmarkReport, storedAt?: string): void;
  fail(message: string): void;
}

export function mountBenchmarkPanel(
  root: HTMLElement,
  options: BenchmarkPanelOptions,
): BenchmarkPanelHandle {
  const doc = root.ownerDocument;
  const io = options.io ?? BROWSER_REPORT_IO;

  const status = h(doc, 'p', {
    class: 'status',
    testid: 'bench-status',
    data: { state: 'running' },
    attrs: { role: 'status' },
    text: t('bench.intro', { seconds: options.seconds }),
  });
  const result = h(doc, 'div', { class: 'result', testid: 'bench-result' });
  const pre = h(doc, 'pre', { class: 'report', testid: 'bench-report' });
  const feedback = h(doc, 'p', { class: 'hint', testid: 'bench-feedback' });
  let json = '';
  let filename = 'benchmark-report.json';

  const button = (id: string, label: string, onClick: () => void, disabled = false) => {
    const b = h(doc, 'button', {
      class: 'button',
      testid: id,
      text: label,
      attrs: { type: 'button' },
    });
    b.disabled = disabled;
    b.addEventListener('click', onClick);
    return b;
  };
  const copy = button(
    'bench-copy',
    t('bench.copy'),
    () => {
      void io.copy(json).then((ok) => {
        feedback.textContent = t(ok ? 'bench.copied' : 'bench.copy-failed');
      });
    },
    true,
  );
  const download = button(
    'bench-download',
    t('bench.download'),
    () => {
      io.download(filename, json);
    },
    true,
  );
  const again = button('bench-again', t('bench.again'), options.onAgain, true);
  const back = button('bench-back', t('bench.back'), options.onBack);

  const section = h(
    doc,
    'section',
    { class: 'panel bench', testid: 'bench-panel' },
    h(doc, 'h2', { text: t('bench.title') }),
    status,
    result,
    pre,
    h(doc, 'div', { class: 'buttons' }, copy, download, again, back),
    feedback,
  );
  clear(root);
  root.append(section);

  return {
    step: (step, detail) => {
      status.dataset['state'] = 'running';
      status.textContent =
        detail === '' ? t(`bench.step.${step}`) : `${t(`bench.step.${step}`)} ${detail}`;
    },
    done: (report, storedAt) => {
      const c = report.classification;
      status.dataset['state'] = 'done';
      status.textContent =
        storedAt === undefined ? t('bench.step.done') : t('bench.stored', { when: storedAt });
      clear(result);
      result.append(
        h(doc, 'p', {
          class: 'verdict',
          testid: 'bench-profile',
          data: { profile: c.profile },
          text: `${t('bench.result.profile')}: ${t(`bench.profile.${c.profile}`)}`,
        }),
        h(doc, 'p', {
          class: 'verdict',
          testid: 'bench-preset',
          data: { preset: c.preset },
          text: `${t('bench.result.preset')}: ${t(`bench.preset.${c.preset}`)}`,
        }),
        h(doc, 'p', { testid: 'bench-explanation' }, explainClassification(c).join(' ')),
        h(
          doc,
          'ul',
          { class: 'parts', testid: 'bench-parts' },
          ...(['cpu', 'gpu', 'cores', 'memory'] as const).map((k) =>
            h(doc, 'li', {
              data: { part: k, tier: c.parts[k] ?? 'none' },
              text: `${t(PART_KEYS[k])}: ${tierText(c.parts[k])}`,
            }),
          ),
        ),
      );
      if (report.notes.length > 0) {
        result.append(
          h(doc, 'p', { text: t('bench.notes') }),
          h(
            doc,
            'ul',
            { testid: 'bench-notes' },
            ...report.notes.map((n) => h(doc, 'li', { text: n })),
          ),
        );
      }
      json = JSON.stringify(report, null, 2);
      filename = reportFileName('benchmark', report.startedAt);
      pre.textContent = json;
      copy.disabled = false;
      download.disabled = false;
      again.disabled = false;
    },
    fail: (message) => {
      status.dataset['state'] = 'error';
      status.textContent = t('bench.error', { message });
      again.disabled = false;
    },
  };
}
