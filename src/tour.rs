use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One beat of `--intro-video`. Append a row when a feature ships, then
/// add a match arm in `PlayerApp::apply_tour_beat`.
pub struct Beat {
    pub id: &'static str,
    pub secs: f32,
    pub tr: &'static str,
    pub en: &'static str,
}

pub const BEATS: &[Beat] = &[
    Beat { id: "language", secs: 2.0, tr: "Dil seç", en: "Pick a language" },
    Beat { id: "empty", secs: 0.8, tr: "Araç çubuğu", en: "Toolbar" },
    Beat { id: "open", secs: 1.2, tr: "Video aç", en: "Open a video" },
    Beat { id: "play", secs: 3.5, tr: "Oynat", en: "Play" },
    Beat { id: "step", secs: 4.0, tr: "Kare kare ±1 / ±10", en: "Frame step ±1 / ±10" },
    Beat { id: "loop", secs: 3.5, tr: "Döngü I/O", en: "Loop I/O" },
    Beat { id: "wave", secs: 2.0, tr: "Dalga formu + ses boost", en: "Waveform + volume boost" },
    Beat { id: "zoom", secs: 3.0, tr: "Önizleme zoom", en: "Preview zoom" },
    Beat { id: "focus", secs: 2.0, tr: "Odak modu", en: "Focus mode" },
    Beat { id: "about", secs: 2.5, tr: "Hakkında", en: "About" },
    Beat { id: "log", secs: 2.0, tr: "Günlük", en: "Log" },
    Beat { id: "fit", secs: 1.3, tr: "Genişliğe sığdır", en: "Fit width" },
    Beat { id: "end", secs: 2.2, tr: "VFX Player", en: "VFX Player" },
];

pub const FPS: f32 = 30.0;

pub fn duration() -> f32 {
    BEATS.iter().map(|b| b.secs).sum()
}

pub fn start_pipe(ffmpeg: &Path, dest: &Path, w: u32, h: u32) -> Result<Child, String> {
    let w = w & !1;
    let h = h & !1;
    if w < 2 || h < 2 {
        return Err("shot size".into());
    }
    let mut child = Command::new(ffmpeg)
        .args([
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{w}x{h}"),
            "-r",
            "30",
            "-i",
            "-",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            dest.to_str().ok_or("video path")?,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("encode: {e}"))?;
    if child.stdin.is_none() {
        let _ = child.kill();
        return Err("encode stdin".into());
    }
    Ok(child)
}

pub fn write_frame(child: &mut Child, rgba: &[u8]) -> Result<(), String> {
    let stdin = child.stdin.as_mut().ok_or("encode stdin")?;
    stdin.write_all(rgba).map_err(|e| format!("frame: {e}"))
}

pub fn stop_pipe(mut child: Child) {
    drop(child.stdin.take());
    let _ = child.wait();
}
