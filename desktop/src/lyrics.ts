import type { Line } from "./types";

/** Index of the last line at or before `time`, or -1 before the first line. */
export function activeIndex(lines: Line[], time: number): number {
  if (!lines.length || time < lines[0].time) return -1;
  let low = 0;
  let high = lines.length - 1;
  while (low < high) {
    const mid = (low + high + 1) >> 1;
    if (lines[mid].time <= time) low = mid;
    else high = mid - 1;
  }
  return low;
}

/**
 * How far through singing line `index` we are (0 = not started, ≥1 = done).
 * Lyrics are timed per line, so the sweep is paced by text length and capped
 * by when the next line starts. Matches the macOS app.
 */
export function sweepProgress(lines: Line[], index: number, time: number): number {
  const line = lines[index];
  const next = index + 1 < lines.length ? lines[index + 1].time : line.time + 5;
  const span = Math.min(next - line.time, line.text.length * 0.085 + 0.9);
  return span > 0 ? (time - line.time) / span : 1;
}

/**
 * Renders `text` as word spans that can light up one after another. The lit
 * edge travels along the words in reading order, so wrapped lines fill line by
 * line rather than all at once.
 */
export function renderSungText(element: HTMLElement, text: string): void {
  element.textContent = "";
  for (const part of text.split(/(\s+)/)) {
    if (!part) continue;
    if (/^\s+$/.test(part)) {
      element.append(document.createTextNode(part));
    } else {
      const word = document.createElement("span");
      word.className = "w";
      word.textContent = part;
      element.append(word);
    }
  }
}

/** Half-width of the soft lit edge, in ems. Keep in sync with `--feather` in base.css. */
const FEATHER_EM = 0.45;

interface SungLayout {
  version: number;
  words: { element: HTMLElement; start: number }[];
  /** Length of the reading path in px, from the first glyph to the last. */
  length: number;
  feather: number;
}

// Word positions depend on fonts and sizes, so re-measure after either changes.
let layoutVersion = 0;
const invalidate = () => void layoutVersion++;
window.addEventListener("resize", invalidate);
document.fonts?.addEventListener("loadingdone", invalidate);

const layouts = new WeakMap<HTMLElement, SungLayout>();

/**
 * Lays the words out on one continuous path in px: each word starts where the
 * previous one ended plus the gap between them, so the edge glides through the
 * spaces at a steady speed instead of jumping from word to word.
 */
function measure(element: HTMLElement): SungLayout {
  const cached = layouts.get(element);
  if (cached && cached.version === layoutVersion) return cached;
  const feather = parseFloat(getComputedStyle(element).fontSize) * FEATHER_EM || 10;
  const words: SungLayout["words"] = [];
  let length = 0;
  let previous: HTMLElement | null = null;
  for (const word of element.querySelectorAll<HTMLElement>(".w")) {
    if (previous) {
      const sameLine = Math.abs(word.offsetTop - previous.offsetTop) < 2;
      // A wrapped word continues the path; count a space-sized gap for the jump.
      length += sameLine ? Math.max(0, word.offsetLeft - (previous.offsetLeft + previous.offsetWidth)) : feather;
    }
    words.push({ element: word, start: length });
    length += word.offsetWidth;
    previous = word;
  }
  const layout = { version: layoutVersion, words, length, feather };
  layouts.set(element, layout);
  return layout;
}

const lastEdge = new WeakMap<HTMLElement, string>();

export function setSungProgress(element: HTMLElement, progress: number): void {
  const { words, length, feather } = measure(element);
  // Start and end one feather past the text so the edge fully clears both ends.
  const edge = -feather + Math.min(Math.max(progress, 0), 1) * (length + feather * 2);
  for (const { element: word, start } of words) {
    const local = Math.min(Math.max(edge - start, -feather * 2), word.offsetWidth + feather * 2);
    const value = `${local.toFixed(1)}px`;
    if (lastEdge.get(word) !== value) {
      word.style.setProperty("--x", value);
      lastEdge.set(word, value);
    }
  }
}

export function clearSungProgress(element: HTMLElement): void {
  for (const word of element.querySelectorAll<HTMLElement>(".w")) {
    word.style.removeProperty("--x");
    lastEdge.delete(word);
  }
}

export function formatTime(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
