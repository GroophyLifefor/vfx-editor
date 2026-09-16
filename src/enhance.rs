use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One step of the Enhancements pipeline. Fixed order: cut dead frames, then interpolate,
/// then upscale — each stage's output feeds the next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Dead,
    Interp,
    Upscale,
}

impl Stage {
    /// Short tag for log lines, e.g. "enh 2/3 rife ok".
    pub fn tag(self) -> &'static str {
        match self {
            Self::Dead => "dead",
            Self::Interp => "rife",
            Self::Upscale => "esrgan",
        }
    }

    /// Suffix appended to the output filename.
    fn suffix(self) -> &'static str {
        match self {
            Self::Dead => "no dead frames",
            Self::Interp => "interpolated",
            Self::Upscale => "upscaled",
        }
    }
}

/// Build the stage list in the fixed pipeline order from checkbox state.
pub fn plan(dead: bool, interp: bool, upscale: bool) -> Vec<Stage> {
    let mut stages = Vec::new();
    if dead {
        stages.push(Stage::Dead);
    }
    if interp {
        stages.push(Stage::Interp);
    }
    if upscale {
        stages.push(Stage::Upscale);
    }
    stages
}

/// "clip.mp4" + [Dead, Upscale] -> "clip - no dead frames - upscaled.mp4"
pub fn out_name(stem: &str, ext: &str, stages: &[Stage]) -> String {
    let mut name = stem.to_string();
    for s in stages {
        name.push_str(" - ");
        name.push_str(s.suffix());
    }
    name.push('.');
    name.push_str(ext);
    name
}

/// Spawn the dead-frame ffmpeg pass. Drops frames byte-identical to the previous one
/// (mpdecimate hi=lo=1 = zero-change only) and retimes so playback speed is unaffected.
/// Re-encodes; drops audio since cut frames break A/V sync anyway. `-progress pipe:1` streams
/// live frame counts to stdout so the caller can show a progress bar (see `pump_dead_progress`).
pub fn spawn_dead_frames(ffmpeg: &Path, src: &Path, dest: &Path) -> Result<Child, String> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .arg("-i")
        .arg(src)
        .args([
            "-vf",
            "mpdecimate=hi=1:lo=1:frac=1,setpts=N/FRAME_RATE/TB",
            "-an",
            "-progress",
            "pipe:1",
            "-nostats",
        ])
        .arg(dest)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| format!("ffmpeg spawn: {e}"))
}

/// Reads `-progress pipe:1`'s `frame=N` lines and re-formats them as "frame=N/total)" — the
/// same shape `compare::parse_progress` already parses for Video2X — so the UI needs no
/// stage-specific progress handling. Returns the stderr tail for error reporting on failure.
pub fn pump_dead_progress(child: &mut Child, total: u64, tx: &Sender<String>) -> String {
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(n) = line
                .strip_prefix("frame=")
                .and_then(|s| s.trim().parse::<u64>().ok())
            {
                let _ = tx.send(format!("frame={n}/{total})"));
            }
        }
    }
    let mut tail = Vec::new();
    if let Some(err) = child.stderr.take() {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            tail.push(line);
            if tail.len() > 40 {
                tail.remove(0);
            }
        }
    }
    tail.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_keeps_fixed_order() {
        assert_eq!(
            plan(true, true, true),
            vec![Stage::Dead, Stage::Interp, Stage::Upscale]
        );
        assert_eq!(plan(false, true, false), vec![Stage::Interp]);
        assert_eq!(plan(false, false, false), vec![]);
    }

    #[test]
    fn out_name_chains_suffixes() {
        assert_eq!(
            out_name("clip", "mp4", &[Stage::Dead]),
            "clip - no dead frames.mp4"
        );
        assert_eq!(
            out_name("clip", "mp4", &plan(true, true, true)),
            "clip - no dead frames - interpolated - upscaled.mp4"
        );
        assert_eq!(out_name("clip", "mp4", &[]), "clip.mp4");
    }
}
