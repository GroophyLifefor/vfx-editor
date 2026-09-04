use std::path::PathBuf;
use std::{env, fs};

static FFMPEG_ZST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ffmpeg.exe.zst"));
static YTDLP_ZST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/yt-dlp.exe.zst"));

pub fn data_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("vfx-editor")
}

pub fn extract() -> Result<PathBuf, String> {
    extract_exe(FFMPEG_ZST, "ffmpeg.exe", env!("FFMPEG_UNCOMPRESSED_SIZE"))
}

pub fn extract_ytdlp() -> Result<PathBuf, String> {
    extract_exe(YTDLP_ZST, "yt-dlp.exe", env!("YTDLP_UNCOMPRESSED_SIZE"))
}

fn extract_exe(zst: &[u8], name: &str, expected: &str) -> Result<PathBuf, String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    let dest = dir.join(name);
    let expected: u64 = expected.parse().expect("UNCOMPRESSED_SIZE");
    if dest.metadata().map(|m| m.len() == expected).unwrap_or(false) {
        return Ok(dest);
    }

    let raw = zstd::decode_all(zst).map_err(|e| format!("decompress {name}: {e}"))?;
    if raw.len() as u64 != expected {
        return Err(format!(
            "{name} size mismatch: got {}, expected {expected}",
            raw.len()
        ));
    }
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, &raw).map_err(|e| format!("write {name}: {e}"))?;
    fs::rename(&tmp, &dest).map_err(|e| format!("rename {name}: {e}"))?;
    Ok(dest)
}
