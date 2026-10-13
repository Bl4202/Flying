/**
 * Small helpers for building DOM without `innerHTML` (GDD 61.11): every string goes in through
 * `textContent`, so nothing a log line or an adapter name contains can become markup.
 */

export interface ElementOptions {
  class?: string;
  text?: string;
  /** `data-testid`, the stable handle tests use. */
  testid?: string;
  data?: Readonly<Record<string, string>>;
  attrs?: Readonly<Record<string, string>>;
}

/** Creates an element, applies `options`, and appends `children` (strings become text nodes). */
export function h<K extends keyof HTMLElementTagNameMap>(
  doc: Document,
  tag: K,
  options: ElementOptions = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const el = doc.createElement(tag);
  if (options.class !== undefined) {
    el.className = options.class;
  }
  if (options.text !== undefined) {
    el.textContent = options.text;
  }
  if (options.testid !== undefined) {
    el.dataset['testid'] = options.testid;
  }
  for (const [key, value] of Object.entries(options.data ?? {})) {
    el.dataset[key] = value;
  }
  for (const [key, value] of Object.entries(options.attrs ?? {})) {
    el.setAttribute(key, value);
  }
  for (const child of children) {
    el.append(child);
  }
  return el;
}

/** Removes everything inside `el`. */
export function clear(el: Element): void {
  el.replaceChildren();
}

/** All nodes under `el` (elements and text), the figure the DOM budget counts. */
export function countNodes(el: Node): number {
  let n = 0;
  for (const child of el.childNodes) {
    n += 1 + countNodes(child);
  }
  return n;
}
