use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
const PEAK_BINS: usize = 2048;

pub struct AudioTrack {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
    pub peaks: Vec<(f32, f32)>,
}

impl AudioTrack {
    pub fn load(ffmpeg: &Path, path: &Path) -> Option<Self> {
        let mut child = Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
            ])
            .arg(path)
            .args([
                "-vn",
                "-map",
                "0:a:0",
                "-f",
                "s16le",
                "-ac",
                "2",
                "-ar",
                "44100",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .ok()?;
        let mut bytes = Vec::new();
        child.stdout.as_mut()?.read_to_end(&mut bytes).ok()?;
        let _ = child.wait();
        if bytes.len() < 4 {
            return None;
        }
        let samples: Vec<i16> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        if samples.len() < CHANNELS as usize {
            return None;
        }
        let peaks = peaks(&samples, CHANNELS as usize);
        Some(Self {
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS,
            samples,
            peaks,
        })
    }

    pub fn pcm_from(&self, time_sec: f64) -> Vec<f32> {
        let ch = self.channels as usize;
        let i = ((time_sec * self.sample_rate as f64).round() as usize).saturating_mul(ch);
        if i >= self.samples.len() {
            Vec::new()
        } else {
            self.samples[i..]
                .iter()
                .map(|s| *s as f32 / 32768.0)
                .collect()
        }
    }
}

fn peaks(samples: &[i16], channels: usize) -> Vec<(f32, f32)> {
    let frames = samples.len() / channels;
    if frames == 0 {
        return Vec::new();
    }
    let bins = PEAK_BINS.min(frames).max(1);
    (0..bins)
        .map(|bin| {
            let start = bin * frames / bins;
            let end = ((bin + 1) * frames / bins).max(start + 1);
            let mut lo = i16::MAX;
            let mut hi = i16::MIN;
            for frame in start..end {
                let s = samples[frame * channels];
                lo = lo.min(s);
                hi = hi.max(s);
            }
            (lo as f32 / 32768.0, hi as f32 / 32768.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_span() {
        let mut samples = vec![0i16; 200];
        samples[0] = 16384;
        samples[2] = -16384;
        let p = peaks(&samples, 2);
        assert!(!p.is_empty());
        assert!(p.iter().any(|(_, hi)| *hi > 0.4));
        assert!(p.iter().any(|(lo, _)| *lo < -0.4));
    }

    #[test]
    fn load_sine_mp4() {
        let ffmpeg = crate::bundle::extract().expect("extract ffmpeg");
        let mp4 = std::env::temp_dir().join("vfx_editor_sine.mp4");
        let status = Command::new(&ffmpeg)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=160x120:rate=24",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&mp4)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .expect("encode");
        assert!(status.success(), "encode failed: {status}");
        let track = AudioTrack::load(&ffmpeg, &mp4).expect("audio");
        assert!(track.samples.len() > 20_000, "got {}", track.samples.len());
        assert!(!track.peaks.is_empty());
        assert!(track.peaks.iter().any(|(_, hi)| *hi > 0.05));
    }
}
