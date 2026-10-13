/** Getting a report out of the page: onto the clipboard, or into a file. */

export interface ReportIo {
  /** Copies `text`; false if the browser refused (no permission, not a secure context). */
  copy(text: string): Promise<boolean>;
  /** Offers `text` as a download named `filename`. */
  download(filename: string, text: string): void;
}

export const BROWSER_REPORT_IO: ReportIo = {
  copy: async (text) => {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      return false;
    }
  },
  download: (filename, text) => {
    const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.append(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  },
};

/** A file name for a report: `perf-ps-0-2026-10-13T12-00-00.json`. */
export function reportFileName(kind: string, startedAt: string): string {
  const stamp = startedAt.replace(/\.\d+Z$/, '').replace(/[:.]/g, '-');
  return `${kind}-${stamp}.json`.toLowerCase();
}
