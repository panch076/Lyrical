import "@fontsource-variable/inter";
import { icons } from "./icons";
import { activeIndex, clearSungProgress, formatTime, renderSungText, setSungProgress, sweepProgress } from "./lyrics";
import { FALLBACK_PALETTE, extractPalette, loadImage, rgb, type RGB } from "./palette";
import { store } from "./store";
import type { Line } from "./types";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const body = document.body;

const art = $<HTMLImageElement>("art");
const column = $("column");
const lyricsPane = $("lyrics");
const bar = $("bar");
const fill = $("fill");
const elapsed = $("elapsed");
const remaining = $("remaining");
const playButton = $("play");

// MARK: Background

const canvases = [$<HTMLCanvasElement>("bg-a"), $<HTMLCanvasElement>("bg-b")];
let visibleCanvas = 0;
const BG_SIZE = 96;

/**
 * Paints the artwork, heavily blurred, into a tiny canvas. CSS scales the
 * canvas up to cover the screen, so the large blur costs almost nothing and
 * the drift animation stays on the compositor.
 */
function paintBackground(image: HTMLImageElement | null, palette: RGB[]): void {
  const next = 1 - visibleCanvas;
  const canvas = canvases[next];
  canvas.width = canvas.height = BG_SIZE;
  const context = canvas.getContext("2d")!;

  const gradient = context.createLinearGradient(0, 0, BG_SIZE, BG_SIZE);
  gradient.addColorStop(0, rgb(palette[0]));
  gradient.addColorStop(0.5, rgb(palette[4]));
  gradient.addColorStop(1, rgb(palette[8]));
  context.fillStyle = gradient;
  context.fillRect(0, 0, BG_SIZE, BG_SIZE);

  if (image) {
    // WebKitGTK only gained canvas filters recently; check before relying on them.
    if (typeof context.filter === "string") {
      context.filter = "blur(7px) saturate(1.35)";
      context.drawImage(image, -12, -12, BG_SIZE + 24, BG_SIZE + 24);
      context.filter = "none";
    } else {
      // No canvas filters: blur by repeatedly downscaling, then scaling back up.
      const small = document.createElement("canvas");
      small.width = small.height = 12;
      const s = small.getContext("2d")!;
      s.drawImage(image, 0, 0, 12, 12);
      context.imageSmoothingQuality = "high";
      context.drawImage(small, -6, -6, BG_SIZE + 12, BG_SIZE + 12);
    }
  }

  // The very first paint appears instantly; later artwork crossfades.
  if (firstPaint) canvas.classList.add("no-anim");
  canvases[visibleCanvas].classList.remove("visible");
  canvas.classList.add("visible");
  if (firstPaint) {
    void canvas.offsetHeight;
    canvas.classList.remove("no-anim");
    firstPaint = false;
  }
  visibleCanvas = next;
}

let firstPaint = true;

let artworkToken = 0;
/** What the art and background currently show; undefined before the first paint. */
let shownArtwork: string | null | undefined;

async function applyArtwork(): Promise<void> {
  // The previous artwork stays up until the new track's arrives.
  if (store.artworkUrl === undefined && shownArtwork !== undefined) return;
  const url = store.artworkUrl ?? null;
  if (url === shownArtwork) return;
  shownArtwork = url;
  const token = ++artworkToken;
  if (!url) {
    art.removeAttribute("src");
    paintBackground(null, FALLBACK_PALETTE);
    return;
  }
  try {
    const image = await loadImage(url);
    if (token !== artworkToken) return;
    art.src = url;
    paintBackground(image, extractPalette(image));
  } catch {
    paintBackground(null, FALLBACK_PALETTE);
  }
}

// MARK: Now playing

function applyTrack(): void {
  const np = store.nowPlaying;
  $("title").textContent = np?.title ?? "Not Playing";
  $("artist").textContent = np?.artist ?? "";
  $("badge").textContent = np?.source ?? "";
  $("transport").hidden = !np?.canControl;
  $("progress").classList.toggle("static", !np?.canSeek);
  updateEmpty();
}

function applyPlayback(): void {
  playButton.innerHTML = store.isPlaying ? icons.pause : icons.play;
  body.classList.toggle("paused", !!store.nowPlaying && !store.isPlaying);
}

