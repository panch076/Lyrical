# Lyrical for Windows & Linux

A cross-platform version of Lyrical built with [Tauri](https://tauri.app). It's a standalone desktop app: a small native core written in Rust, with the interface drawn by the system's web engine (WebView2 on Windows, WebKitGTK on Linux).

- **Floating lyrics**: a borderless, always-on-top box. It shows three lines with the current one in the middle and rolls as the song plays; lines fill in as they're sung. Drag it anywhere and resize it from the corner. You can make it click-through.
- **Full-screen lyrics**: blurred-artwork background, artwork, scrubber, and lyrics that scroll with a cascading animation. Click a line to jump to it, scroll to look around. Press `Esc` to exit, `Space` to play/pause, `Ctrl+←/→` to skip.
- **Tray icon**: toggle the floating box, open full screen, turn on click-through, open settings, quit.

## How it finds the song

| Platform | Source | Works with |
|---|---|---|
| Windows 10/11 | System Media Transport Controls (the media flyout) | Spotify, Chrome/Edge (incl. YouTube Music), Apple Music, Media Player, … |
| Linux | MPRIS over D-Bus | Spotify, Chrome/Chromium, Firefox, Rhythmbox, Elisa, VLC, … |

Lyrics come from [LRCLIB](https://lrclib.net). Artwork comes from the player, or from the iTunes Search API when the player doesn't provide any.

## Glass

- **Windows:** Acrylic, the native frosted backdrop with rounded Windows 11 corners. Frosted, Clear and Album Tint styles are also available.
- **Linux:** Frosted, Clear and Album Tint, drawn by the app itself. Linux desktops don't offer a standard blur-behind effect.

On GNOME, apps can't force themselves above other windows. If the floating box falls behind, click it, press `Alt+Space` and choose **Always on Top**.

## Develop

Requires Node 22+ and Rust (stable). Linux additionally needs `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`.

    npm install
    npm run tauri dev        # run the app
    npx vite                 # UI only, in a browser, using the demo track
    npm run tauri build      # installers for the current OS

On machines without a supported media API, or with `LYRICAL_DEMO=1`, the app plays a demo track with placeholder lyrics. Opened in a normal browser, the UI uses the same demo. Add `?t=20&paused=1` to freeze it at a moment.

Windows and Linux installers are built by GitHub Actions (`.github/workflows/desktop.yml`) on every push that touches `desktop/`.
