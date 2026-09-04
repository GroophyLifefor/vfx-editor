use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon_png = manifest.join("icon.png");
    println!("cargo:rerun-if-changed={}", icon_png.display());
    embed_exe_icon(&icon_png);

    let ffmpeg = find_ffmpeg();
    println!("cargo:rerun-if-changed={}", ffmpeg.display());
    println!("cargo:rerun-if-env-changed=FFMPEG_EXE");
    pack_exe(&ffmpeg, "ffmpeg.exe.zst", "FFMPEG_UNCOMPRESSED_SIZE");

    let ytdlp = find_ytdlp();
    println!("cargo:rerun-if-changed={}", ytdlp.display());
    println!("cargo:rerun-if-env-changed=YTDLP_EXE");
    pack_exe(&ytdlp, "yt-dlp.exe.zst", "YTDLP_UNCOMPRESSED_SIZE");
}

fn pack_exe(src: &Path, zst_name: &str, size_env: &str) {
    let meta = fs::metadata(src).unwrap_or_else(|e| {
        panic!("cannot read {}: {e}", src.display());
    });
    if meta.len() < 1_000_000 {
        panic!(
            "{} is {} bytes (shim/link?). Set the real exe path.",
            src.display(),
            meta.len()
        );
    }
    let zst = PathBuf::from(env::var("OUT_DIR").unwrap()).join(zst_name);
    let bytes = fs::read(src).unwrap_or_else(|e| panic!("read {}: {e}", src.display()));
    let compressed = zstd::encode_all(bytes.as_slice(), 8).unwrap_or_else(|e| panic!("zstd: {e}"));
    fs::write(&zst, compressed).unwrap_or_else(|e| panic!("write zst: {e}"));
    println!("cargo:rustc-env={size_env}={}", meta.len());
}

fn find_ffmpeg() -> PathBuf {
    if let Ok(p) = env::var("FFMPEG_EXE") {
        return resolve(Path::new(&p));
    }
    let output = Command::new("where")
        .arg("ffmpeg")
        .output()
        .unwrap_or_else(|e| panic!("where ffmpeg failed: {e}. Install ffmpeg on the build machine."));
    if !output.status.success() {
        panic!("ffmpeg not on PATH. Install it or set FFMPEG_EXE.");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let p = resolve(Path::new(line));
        if fs::metadata(&p).map(|m| m.len() >= 1_000_000).unwrap_or(false) {
            return p;
        }
    }
    panic!(
        "ffmpeg on PATH is a shim. Set FFMPEG_EXE to the real ffmpeg.exe"
    );
}

fn find_ytdlp() -> PathBuf {
    if let Ok(p) = env::var("YTDLP_EXE") {
        return resolve(Path::new(&p));
    }
    if let Ok(output) = Command::new("where").arg("yt-dlp").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().map(str::trim).filter(|s| !s.is_empty()) {
                let p = resolve(Path::new(line));
                if fs::metadata(&p).map(|m| m.len() >= 1_000_000).unwrap_or(false) {
                    return p;
                }
            }
        }
    }
    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("yt-dlp.exe");
    if dest.metadata().map(|m| m.len() >= 1_000_000).unwrap_or(false) {
        return dest;
    }
    let ok = Command::new("curl")
        .args([
            "-fsSL",
            "-o",
        ])
        .arg(&dest)
        .arg("https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok || dest.metadata().map(|m| m.len() < 1_000_000).unwrap_or(true) {
        panic!("yt-dlp missing. Install it or set YTDLP_EXE, or allow curl to GitHub.");
    }
    dest
}

fn resolve(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn embed_exe_icon(png_path: &Path) {
    let png = fs::read(png_path).unwrap_or_else(|e| panic!("read icon.png: {e}"));
    let ico_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("icon.ico");
    write_png_ico(&png, &ico_path);
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().expect("ico path"));
    res.compile().unwrap_or_else(|e| panic!("embed icon: {e}"));
}

fn write_png_ico(png: &[u8], dest: &Path) {
    assert!(png.len() >= 24, "icon.png too small");
    let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
    let mut ico = Vec::with_capacity(22 + png.len());
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.push(if w >= 256 { 0 } else { w as u8 });
    ico.push(if h >= 256 { 0 } else { h as u8 });
    ico.push(0);
    ico.push(0);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(png);
    fs::write(dest, ico).unwrap_or_else(|e| panic!("write ico: {e}"));
}
