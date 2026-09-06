# Changelog

CI takes the `##` section that matches `Cargo.toml`.

## 0.1.8

- Compare two clips with a wipe or side by side (B from file or Video2X)
- Optional Video2X download (one-time prompt), GPU pick, upscale or frame interpolation

## 0.1.7

- `--intro-video` records a ~30s scripted tour with captions on the preview
- Keep volume when you open another video or restart the app

## 0.1.6

- Restart play from the start when decode hits EOF before the probed last frame
- Write `crash.log` next to `lang` on panic (`%LOCALAPPDATA%\vfx-editor`)
- About window follows light/dark theme; dark text contrast
- In-app log (L)

## 0.1.5

- Play at the last frame seeks back to the start

## 0.1.4

- Elide long paths in the top bar

## 0.1.3

- Tag fix (previous tag was meant to be 0.1.3)

## 0.1.2

- In-place updater with progress and success dialogs
- Bundled yt-dlp and quality picker

## 0.1.1

- Transport UX, focus/wave modes, About window

## 0.1

- First Windows release, no installer, FFmpeg bundled
