/**
 * The results table the self-test pages (`?selftest=renderer`, `?selftest=audio`) fill in. Each row
 * is a cell with a stable test id; `data-result` carries the machine-readable outcome next to the
 * text, and the `status` cell reads `running` and then `done`.
 */
export type RowResult = 'pass' | 'fail' | 'skipped' | 'info';

export interface SelfTestTable {
  row(id: string, label: string, text: string, result: RowResult): void;
  done(): void;
}

export function mountSelfTestTable(root: HTMLElement): SelfTestTable {
  const table = root.querySelector('table');
  if (!table) {
    throw new Error('SYS-010: the self-test table is missing from the page');
  }
  root.hidden = false;
  const status = document.createElement('td');
  status.dataset['testid'] = 'status';
  status.textContent = 'running';
  const head = document.createElement('tr');
  const th = document.createElement('th');
  th.scope = 'row';
  th.textContent = 'Status';
  head.append(th, status);
  table.append(head);
  return {
    row(id, label, text, result) {
      const tr = document.createElement('tr');
      const rowHead = document.createElement('th');
      rowHead.scope = 'row';
      rowHead.textContent = label;
      const td = document.createElement('td');
      td.dataset['testid'] = id;
      td.dataset['state'] = result === 'pass' || result === 'info' ? 'ok' : 'warn';
      td.dataset['result'] = result;
      td.textContent = text;
      tr.append(rowHead, td);
      table.append(tr);
    },
    done() {
      status.textContent = 'done';
    },
  };
}
