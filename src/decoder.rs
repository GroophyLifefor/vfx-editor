use std::io::{BufReader, ErrorKind, Read};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub frame_count: u64,
}

pub struct Decoder {
    ffmpeg: PathBuf,
    path: PathBuf,
    pub info: VideoInfo,
    pub current: u64,
    pub rgb: Vec<u8>,
    child: Option<Child>,
    stdout: Option<BufReader<ChildStdout>>,
}

impl Decoder {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn open(ffmpeg: PathBuf, path: PathBuf) -> Result<Self, String> {
        let info = probe(&ffmpeg, &path)?;
        let mut dec = Self {
            ffmpeg,
            path,
            info,
            current: 0,
            rgb: Vec::new(),
            child: None,
            stdout: None,
        };
        dec.seek(0)?;
        Ok(dec)
    }

    pub fn frame_bytes(&self) -> usize {
        self.info.width as usize * self.info.height as usize * 3
    }

    pub fn advance(&mut self) -> Result<bool, String> {
        if self.stdout.is_none() {
            self.spawn_from(self.current.saturating_add(1))?;
            return Ok(true);
        }
        match self.read_frame() {
            Ok(true) => {
                self.current += 1;
                Ok(true)
            }
            Ok(false) | Err(_) => {
                self.stop();
                Ok(false)
            }
        }
    }

    pub fn step(&mut self, delta: i64) -> Result<(), String> {
        if delta > 0 {
            for _ in 0..delta {
                if !self.advance()? {
                    break;
                }
            }
            Ok(())
        } else if delta < 0 {
            let target = self.current.saturating_sub(delta.unsigned_abs());
            self.seek(target)
        } else {
            Ok(())
        }
    }

    pub fn seek(&mut self, frame: u64) -> Result<(), String> {
        if self.stdout.is_some() && frame > self.current {
            let skip = frame - self.current;
            // ponytail: skip-in-pipe only for short jumps; long forward still restarts via -ss
            if skip <= 120 {
                for _ in 0..skip {
                    if !self.advance()? {
                        return Ok(());
                    }
                }
                return Ok(());
            }
        }
        self.spawn_from(frame)
    }

