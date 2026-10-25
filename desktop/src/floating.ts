import { icons } from "./icons";
import { activeIndex, clearSungProgress, renderSungText, setSungProgress, sweepProgress } from "./lyrics";
import { accent, extractPalette, loadImage, rgb } from "./palette";
import { store } from "./store";
import type { Line } from "./types";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const panel = $("panel");
const art = $<HTMLImageElement>("art");
const source = $("source");
const meta = $("meta");
const roller = $("roller");
const track = $("track");
const message = $("message");
const playButton = $("play");
const nextButton = $("next");

/** Row -1 is a virtual intro row shown before the first line starts. */
const INTRO = -1;
const WINDOW = 3;

let lines: Line[] = [];
let rows = new Map<number, HTMLElement>();
let focus: number | null = null;

function rowHeight(): number {
  return Math.round(32 * store.settings.fontScale);
}

// MARK: Settings

function applySettings(): void {
  const s = store.settings;
  const body = document.body;
  for (const style of ["acrylic", "frosted", "clear", "tinted"]) body.classList.toggle(`glass-${style}`, s.glass === style);
  body.classList.toggle("show-art", s.showArtwork);
  body.classList.toggle("single-line", !s.surroundingLines);
  body.style.setProperty("--scale", String(s.fontScale));
  body.style.setProperty("--row", `${rowHeight()}px`);
  body.style.setProperty("--rows", s.surroundingLines ? "3" : "1");
  rebuild();
}

// MARK: Now playing

function applyTrack(): void {
  const np = store.nowPlaying;
  meta.textContent = np ? (np.artist ? `${np.title} — ${np.artist}` : np.title) : "";
  source.textContent = np?.source ?? "";
  playButton.hidden = nextButton.hidden = !np?.canControl;
  updateMessage();
}

function applyPlayback(): void {
  playButton.innerHTML = store.isPlaying ? icons.pause : icons.play;
}

async function applyArtwork(): Promise<void> {
  const url = store.artwork?.trackId === store.nowPlaying?.trackId ? store.artwork?.dataUrl : null;
  if (!url) {
    art.removeAttribute("src");
    return;
  }
  art.src = url;
  try {
    const palette = extractPalette(await loadImage(url));
    document.body.style.setProperty("--accent", rgb(accent(palette)));
  } catch {
    /* keep previous accent */
  }
}

function updateMessage(): void {
  const lyrics = store.lyrics;
  let text = "";
  if (!store.nowPlaying) text = "♪  Play something to see lyrics";
  else if (!lyrics || lyrics.trackId !== store.nowPlaying.trackId || lyrics.state === "loading") text = "Finding lyrics…";
  else if (lyrics.state === "plain") text = "Lyrics for this song aren't time-synced";
  else if (lyrics.state === "unavailable") text = "No lyrics found for this song";
  else if (lyrics.state === "instrumental") text = "Instrumental";
  message.textContent = text;
  document.body.classList.toggle("has-message", text !== "");
}

function applyLyrics(): void {
  const lyrics = store.lyrics;
  lines = lyrics?.state === "synced" && lyrics.trackId === store.nowPlaying?.trackId ? lyrics.lines : [];
  updateMessage();
  rebuild();
}

// MARK: Roller

function rebuild(): void {
  track.textContent = "";
  rows = new Map();
  focus = null;
}

function makeRow(index: number): HTMLElement {
  const row = document.createElement("div");
  row.className = "row";
  row.style.top = `${index * rowHeight()}px`;
  if (index === INTRO || !lines[index].text) {
    row.innerHTML = '<div class="dots"><span></span><span></span><span></span></div>';
  } else {
    const text = document.createElement("div");
    text.className = "text sung";
    renderSungText(text, lines[index].text);
    row.append(text);
  }
  return row;
}

/** Shrinks a long line (down to 55%) so it fits on one row, like a label's minimum scale factor. */
function fit(row: HTMLElement): void {
  const text = row.querySelector<HTMLElement>(".text");
  if (!text) return;
  text.style.fontSize = "";
  const available = row.clientWidth;
  const needed = text.scrollWidth;
  if (needed > available && available > 0) {
    const base = parseFloat(getComputedStyle(text).fontSize);
    text.style.fontSize = `${base * Math.max(0.55, available / needed)}px`;
  }
}

function setFocus(next: number, animate: boolean): void {
  const first = Math.max(INTRO, next - WINDOW);
  const last = Math.min(lines.length - 1, next + WINDOW);
  if (!animate) track.classList.add("no-anim");

  for (const [index, row] of rows) {
    if (index < first || index > last) {
      row.remove();
      rows.delete(index);
    }
  }
  for (let index = first; index <= last; index++) {
    let row = rows.get(index);
    if (!row) {
      row = makeRow(index);
      track.append(row);
      rows.set(index, row);
      fit(row);
    }
    row.classList.toggle("focus", index === next);
    row.classList.toggle("past", index < next);
    row.classList.toggle("future", index > next);
    if (index !== next) {
      const text = row.querySelector<HTMLElement>(".text");
      if (text) clearSungProgress(text);
    }
  }

  const height = roller.clientHeight;
  const offset = (height - rowHeight()) / 2 - next * rowHeight();
  track.style.transform = `translateY(${offset}px)`;

  if (!animate) {
    void track.offsetHeight;
    track.classList.remove("no-anim");
  }
  focus = next;
}

function frame(): void {
  if (lines.length) {
    const time = store.lyricTime();
    const index = activeIndex(lines, time);
    const next = index === -1 ? INTRO : index;
    if (next !== focus) setFocus(next, focus !== null);

    const text = rows.get(next)?.querySelector<HTMLElement>(".text");
    if (text) {
      const progress = store.settings.sweep && index >= 0 ? sweepProgress(lines, index, time) : 1;
      setSungProgress(text, progress);
    }
  }
  requestAnimationFrame(frame);
}

// MARK: Interaction

panel.addEventListener("mousedown", (event) => {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, .grip")) return;
  store.startDragging();
});

$("grip").addEventListener("mousedown", (event) => {
  event.preventDefault();
  store.startResizing();
});

playButton.addEventListener("click", () => store.playPause());
nextButton.addEventListener("click", () => store.next());
$("fullscreen").addEventListener("click", () => void store.invoke("show_fullscreen"));
$("hide").addEventListener("click", () => void store.invoke("set_floating_visible", { visible: false }));

nextButton.innerHTML = icons.next;
$("fullscreen").innerHTML = icons.expand;
$("hide").innerHTML = icons.close;

let resizeTimer = 0;
new ResizeObserver(() => {
  clearTimeout(resizeTimer);
  resizeTimer = window.setTimeout(() => {
    rows.forEach(fit);
    if (focus !== null) setFocus(focus, false);
  }, 60);
}).observe(roller);

// MARK: Start

await store.init();
store.on("settings", applySettings);
store.on("track", () => {
  applyTrack();
  void applyArtwork();
});
store.on("playback", applyPlayback);
store.on("artwork", () => void applyArtwork());
store.on("lyrics", applyLyrics);

// Apply the initial look without transitions, then enable them.
document.body.classList.add("no-anim");
applySettings();
applyTrack();
applyPlayback();
void applyArtwork();
applyLyrics();
requestAnimationFrame(() => document.body.classList.remove("no-anim"));
requestAnimationFrame(frame);
