# Changelog

CI takes the `##` section that matches `Cargo.toml`.

## 0.1.13

- Log lines carry a level (`INF` / `WRN` / `ERR`), shown in colour, with All / Warn / Error filters
- Header block now reports playing, audio, timeline, playhead, zoom and compare state
- Log mirrors to `session.log` next to `lang`, so it survives a close; crash log gains the version and exe
- More events logged: loop in/out/span/clear, copy and compare saves, URL extraction and download, start pid/exe/data dir
- Failures now log as `ERR` instead of blending into the rest

## 0.1.12

- Update check lists every release newer than this build, not just the latest
- Update dialog shows the release notes for each of those versions
- Badge marks the jump as Patch, New features, or Major release

## 0.1.11

- `Ctrl+S` saves the open video as-is (no trimming); with a loop active it trims and saves instead

## 0.1.10

- Start playback when launched by dropping a video on the exe

## 0.1.9

- Upscale replaced by Enhancements: cut dead frames, interpolate and upscale, chainable in one run
- Enhancements dialog shows a live pipeline strip and per-stage progress
- Output name previews the applied stages, e.g. `clip - no dead frames - upscaled.mp4`

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
