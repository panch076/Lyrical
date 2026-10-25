// Browser-only stand-in for the Rust core, so the UI can be developed and
// previewed in a normal browser. Mirrors the core's demo track; the lyrics are
// original placeholder text.

import type { Bridge } from "./bridge";
import type { AppState, Line, Settings, Snapshot } from "./types";

const DEMO_LINES: Line[] = [
  [4, "This is a demo track for Lyrical"],
  [8, "Every line here is placeholder text"],
  [12, "Watch the highlight sweep across each word as it plays"],
  [17, "Longer lines wrap onto a second row in full screen and keep filling in order"],
  [23, "Short line"],
  [26, ""],
  [34, "That pause was an instrumental break"],
  [38, "Click any line in full screen to jump to it"],
  [43, "Press space to pause, escape to leave"],
  [48, "The floating box keeps three lines in view"],
  [53, "With the current one centred in the middle"],
  [58, "And rolls to the next as the song moves on"],
  [63, "Play real music to see real lyrics"],
  [68, ""],
  [76, "Back for one last chorus of nothing in particular"],
  [82, "Thanks for testing Lyrical"],
].map(([time, text]) => ({ time: time as number, text: text as string }));

const DURATION = 96;
const TRACK_ID = "demo|Glass Horizon|The Placeholders";

/** Abstract generated cover art (no external assets). */
function demoArtwork(): string {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 600;
  const c = canvas.getContext("2d")!;
  const base = c.createLinearGradient(0, 0, 600, 600);
  base.addColorStop(0, "#ff5f6d");
  base.addColorStop(0.5, "#c2257a");
  base.addColorStop(1, "#3a1c71");
  c.fillStyle = base;
  c.fillRect(0, 0, 600, 600);
  const blob = (x: number, y: number, r: number, color: string) => {
    const g = c.createRadialGradient(x, y, 0, x, y, r);
    g.addColorStop(0, color);
    g.addColorStop(1, "rgba(0,0,0,0)");
    c.fillStyle = g;
    c.fillRect(0, 0, 600, 600);
  };
  blob(430, 170, 260, "rgba(255, 200, 90, 0.85)");
  blob(150, 460, 300, "rgba(60, 140, 255, 0.7)");
  c.strokeStyle = "rgba(255,255,255,0.85)";
  c.lineWidth = 14;
  c.beginPath();
  c.arc(300, 300, 120, 0, Math.PI * 2);
  c.stroke();
  return canvas.toDataURL("image/jpeg", 0.9);
}

export function mockBridge(): Bridge {
  const params = new URLSearchParams(location.search);
  let offset = Number(params.get("t") ?? 0);
  let anchor = performance.now();
  let playing = params.get("paused") !== "1";
  const handlers = new Map<string, ((p: unknown) => void)[]>();
  let settings: Settings = {
    glass: (params.get("glass") as Settings["glass"]) ?? "frosted",
    showArtwork: true,
    surroundingLines: true,
    sweep: true,
    fontScale: 1,
    syncOffset: 0,
    clickThrough: false,
    floatingVisible: true,
  };

  const position = () => {
    const elapsed = playing ? (performance.now() - anchor) / 1000 : 0;
    return (offset + elapsed) % DURATION;
  };
  const snapshot = (): Snapshot => ({
    source: "Demo",
    trackId: TRACK_ID,
    title: "Glass Horizon",
    artist: "The Placeholders",
    album: "Lyrical Demo",
    duration: DURATION,
    position: position(),
    timestampMs: Date.now(),
    isPlaying: playing,
    canControl: true,
    canSeek: true,
  });
  const emit = (event: string, payload: unknown) => handlers.get(event)?.forEach((h) => h(payload));
  setInterval(() => emit("now-playing", snapshot()), 500);

  const artwork = params.get("noart") === "1" ? null : demoArtwork();
  const state = (): AppState => ({
    nowPlaying: snapshot(),
    artwork: { trackId: TRACK_ID, dataUrl: artwork },
    lyrics: { trackId: TRACK_ID, state: "synced", lines: DEMO_LINES, plain: [] },
    settings,
    platform: (params.get("platform") as AppState["platform"]) ?? "windows",
  });

  return {
    getState: async () => state(),
    on: async (event, handler) => {
      if (!handlers.has(event)) handlers.set(event, []);
      handlers.get(event)!.push(handler);
    },
    invoke: async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
      if (command === "media_command") {
        const current = position();
        if (args?.action === "playPause") playing = !playing;
        offset = args?.action === "seek" ? Number(args.position) : args?.action === "playPause" ? current : 0;
        anchor = performance.now();
        emit("now-playing", snapshot());
      } else if (command === "update_settings") {
        settings = { ...settings, ...(args?.patch as Partial<Settings>) };
        emit("settings-changed", settings);
        return settings as T;
      } else if (command === "hide_fullscreen") {
        history.back();
      } else if (command === "show_fullscreen") {
        location.href = "fullscreen.html";
      } else if (command === "get_autostart" || command === "set_autostart") {
        return Boolean(args?.enabled) as T;
      }
      return undefined as T;
    },
    startDragging: () => {},
    startResizing: () => {},
  };
}
