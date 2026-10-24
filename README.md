# Lyrical
# slop slop slop idc— your discretion

A menu-bar app for macOS 26 that shows synced lyrics for whatever is playing, styled with Liquid Glass.

- **Floating lyrics**: a glass panel that stays above every Space and full-screen app. Lines fill in as they're sung, with the next line shown underneath. Hover to get controls. You can make it click-through from the menu.
- **Full-screen lyrics**: an Apple Music-style player. An animated, blurred artwork background, large artwork, title and artist, a scrubber, glass transport controls, and auto-scrolling lyrics. Click a line to jump to it. `esc` exits, `space` plays/pauses, `⌘←/→` skips tracks.
- **Song detection**:
  - Apple Music and Spotify are read directly, which gives exact position and lets the app control playback.
  - Anything else (browsers, video players…) is identified by ShazamKit from system audio. Shazam also reports the offset into the song, so the lyrics stay in sync.
- Lyrics come from [LRCLIB](https://lrclib.net). Artwork and metadata fall back to the iTunes Search API.

## Build

    ./build.sh            # -> build/Lyrical.app
    open build/Lyrical.app

Requires Xcode 26 and macOS 26.

## Permissions

- **Automation** (Music/Spotify): you're asked on first use.
- **Screen & System Audio Recording**: needed for Shazam. Only audio is used; the video stream is 2×2 px and thrown away.

Ad-hoc builds get a new code signature each time you rebuild, so macOS may ask for these permissions again.

ShazamKit's music catalog may require the app to be signed with an Apple Developer team that has ShazamKit enabled for the bundle ID (`app.lyrical.Lyrical`):

    SIGN_IDENTITY="Apple Development: you@example.com (TEAMID)" ./build.sh

If recognition reports an error, Apple Music and Spotify still work fully.