function updateEmpty(): void {
  const lyrics = store.lyrics;
  const current = lyrics && lyrics.trackId === store.nowPlaying?.trackId ? lyrics : null;
  let text = "";
  if (!store.nowPlaying) text = "Play a song in any app to see lyrics";
  else if (!current || current.state === "loading") text = "Finding lyrics…";
  else if (current.state === "unavailable") text = "Lyrics aren't available for this song";
  else if (current.state === "instrumental") text = "Instrumental";
  $("empty").textContent = text;
  const hasLyrics = current?.state === "synced" || current?.state === "plain";
  body.classList.toggle("no-lyrics", !hasLyrics);
  body.classList.toggle("plain-lyrics", current?.state === "plain");
}

// MARK: Lyrics column

const INTRO = -1;
let lines: Line[] = [];
let rowElements: HTMLElement[] = [];
let rowIndices: number[] = [];
let focusPosition = -1;
let userShift = 0;
let scrollTimer = 0;

function applyLyrics(): void {
  const lyrics = store.lyrics;
  const current = lyrics && lyrics.trackId === store.nowPlaying?.trackId ? lyrics : null;
  updateEmpty();

  const plain = $("plain");
  plain.textContent = "";
  if (current?.state === "plain") {
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "These lyrics aren't time-synced";
    plain.append(note);
    for (const text of current.plain) {
      const p = document.createElement("p");
      p.textContent = text || " ";
      plain.append(p);
    }
  }

  lines = current?.state === "synced" ? current.lines : [];
  column.textContent = "";
  rowElements = [];
  rowIndices = [];
  focusPosition = -1;
  userShift = 0;

  const hasIntro = (lines[0]?.time ?? 0) > 3;
  const indices = [...(hasIntro ? [INTRO] : []), ...lines.map((_, i) => i)];
  for (const index of indices) {
    const row = document.createElement("div");
    row.className = "line future";
    if (index === INTRO || !lines[index].text) {
      row.innerHTML = '<div class="dots"><span></span><span></span><span></span></div>';
    } else {
      const text = document.createElement("div");
      text.className = "text sung";
      renderSungText(text, lines[index].text);
      row.append(text);
      row.addEventListener("click", () => {
        if (store.nowPlaying?.canSeek) store.seek(lines[index].time - store.settings.syncOffset + 0.02);
      });
    }
    column.append(row);
    rowElements.push(row);
    rowIndices.push(index);
  }
  if (rowElements.length) layoutRows(0, false);
}

/** Positions every row relative to the focused one. */
function layoutRows(position: number, animate: boolean): void {
  const previous = focusPosition;
  focusPosition = position;
  const anchor = lyricsPane.clientHeight * 0.3;
  const offset = anchor - (rowElements[position]?.offsetTop ?? 0) + userShift;

  rowElements.forEach((row, i) => {
    const distance = i - position;
    const isActive = distance === 0 && (rowIndices[i] !== INTRO || activeLineIndex === -1);
    // Lines from just above the focus downward start progressively later.
    const delay = animate ? Math.min(Math.max(distance + 1, 0), 8) * 40 : 0;
    row.style.transitionDelay = `${delay}ms, 0ms, 0ms, 0ms, 0ms`;
    row.style.transform = `translateY(${offset}px) scale(${isActive ? 1 : 0.95})`;
    row.style.setProperty("--blur", isActive ? "0px" : `${Math.min(Math.abs(distance) * 0.8, 4.5)}px`);
    row.classList.toggle("active", isActive);
    row.classList.toggle("past", distance < 0);
    row.classList.toggle("future", distance > 0);
    if (!isActive && i === previous) {
      const text = row.querySelector<HTMLElement>(".text");
      if (text) clearSungProgress(text);
    }
  });

  if (!animate) {
    column.classList.add("no-anim");
    void column.offsetHeight;
    column.classList.remove("no-anim");
  }
}

let activeLineIndex = -1;

