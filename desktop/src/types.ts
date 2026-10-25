export interface Snapshot {
  source: string;
  trackId: string;
  title: string;
  artist: string;
  album: string;
  duration: number | null;
  /** Seconds, valid at `timestampMs`. */
  position: number;
  timestampMs: number;
  isPlaying: boolean;
  canControl: boolean;
  canSeek: boolean;
}

export interface Line {
  time: number;
  text: string;
}

export type LyricsState = "loading" | "synced" | "plain" | "instrumental" | "unavailable";

export interface Lyrics {
  trackId: string;
  state: LyricsState;
  lines: Line[];
  plain: string[];
}

export interface ArtworkPayload {
  trackId: string;
  dataUrl: string | null;
}

export type GlassStyle = "acrylic" | "frosted" | "clear" | "tinted";

export interface Settings {
  glass: GlassStyle;
  showArtwork: boolean;
  surroundingLines: boolean;
  sweep: boolean;
  fontScale: number;
  syncOffset: number;
  clickThrough: boolean;
  floatingVisible: boolean;
}

export type Platform = "windows" | "linux" | "macos";

export interface AppState {
  nowPlaying: Snapshot | null;
  artwork: ArtworkPayload | null;
  lyrics: Lyrics | null;
  settings: Settings;
  platform: Platform;
}