    fn spawn_from(&mut self, start_frame: u64) -> Result<(), String> {
        self.stop();
        let mut cmd = Command::new(&self.ffmpeg);
        cmd.arg("-hide_banner").arg("-loglevel").arg("error");
        if start_frame > 0 {
            let ts = start_frame as f64 / self.info.fps;
            cmd.arg("-ss").arg(format!("{ts:.6}"));
        }
        cmd.arg("-i")
            .arg(&self.path)
            .args([
                "-an",
                "-sn",
                "-map",
                "0:v:0",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-fps_mode",
                "passthrough",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("ffmpeg spawn: {e}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "ffmpeg stdout missing".to_string())?;
        self.child = Some(child);
        self.stdout = Some(BufReader::with_capacity(1 << 20, stdout));
        match self.read_frame()? {
            true => {
                self.current = start_frame;
                Ok(())
            }
            false => {
                self.stop();
                if start_frame == 0 && self.rgb.len() != self.frame_bytes() {
                    return Err("empty video".into());
                }
                self.current = start_frame;
                Ok(())
            }
        }
    }

    fn read_frame(&mut self) -> Result<bool, String> {
        let n = self.frame_bytes();
        let mut buf = vec![0u8; n];
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| "decoder not running".to_string())?;
        match stdout.read_exact(&mut buf) {
            Ok(()) => {
                self.rgb = buf;
                Ok(true)
            }
            Err(e) if matches!(e.kind(), ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe) => {
                Ok(false)
            }
            Err(e) => Err(format!("read frame: {e}")),
        }
    }

    fn stop(&mut self) {
        self.stdout.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn probe(ffmpeg: &Path, path: &Path) -> Result<VideoInfo, String> {
    let output = Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("ffmpeg probe: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_probe(&stderr)
}

pub fn parse_probe(stderr: &str) -> Result<VideoInfo, String> {
    let line = stderr
        .lines()
        .find(|l| l.contains("Video:"))
        .ok_or_else(|| "no video stream".to_string())?;
    let (width, height) = parse_wxh(line).ok_or_else(|| format!("no WxH in: {line}"))?;
    let fps = parse_fps(line).ok_or_else(|| format!("no fps in: {line}"))?;
    if !(fps.is_finite() && fps > 0.0) {
        return Err(format!("bad fps {fps}"));
    }
    let duration = parse_duration(stderr).unwrap_or(0.0);
    let mut frame_count = (duration * fps).round().max(1.0) as u64;
    if let Some(n) = parse_nb_frames(line) {
        frame_count = n;
    }
    Ok(VideoInfo {
        width,
        height,
        fps,
        frame_count,
    })
}

fn parse_wxh(line: &str) -> Option<(u32, u32)> {
    for token in line.split([',', ' ', '[']) {
        let Some((w, h)) = token.split_once('x') else {
            continue;
        };
        let Ok(w) = w.parse::<u32>() else {
            continue;
        };
        let Ok(h) = h.parse::<u32>() else {
            continue;
        };
        if w >= 16 && h >= 16 {
            return Some((w, h));
        }
    }
    None
}

fn parse_fps(line: &str) -> Option<f64> {
    for tag in [" fps", " tbr"] {
        if let Some(v) = number_before(line, tag) {
            return Some(v);
        }
    }
    None
}

fn number_before(line: &str, tag: &str) -> Option<f64> {
    let i = line.find(tag)?;
    let before = &line[..i];
    let num = before
        .rsplit(|c: char| !matches!(c, '0'..='9' | '.'))
        .next()?;
    let v: f64 = num.parse().ok()?;
    (v > 0.0).then_some(v)
}

fn parse_duration(s: &str) -> Option<f64> {
    let key = "Duration: ";
    let i = s.find(key)?;
    let t = s[i + key.len()..].split(',').next()?.trim();
    let mut parts = t.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let sec: f64 = parts.next()?.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + sec)
}

fn parse_nb_frames(line: &str) -> Option<u64> {
    number_before(line, " frames").map(|n| n as u64)
}

pub fn export_span(
    ffmpeg: &Path,
    src: &Path,
    dest: &Path,
    start: f64,
    end: f64,
) -> Result<(), String> {
    let start = start.max(0.0);
    let dur = (end - start).max(0.0);
    // ponytail: re-encode; stream-copy seeks to the previous keyframe and pads black
    let mut enc = Command::new(ffmpeg);
    enc.args(["-hide_banner", "-loglevel", "error", "-y"])
        .arg("-i")
        .arg(src)
        .arg("-ss")
        .arg(format!("{start:.6}"))
        .arg("-t")
        .arg(format!("{dur:.6}"))
        .args(["-map", "0", "-muxdelay", "0", "-muxpreload", "0"])
        .arg(dest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW);
    let out = enc.output().map_err(|e| format!("ffmpeg: {e}"))?;
    if out.status.success() && dest.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        Err(if err.is_empty() {
            "export failed".into()
        } else {
            err.into()
        })
    }
}

pub fn format_time(frame: u64, fps: f64) -> String {
    let ms_total = (frame as f64 * 1000.0 / fps).round().max(0.0) as u64;
    let sec = ms_total / 1000;
    let ms = ms_total % 1000;
    format!("{sec}:{ms:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'a.mp4':
  Duration: 00:00:02.00, start: 0.000000, bitrate: 100 kb/s
  Stream #0:0: Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 320x240 [SAR 1:1 DAR 4:3], 50 kb/s, 24 fps, 24 tbr, 12288 tbn
"#;

    #[test]
    fn probe_sample() {
        let info = parse_probe(SAMPLE).unwrap();
        assert_eq!(info.width, 320);
        assert_eq!(info.height, 240);
        assert!((info.fps - 24.0).abs() < 1e-6);
        assert_eq!(info.frame_count, 48);
    }

    #[test]
    fn time_at_fps() {
        assert_eq!(format_time(0, 24.0), "0:000");
        assert_eq!(format_time(24, 24.0), "1:000");
        assert_eq!(format_time(1, 30.0), "0:033");
    }

    #[test]
    fn decode_generated_mp4() {
        let ffmpeg = crate::bundle::extract().expect("extract ffmpeg");
        let mp4 = std::env::temp_dir().join("vfx_editor_testsrc.mp4");
        let status = Command::new(&ffmpeg)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=160x120:rate=24",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-an",
            ])
            .arg(&mp4)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .expect("spawn ffmpeg encode");
        assert!(status.success(), "encode failed: {status}");

        let mut dec = Decoder::open(ffmpeg, mp4).expect("open");
        assert_eq!(dec.info.width, 160);
        assert_eq!(dec.info.height, 120);
        assert!((dec.info.fps - 24.0).abs() < 0.01);
        assert_eq!(dec.current, 0);

        let mut count = 1u64;
        while dec.advance().expect("advance") {
            count += 1;
        }
        assert!(
            (24..=25).contains(&count),
            "expected ~24 frames, got {count}"
        );

        dec.step(-10).expect("back 10");
        assert_eq!(dec.current, count.saturating_sub(1 + 10));
        dec.step(1).expect("fwd 1");
        assert_eq!(dec.current, count.saturating_sub(1 + 9));
        dec.seek(10_000).expect("past end is eof not error");
    }
}