function frame(): void {
  const np = store.nowPlaying;
  const now = performance.now();

  // Progress bar
  const duration = np?.duration ?? 0;
  const time = store.time(now);
  const fraction = dragFraction ?? (duration > 0 ? Math.min(time / duration, 1) : 0);
  fill.style.width = `${fraction * 100}%`;
  const shown = duration > 0 ? fraction * duration : time;
  elapsed.textContent = formatTime(shown);
  remaining.textContent = duration > 0 ? `-${formatTime(duration - shown)}` : "";

  // Lyrics
  if (lines.length) {
    const lyricTime = store.lyricTime(now);
    activeLineIndex = activeIndex(lines, lyricTime);
    const position = Math.max(0, rowIndices.indexOf(activeLineIndex === -1 ? INTRO : activeLineIndex));
    if (position !== focusPosition) layoutRows(position, true);

    if (activeLineIndex >= 0) {
      const text = rowElements[focusPosition]?.querySelector<HTMLElement>(".text");
      if (text) {
        const progress = store.settings.sweep ? sweepProgress(lines, activeLineIndex, lyricTime) : 1;
        setSungProgress(text, progress);
      }
    }
  }
  requestAnimationFrame(frame);
}

// MARK: Interaction

let dragFraction: number | null = null;

bar.addEventListener("pointerdown", (event) => {
  if (!store.nowPlaying?.canSeek || !store.nowPlaying.duration) return;
  bar.setPointerCapture(event.pointerId);
  bar.classList.add("dragging");
  const update = (e: PointerEvent) => {
    const rect = bar.getBoundingClientRect();
    dragFraction = Math.min(Math.max((e.clientX - rect.left) / rect.width, 0), 1);
  };
  update(event);
  const move = (e: PointerEvent) => update(e);
  const up = () => {
    bar.removeEventListener("pointermove", move);
    bar.classList.remove("dragging");
    if (dragFraction !== null && store.nowPlaying?.duration) store.seek(dragFraction * store.nowPlaying.duration);
    dragFraction = null;
  };
  bar.addEventListener("pointermove", move);
  bar.addEventListener("pointerup", up, { once: true });
});

// Scrolling the lyrics pauses the auto-follow briefly, like Apple Music.
lyricsPane.addEventListener(
  "wheel",
  (event) => {
    if (!rowElements.length) return;
    event.preventDefault();
    userShift -= event.deltaY;
    column.classList.add("scrolling");
    layoutRows(focusPosition, false);
    clearTimeout(scrollTimer);
    scrollTimer = window.setTimeout(() => {
      userShift = 0;
      column.classList.remove("scrolling");
      layoutRows(focusPosition, true);
    }, 2500);
  },
  { passive: false },
);

const close = () => void store.invoke("hide_fullscreen");

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") close();
  else if (event.key === " ") {
    event.preventDefault();
    store.playPause();
  } else if (event.key === "ArrowRight" && (event.ctrlKey || event.metaKey)) store.next();
  else if (event.key === "ArrowLeft" && (event.ctrlKey || event.metaKey)) store.previous();
});

$("close").addEventListener("click", close);
$("previous").addEventListener("click", () => store.previous());
$("next").addEventListener("click", () => store.next());
playButton.addEventListener("click", () => store.playPause());

$("close").innerHTML = icons.chevronDown;
$("previous").innerHTML = icons.previous;
$("next").innerHTML = icons.next;

// Hide the close button and cursor after 3 s without movement.
let idleTimer = 0;
const wake = () => {
  body.classList.remove("idle");
  clearTimeout(idleTimer);
  idleTimer = window.setTimeout(() => body.classList.add("idle"), 3000);
};
document.addEventListener("mousemove", wake);
wake();

window.addEventListener("resize", () => {
  if (rowElements.length) layoutRows(focusPosition, false);
});

function applySettings(): void {
  body.style.setProperty("--scale", String(Math.min(Math.max(store.settings.fontScale, 0.8), 1.4)));
  column.classList.toggle("seekable", !!store.nowPlaying?.canSeek);
  if (rowElements.length) requestAnimationFrame(() => layoutRows(focusPosition, false));
}

// MARK: Start

await store.init();
store.on("track", () => {
  applyTrack();
  applySettings();
  void applyArtwork();
});
store.on("playback", applyPlayback);
store.on("artwork", () => void applyArtwork());
store.on("lyrics", applyLyrics);
store.on("settings", applySettings);

applyTrack();
applyPlayback();
applySettings();
void applyArtwork();
applyLyrics();
requestAnimationFrame(frame);
