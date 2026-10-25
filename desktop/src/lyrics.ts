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
 * Renders `text` as word spans that can light up one after another. Because
 * each word sweeps independently in reading order, wrapped lines fill line by
 * line rather than all at once.
 */
export function renderSungText(element: HTMLElement, text: string): void {
  element.textContent = "";
  let offset = 0;
  for (const part of text.split(/(\s+)/)) {
    if (!part) continue;
    if (/^\s+$/.test(part)) {
      element.append(document.createTextNode(part));
    } else {
      const word = document.createElement("span");
      word.className = "w";
      word.textContent = part;
      word.dataset.start = String(offset);
      word.dataset.length = String(part.length);
      element.append(word);
    }
    offset += part.length;
  }
  element.dataset.total = String(text.length);
}

const lastProgress = new WeakMap<HTMLElement, string>();

export function setSungProgress(element: HTMLElement, progress: number): void {
  const total = Number(element.dataset.total) || 1;
  const lit = Math.min(Math.max(progress, 0), 1) * total;
  for (const word of element.querySelectorAll<HTMLElement>(".w")) {
    const start = Number(word.dataset.start);
    const length = Number(word.dataset.length);
    const fraction = Math.min(Math.max((lit - start) / length, 0), 1);
    // -10%…110% so the soft edge fully clears both ends of the word.
    const value = `${(fraction * 120 - 10).toFixed(1)}%`;
    if (lastProgress.get(word) !== value) {
      word.style.setProperty("--p", value);
      lastProgress.set(word, value);
    }
  }
}

export function clearSungProgress(element: HTMLElement): void {
  for (const word of element.querySelectorAll<HTMLElement>(".w")) {
    word.style.removeProperty("--p");
    lastProgress.delete(word);
  }
}

export function formatTime(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
