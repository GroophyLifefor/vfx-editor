pub fn fps_differs(a: f64, b: f64) -> bool {
    (a - b).abs() > 0.05
}

pub fn timeline_len(p_n: u64, p_fps: f64, c_n: u64, c_fps: f64) -> u64 {
    let p_n = p_n.max(1);
    let p_fps = p_fps.max(0.001);
    let c_n = c_n.max(1);
    let c_fps = c_fps.max(0.001);
    let c_as_p = ((c_n as f64 / c_fps) * p_fps).round() as u64;
    p_n.max(c_as_p.max(1))
}

pub fn map_compare_frame(p_frame: u64, p_fps: f64, c_n: u64, c_fps: f64) -> u64 {
    let last = c_n.saturating_sub(1);
    let t = p_frame as f64 / p_fps.max(0.001);
    let f = (t * c_fps.max(0.001)).floor() as u64;
    f.min(last)
}

pub fn parse_devices(text: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('[') {
            let Some((id_name, _)) = rest.split_once(']') else {
                continue;
            };
            let mut parts = id_name.splitn(2, char::is_whitespace);
            let Some(id) = parts.next().and_then(|s| s.parse().ok()) else {
                continue;
            };
            let name = parts.next().unwrap_or("GPU").trim();
            if !name.is_empty() {
                out.push((id, name.to_string()));
            }
            continue;
        }
        // Video2X 6.4: "0. Intel(R) UHD Graphics"
        let Some((id, name)) = line.split_once('.') else {
            continue;
        };
        let Some(id) = id.trim().parse().ok() else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() {
            out.push((id, name.to_string()));
        }
    }
    out
}

pub fn parse_progress(line: &str) -> Option<(u64, u64)> {
    let line = line.trim();
    let rest = line.split("frame=").nth(1)?;
    let pair = rest.split(')').next()?;
    let nums = pair.split('(').next()?.trim();
    let (a, b) = nums.split_once('/')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

pub fn estimate_bytes(src_len: u64, scale: u32, frame_mult: f64) -> u64 {
    let area = (scale as u64).max(1).pow(2);
    let m = frame_mult.clamp(0.25, 8.0);
    ((src_len as f64 * area as f64 * m) * 1.2) as u64
}

#[derive(Clone, Copy, PartialEq)]
pub enum Recipe {
    AnimeFast,
    AnimeSlow,
    General,
    RifeLite,
    RifeMid,
    RifeBest,
}

impl Recipe {
    pub fn processor(self) -> &'static str {
        match self {
            Self::RifeLite | Self::RifeMid | Self::RifeBest => "rife",
            _ => "realesrgan",
        }
    }

    pub fn model(self) -> &'static str {
        match self {
            Self::AnimeFast => "realesr-animevideov3",
            Self::AnimeSlow => "realesrgan-plus-anime",
            Self::General => "realesrgan-plus",
            Self::RifeLite => "rife-v4.25-lite",
            Self::RifeMid => "rife-v4.25",
            Self::RifeBest => "rife-v4.26",
        }
    }

    pub fn is_rife(self) -> bool {
        matches!(self, Self::RifeLite | Self::RifeMid | Self::RifeBest)
    }
}
