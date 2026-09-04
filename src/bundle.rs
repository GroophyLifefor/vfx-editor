use std::path::PathBuf;
use std::{env, fs};

static FFMPEG_ZST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ffmpeg.exe.zst"));

pub fn data_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("vfx-editor")
}

pub fn extract() -> Result<PathBuf, String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    let dest = dir.join("ffmpeg.exe");
    let expected: u64 = env!("FFMPEG_UNCOMPRESSED_SIZE")
        .parse()
        .expect("FFMPEG_UNCOMPRESSED_SIZE");
    if dest.metadata().map(|m| m.len() == expected).unwrap_or(false) {
        return Ok(dest);
    }

    let raw = zstd::decode_all(FFMPEG_ZST).map_err(|e| format!("decompress ffmpeg: {e}"))?;
    if raw.len() as u64 != expected {
        return Err(format!(
            "ffmpeg size mismatch: got {}, expected {expected}",
            raw.len()
        ));
    }
    let tmp = dir.join("ffmpeg.exe.tmp");
    fs::write(&tmp, &raw).map_err(|e| format!("write ffmpeg: {e}"))?;
    fs::rename(&tmp, &dest).map_err(|e| format!("rename ffmpeg: {e}"))?;
    Ok(dest)
}
