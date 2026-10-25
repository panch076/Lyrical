// Talks to the Rust core inside Tauri. Opened in a normal browser (for UI
// development), it falls back to an in-page mock that plays the demo track.

import type { AppState } from "./types";

type Handler = (payload: unknown) => void;

export interface Bridge {
  getState(): Promise<AppState>;
  invoke<T = void>(command: string, args?: Record<string, unknown>): Promise<T>;
  on(event: string, handler: Handler): Promise<void>;
  startDragging(): void;
  startResizing(): void;
}

export const inTauri = "__TAURI_INTERNALS__" in window;

async function tauriBridge(): Promise<Bridge> {
  const [{ invoke }, { listen }, { getCurrentWindow }] = await Promise.all([
    import("@tauri-apps/api/core"),
    import("@tauri-apps/api/event"),
    import("@tauri-apps/api/window"),
  ]);
  return {
    getState: () => invoke<AppState>("get_state"),
    invoke: (command, args) => invoke(command, args),
    on: async (event, handler) => {
      await listen(event, (e) => handler(e.payload));
    },
    startDragging: () => void getCurrentWindow().startDragging(),
    startResizing: () => void getCurrentWindow().startResizeDragging("SouthEast"),
  };
}

let bridgePromise: Promise<Bridge> | null = null;

export function bridge(): Promise<Bridge> {
  bridgePromise ??= inTauri ? tauriBridge() : import("./mock").then((m) => m.mockBridge());
  return bridgePromise;
}
