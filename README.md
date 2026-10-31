<img width="2560" height="1440" alt="Screenshot 2026-10-24 at 9 28 30 PM" src="https://github.com/user-attachments/assets/0ec47a86-3c12-4f5f-9480-9058445591f6" />
<img width="1053" height="240" alt="image" src="https://github.com/user-attachments/assets/9e044f9a-d9ac-4799-8b45-c2dfe5801dc3" />

# Lyrical
# slop slop slop idc— your discretion

A menu-bar app for macOS that shows synced lyrics for whatever is playing, styled with Liquid Glass.

- **Floating lyrics**: a glass panel that stays above every Space and full-screen app. Lines fill in as they're sung, with the next line shown underneath. Hover to get controls. You can make it click-through from the menu.
- **Full-screen lyrics**: an Apple Music-style player. An animated, blurred artwork background, large artwork, title and artist, a scrubber, glass transport controls, and auto-scrolling lyrics. Click a line to jump to it. `esc` exits, `space` plays/pauses, `⌘←/→` skips tracks.
- **Song detection**:
  - Apple Music and Spotify are read directly, which gives exact position and lets the app control playback.
  - Anything else (browsers, video players…) is identified by ShazamKit from system audio. Shazam also reports the offset into the song, so the lyrics stay in sync.
- Lyrics come from [LRCLIB](https://lrclib.net). On macOS, songs it can't provide synced lyrics for fall back to NetEase Cloud Music, which receives the title and artist. Artwork and metadata fall back to the iTunes Search API.

## Download

- **macOS 14+**: [Lyrical 1.2.0](https://github.com/panch076/Lyrical/releases/tag/v1.2.0)
- **Windows 10/11 and Linux**: [Lyrical for Windows & Linux 1.0.1](https://github.com/panch076/Lyrical/releases/tag/desktop-v1.0.1) (`.exe`, `.msi`, `.deb`, `.rpm`, `.AppImage`)

## Windows & Linux

[`desktop/`](desktop) contains a standalone cross-platform version built with Tauri, with the same floating lyrics, full-screen view and sing-along highlight. It detects songs from any app in the Windows media controls or any Linux player that supports MPRIS (Spotify, Chrome, Edge, Firefox, YouTube Music in a browser, and more). See [desktop/README.md](desktop/README.md) for details and build instructions. GitHub Actions builds the installers on every change.

## Build (macOS)

    ./build.sh            # -> build/Lyrical.app
    open build/Lyrical.app

Runs on **macOS 14 Sonoma and later**. On macOS 26 Tahoe it uses Liquid Glass. On Sonoma and Sequoia it falls back to native frosted materials; the floating panel blurs the desktop behind it. Building requires Xcode 26 (for the macOS 26 SDK).

    UNIVERSAL=1 ./build.sh   # Apple Silicon + Intel, for distribution

## Permissions

- **Automation** (Music/Spotify): you're asked on first use.
- **Screen & System Audio Recording**: needed for Shazam. Only audio is used; the video stream is 2×2 px and thrown away.

Ad-hoc builds get a new code signature each time you rebuild, so macOS may ask for these permissions again.

ShazamKit's music catalog may require the app to be signed with an Apple Developer team that has ShazamKit enabled for the bundle ID (`app.lyrical.Lyrical`):

    SIGN_IDENTITY="Apple Development: you@example.com (TEAMID)" ./build.sh

If recognition reports an error, Apple Music and Spotify still work fully.
