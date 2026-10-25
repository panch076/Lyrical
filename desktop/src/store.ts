import { bridge, type Bridge } from "./bridge";
import type { ArtworkPayload, AppState, Lyrics, Platform, Settings, Snapshot } from "./types";

type Listener = () => void;

/**
 * Shared client state for every window: the now-playing snapshot, artwork,
 * lyrics and settings, plus a local playback clock that extrapolates the
 * position between the core's twice-a-second updates.
 */
class Store {
  nowPlaying: Snapshot | null = null;
  artwork: ArtworkPayload | null = null;
  lyrics: Lyrics | null = null;
  settings!: Settings;
  platform: Platform = "windows";

  private anchorPosition = 0;
  private anchorTime = 0;
  private rate = 0;
  private graceUntil = 0;
  private listeners = new Map<string, Set<Listener>>();
  private api!: Bridge;

  async init(): Promise<void> {
    this.api = await bridge();
    const state: AppState = await this.api.getState();
    this.platform = state.platform;
    this.settings = state.settings;
    this.artwork = state.artwork;
    this.lyrics = state.lyrics;
    this.applySnapshot(state.nowPlaying, true);

    await Promise.all([
      this.api.on("now-playing", (p) => this.applySnapshot(p as Snapshot | null)),
      this.api.on("artwork", (p) => {
        this.artwork = p as ArtworkPayload | null;
        this.emit("artwork");
      }),
      this.api.on("lyrics", (p) => {
        this.lyrics = p as Lyrics | null;
        this.emit("lyrics");
      }),
      this.api.on("settings-changed", (p) => {
        this.settings = p as Settings;
        this.emit("settings");
      }),
    ]);
  }

  on(topic: "track" | "playback" | "artwork" | "lyrics" | "settings", listener: Listener): void {
    if (!this.listeners.has(topic)) this.listeners.set(topic, new Set());
    this.listeners.get(topic)!.add(listener);
  }

  private emit(topic: string): void {
    this.listeners.get(topic)?.forEach((l) => l());
  }

  /** Current playback position in seconds (without the user's sync offset). */
  time(now = performance.now()): number {
    const t = this.anchorPosition + ((now - this.anchorTime) / 1000) * this.rate;
    const duration = this.nowPlaying?.duration;
    return Math.max(0, duration ? Math.min(t, duration) : t);
  }

  /** Lyric time: playback position shifted by the sync offset. */
  lyricTime(now = performance.now()): number {
    return this.time(now) + (this.settings?.syncOffset ?? 0);
  }

  get isPlaying(): boolean {
    return this.rate > 0;
  }

  private applySnapshot(snapshot: Snapshot | null, initial = false): void {
    const trackChanged = snapshot?.trackId !== this.nowPlaying?.trackId;
    const wasPlaying = this.nowPlaying?.isPlaying;
    this.nowPlaying = snapshot;

    if (!snapshot) {
      this.setAnchor(0, 0);
    } else if (trackChanged || initial || performance.now() > this.graceUntil) {
      const elapsed = snapshot.isPlaying ? (Date.now() - snapshot.timestampMs) / 1000 : 0;
      const position = snapshot.position + Math.max(0, elapsed);
      const playing = snapshot.isPlaying;
      const drift = position - this.time();
      if (trackChanged || initial || playing !== this.rate > 0 || Math.abs(drift) > 1.5) {
        // Seeks, track changes and play/pause jump straight to the new spot.
        this.setAnchor(position, playing ? 1 : 0);
      } else if (playing) {
        // Small disagreements are eased out by running the clock slightly fast
        // or slow until the next update, so the sweep never visibly jumps.
        const correction = Math.abs(drift) < 0.03 ? 0 : Math.min(Math.max(drift, -0.15), 0.15);
        this.setAnchor(this.time(), 1 + correction);
      }
    }

    if (trackChanged || initial) this.emit("track");
    if (trackChanged || initial || wasPlaying !== snapshot?.isPlaying) this.emit("playback");
  }

  private setAnchor(position: number, rate: number): void {
    this.anchorPosition = position;
    this.anchorTime = performance.now();
    this.rate = rate;
  }

  // MARK: Commands

  playPause(): void {
    if (!this.nowPlaying?.canControl) return;
    this.setAnchor(this.time(), this.rate > 0 ? 0 : 1);
    this.graceUntil = performance.now() + 1200;
    this.emit("playback");
    void this.api.invoke("media_command", { action: "playPause" });
  }

  next(): void {
    void this.api.invoke("media_command", { action: "next" });
  }

  previous(): void {
    void this.api.invoke("media_command", { action: "previous" });
  }

  seek(seconds: number): void {
    if (!this.nowPlaying?.canSeek) return;
    this.setAnchor(Math.max(0, seconds), this.rate > 0 ? 1 : 0);
    this.graceUntil = performance.now() + 1200;
    void this.api.invoke("media_command", { action: "seek", position: seconds });
  }

  updateSettings(patch: Partial<Settings>): void {
    this.settings = { ...this.settings, ...patch };
    this.emit("settings");
    void this.api.invoke("update_settings", { patch });
  }

  invoke<T = void>(command: string, args?: Record<string, unknown>): Promise<T> {
    return this.api.invoke<T>(command, args);
  }

  startDragging(): void {
    this.api.startDragging();
  }

  startResizing(): void {
    this.api.startResizing();
  }
}

export const store = new Store();
