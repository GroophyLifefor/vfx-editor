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

    let meta = fs::metadata(&ffmpeg).unwrap_or_else(|e| {
        panic!("cannot read {}: {e}", ffmpeg.display());
    });
    if meta.len() < 1_000_000 {
        panic!(
            "ffmpeg at {} is {} bytes (shim/link?). Set FFMPEG_EXE to the real ffmpeg.exe",
            ffmpeg.display(),
            meta.len()
        );
    }

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let zst = out.join("ffmpeg.exe.zst");
    let src = fs::read(&ffmpeg).unwrap_or_else(|e| panic!("read ffmpeg: {e}"));
    let compressed = zstd::encode_all(src.as_slice(), 8).unwrap_or_else(|e| panic!("zstd: {e}"));
    fs::write(&zst, compressed).unwrap_or_else(|e| panic!("write zst: {e}"));

    println!("cargo:rustc-env=FFMPEG_UNCOMPRESSED_SIZE={}", meta.len());
    println!("cargo:rustc-env=FFMPEG_SRC={}", ffmpeg.display());
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
    let line = stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| panic!("where ffmpeg returned nothing"));
    resolve(Path::new(line))
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
