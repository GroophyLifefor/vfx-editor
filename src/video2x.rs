use crate::compare::Recipe;
use crate::bundle;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const V2X_TAG: &str = "6.4.0";
const V2X_ZIP: &str = "https://github.com/k4yt3x/video2x/releases/download/6.4.0/video2x-windows-amd64.zip";

pub fn install_dir() -> PathBuf {
    bundle::data_dir().join("video2x")
}

pub fn find_exe() -> Option<PathBuf> {
    find_exe_under(&install_dir())
}

fn find_exe_under(root: &Path) -> Option<PathBuf> {
    let direct = root.join("video2x.exe");
    if direct.is_file() {
        return Some(direct);
    }
    let rd = std::fs::read_dir(root).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.file_name().and_then(|n| n.to_str()) == Some("video2x.exe") && p.is_file() {
            return Some(p);
        }
        if p.is_dir() {
            if let Some(hit) = find_exe_under(&p) {
                return Some(hit);
            }
        }
    }
    None
}

pub fn install(tx: &Sender<String>) -> Result<PathBuf, String> {
    if let Some(exe) = find_exe() {
        return Ok(exe);
    }
    let dir = install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("video2x dir: {e}"))?;
    let zip = dir.join("video2x-windows-amd64.zip");
    let _ = tx.send("Video2X downloading…".into());
    let st = Command::new("curl")
        .args(["-L", "--fail", "-o"])
        .arg(&zip)
        .arg(V2X_ZIP)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("curl: {e}"))?;
    if !st.success() {
        return Err("Video2X download failed".into());
    }
    let _ = tx.send("Video2X extracting…".into());
    let st = Command::new("tar")
        .args(["-xf"])
        .arg(&zip)
        .arg("-C")
        .arg(&dir)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !st.success() {
        return Err("Video2X extract failed".into());
    }
    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::write(dir.join("tag"), V2X_TAG);
    find_exe().ok_or_else(|| "video2x.exe missing after extract".into())
}

pub fn list_devices(exe: &Path) -> Vec<(u32, String)> {
    let out = Command::new(exe)
        .current_dir(exe.parent().unwrap_or(exe))
        .arg("--list-devices")
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let Ok(out) = out else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    let mut list = crate::compare::parse_devices(&text);
    if list.is_empty() {
        list = crate::compare::parse_devices(&err);
    }
    list
}

pub fn spawn(
    exe: &Path,
    input: &Path,
    output: &Path,
    recipe: Recipe,
    scale: u32,
    device: u32,
    best_encode: bool,
) -> Result<Child, String> {
    let mut cmd = Command::new(exe);
    cmd.current_dir(exe.parent().unwrap_or(exe));
    cmd.arg("-i").arg(input).arg("-o").arg(output);
    cmd.arg("-p").arg(recipe.processor());
    cmd.arg("-d").arg(device.to_string());
    if recipe.is_rife() {
        cmd.arg("-m").arg("2");
        cmd.arg("--rife-model").arg(recipe.model());
    } else {
        cmd.arg("-s").arg(scale.max(2).to_string());
        cmd.arg("--realesrgan-model").arg(recipe.model());
    }
    cmd.arg("-c").arg("libx264");
    cmd.arg("-e").arg("crf=23");
    cmd.arg("-e").arg(if best_encode {
        "preset=slow"
    } else {
        "preset=medium"
    });
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("video2x: {e}"))
}

pub fn pump_progress(child: &mut Child, tx: &Sender<String>) -> String {
    let mut tail = Vec::new();
    let mut eat = |line: String| {
        if crate::compare::parse_progress(&line).is_some() || line.contains("frame=") {
            let _ = tx.send(line);
            return;
        }
        let t = line.trim();
        if t.is_empty() {
            return;
        }
        let _ = tx.send(line.clone());
        tail.push(line);
        if tail.len() > 40 {
            tail.remove(0);
        }
    };
    if let Some(err) = child.stderr.take() {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            eat(line);
        }
    }
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            eat(line);
        }
    }
    tail.join("\n")
}

pub fn output_ready(dest: &Path) -> bool {
    dest.metadata().map(|m| m.is_file() && m.len() > 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_not_ready() {
        assert!(!output_ready(Path::new("C:\\no-such-v2x-out.mp4")));
    }
}
