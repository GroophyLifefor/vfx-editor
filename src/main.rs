#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod bundle;
mod decoder;

use audio::AudioTrack;
use decoder::{export_span, Decoder};
use eframe::egui::{
    self, Align2, Color32, ColorImage, CursorIcon, FontId, IconData, Key, Modifiers, Pos2, Rect,
    Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const VIDEO_EXTS: &[&str] = &[
    "mp4", "mov", "m4v", "mkv", "webm", "avi", "wmv", "mpg", "mpeg", "m2ts", "ts", "vob", "flv",
    "3gp", "ogv", "mxf",
];
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const ABOUT_URL: &str = "https://github.com/GroophyLifefor";
const REPO_URL: &str = "https://github.com/GroophyLifefor/vfx-editor";
const REPO_RELEASES: &str = "https://github.com/GroophyLifefor/vfx-editor/releases/latest";
const REPO_API: &str =
    "https://api.github.com/repos/GroophyLifefor/vfx-editor/releases/latest";
const EXE_DOWNLOAD: &str =
    "https://github.com/GroophyLifefor/vfx-editor/releases/latest/download/vfx_editor.exe";
const ABOUT_FEATURES: &[(&str, &str)] = &[
    ("Yakınlaştır", "Zoom in"),
    ("Uzaklaştır", "Zoom out"),
    ("Genişliğe sığdır", "Fit width"),
    ("Yüksekliğe sığdır", "Fit height"),
    ("Yakınken sürükleyerek kaydır", "Pan while zoomed"),
    ("Oynat / duraklat", "Play / pause"),
    ("±1 / ±10 kare", "±1 / ±10 frames"),
    ("Oynatma FPS (2 ondalık)", "Playback FPS (2 decimals)"),
    ("Kare ve saniye zaman çizelgesi", "Frame + time timeline"),
    ("Ses dalga formu", "Audio waveform"),
    ("Ses %0–125, %100 üstü boost", "Volume 0–125%, boost above 100%"),
    ("Döngü (I/O, Shift+sürükle)", "Loop (I/O, Shift+drag)"),
    ("Döngüyü aynı formatta kırp ve kaydet", "Trim loop and save (same format)"),
    (
        "YouTube / Instagram / TikTok / Facebook / X / Reddit (kalite seç)",
        "YouTube / Instagram / TikTok / Facebook / X / Reddit (pick quality)",
    ),
    ("Exe üzerine bırakarak aç", "Open by dropping a file on the exe"),
    ("TR / EN", "TR / EN"),
    ("Koyu / açık tema", "Dark / light theme"),
    ("Zaman çizelgesi zoom (saniye → kare)", "Timeline zoom (seconds → frames)"),
    ("Odak modu (F)", "Focus mode (F)"),
    ("Dalga formunu gizle / göster", "Show / hide waveform"),
];

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Tr,
    En,
}

impl Lang {
    fn tr(self, tr: &'static str, en: &'static str) -> &'static str {
        match self {
            Self::Tr => tr,
            Self::En => en,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Tr => "tr",
            Self::En => "en",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "tr" => Some(Self::Tr),
            "en" => Some(Self::En),
            _ => None,
        }
    }
}

fn lang_path() -> PathBuf {
    bundle::data_dir().join("lang")
}

fn load_lang() -> Option<Lang> {
    std::fs::read_to_string(lang_path())
        .ok()
        .and_then(|s| Lang::parse(&s))
}

fn save_lang(lang: Lang) {
    let _ = std::fs::create_dir_all(bundle::data_dir());
    let _ = std::fs::write(lang_path(), lang.code());
}

fn theme_path() -> PathBuf {
    bundle::data_dir().join("theme")
}

fn load_dark() -> bool {
    match std::fs::read_to_string(theme_path()).as_deref() {
        Ok("light") => false,
        _ => true,
    }
}

fn save_dark(dark: bool) {
    let _ = std::fs::create_dir_all(bundle::data_dir());
    let _ = std::fs::write(theme_path(), if dark { "dark" } else { "light" });
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals(if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
}

fn load_icon() -> IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../icon.png")).expect("icon.png")
}

fn parse_cli() -> (Option<PathBuf>, Option<PathBuf>, bool) {
    let mut open = None;
    let mut shots = None;
    let mut updated = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        if a == "--intro-shots" {
            shots = args.next().map(PathBuf::from);
        } else if a == "--updated" {
            updated = true;
        } else {
            let p = PathBuf::from(a);
            if !p.to_string_lossy().starts_with('-') {
                open = Some(p);
            }
        }
    }
    (open, shots, updated)
}

fn main() -> eframe::Result {
    let (open, shots, updated) = parse_cli();
    let icon = load_icon();
    let size = if shots.is_some() {
        [1280.0, 800.0]
    } else {
        [1100.0, 780.0]
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_title("VFX Player")
            .with_icon(icon.clone()),
        ..Default::default()
    };
    eframe::run_native(
        "VFX Player",
        options,
        Box::new(move |cc| Ok(Box::new(PlayerApp::new(cc, &icon, open, shots, updated)))),
    )
}

#[derive(Clone, Copy, PartialEq)]
enum ZoomMode {
    Contain,
    FitWidth,
    FitHeight,
    Manual,
}

const ZOOM_MIN: f32 = 0.05;
const ZOOM_MAX: f32 = 16.0;

fn view_scale(
    avail_w: f32,
    avail_h: f32,
    vid_w: f32,
    vid_h: f32,
    mode: ZoomMode,
    zoom: f32,
) -> f32 {
    let vw = vid_w.max(1.0);
    let vh = vid_h.max(1.0);
    let s = match mode {
        ZoomMode::Contain => (avail_w / vw).min(avail_h / vh),
        ZoomMode::FitWidth => avail_w / vw,
        ZoomMode::FitHeight => avail_h / vh,
        ZoomMode::Manual => zoom,
    };
    s.clamp(ZOOM_MIN, ZOOM_MAX)
}

fn image_origin(size: Vec2, avail: Vec2, pan: Vec2) -> Vec2 {
    Vec2::new(
        if size.x <= avail.x {
            (avail.x - size.x) * 0.5
        } else {
            -pan.x
        },
        if size.y <= avail.y {
            (avail.y - size.y) * 0.5
        } else {
            -pan.y
        },
    )
}

fn pan_from_origin(origin: Vec2, size: Vec2, avail: Vec2) -> Vec2 {
    Vec2::new(
        if size.x <= avail.x {
            0.0
        } else {
            (-origin.x).clamp(0.0, size.x - avail.x)
        },
        if size.y <= avail.y {
            0.0
        } else {
            (-origin.y).clamp(0.0, size.y - avail.y)
        },
    )
}

fn remap_pan(pan: Vec2, old_size: Vec2, new_size: Vec2, avail: Vec2, anchor: Vec2) -> Vec2 {
    let origin = image_origin(old_size, avail, pan);
    let img = anchor - origin;
    let factor = Vec2::new(
        if old_size.x > 1.0 {
            new_size.x / old_size.x
        } else {
            1.0
        },
        if old_size.y > 1.0 {
            new_size.y / old_size.y
        } else {
            1.0
        },
    );
    pan_from_origin(anchor - img * factor, new_size, avail)
}

struct PlayerApp {
    ffmpeg: Result<PathBuf, String>,
    decoder: Option<Decoder>,
    audio: Option<AudioTrack>,
    output: Option<rodio::OutputStream>,
    sink: Option<rodio::Sink>,
    texture: Option<TextureHandle>,
    logo: TextureHandle,
    playing: bool,
    playback_fps: f64,
    last_tick: Option<Instant>,
    accum: f64,
    status: String,
    playhead: Option<u64>,
    scrubbing: bool,
    last_scrub_at: Option<Instant>,
    zoom_mode: ZoomMode,
    zoom: f32,
    pan: Vec2,
    last_scale: f32,
    loop_in: Option<u64>,
    loop_out: Option<u64>,
    loop_on: bool,
    range_drag: Option<RangeDrag>,
    lang: Option<Lang>,
    url: String,
    fetch: Option<mpsc::Receiver<FetchEvent>>,
    format_pick: Option<(String, Vec<FormatOpt>)>,
    export: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    update_rx: Option<mpsc::Receiver<Option<String>>>,
    update_url: Option<String>,
    update_modal: Option<UpdateModal>,
    about_open: bool,
    volume: f32,
    dark: bool,
    pending_open: Option<PathBuf>,
    intro: Option<IntroShots>,
    tl_zoom: f32,
    tl_scroll: f32,
    wave_h: f32,
    bar_h: f32,
    wave_on: bool,
    focus: bool,
}

struct IntroShots {
    dir: PathBuf,
    phase: u8,
    wait: u8,
    capture: Option<&'static str>,
    requested: bool,
}

enum FetchEvent {
    Status(String),
    Formats(String, Vec<FormatOpt>),
    Done(Result<PathBuf, String>),
}

#[derive(Clone)]
struct FormatOpt {
    label: String,
    spec: String,
    merge: &'static str,
}

enum UpdateModal {
    Busy(String),
    Fail(String),
    Done,
}

#[derive(Clone, Copy, PartialEq)]
enum RangeDrag {
    Create { origin: u64 },
    In,
    Out,
}

impl PlayerApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        icon: &IconData,
        pending_open: Option<PathBuf>,
        intro_dir: Option<PathBuf>,
        from_update: bool,
    ) -> Self {
        let logo = cc.egui_ctx.load_texture(
            "logo",
            ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            TextureOptions::LINEAR,
        );
        let ffmpeg = bundle::extract();
        let intro = intro_dir.map(|dir| IntroShots {
            dir,
            phase: 0,
            wait: 12,
            capture: None,
            requested: false,
        });
        let lang = if intro.is_some() {
            None
        } else {
            load_lang().or(pending_open.as_ref().map(|_| Lang::En))
        };
        let dark = intro.is_some() || load_dark();
        apply_theme(&cc.egui_ctx, dark);
        let status = match &ffmpeg {
            Ok(_) => lang
                .map(|l| l.tr("Video aç", "Open a video").to_string())
                .unwrap_or_default(),
            Err(e) => e.clone(),
        };
        let mut app = Self {
            ffmpeg,
            decoder: None,
            audio: None,
            output: None,
            sink: None,
            texture: None,
            logo,
            playing: false,
            playback_fps: 24.0,
            last_tick: None,
            accum: 0.0,
            status,
            playhead: None,
            scrubbing: false,
            last_scrub_at: None,
            zoom_mode: ZoomMode::Contain,
            zoom: 1.0,
            pan: Vec2::ZERO,
            last_scale: 0.0,
            loop_in: None,
            loop_out: None,
            loop_on: false,
            range_drag: None,
            lang,
            url: String::new(),
            fetch: None,
            format_pick: None,
            export: None,
            update_rx: None,
            update_url: None,
            update_modal: None,
            about_open: false,
            volume: 1.0,
            dark,
            pending_open,
            intro,
            tl_zoom: 1.0,
            tl_scroll: 0.0,
            wave_h: 56.0,
            bar_h: 28.0,
            wave_on: true,
            focus: false,
        };
        if app.intro.is_none() {
            let marked = take_just_updated();
            if from_update || marked {
                app.update_modal = Some(UpdateModal::Done);
            }
            app.spawn_update_check();
            if let Some(p) = app.pending_open.take() {
                app.open_path(p, &cc.egui_ctx);
            }
        }
        app
    }

    fn lang(&self) -> Lang {
        self.lang.unwrap_or(Lang::En)
    }

    fn set_lang(&mut self, lang: Lang) {
        self.lang = Some(lang);
        save_lang(lang);
        if self.decoder.is_none() && self.fetch.is_none() && self.ffmpeg.is_ok() {
            self.status = lang.tr("Video aç", "Open a video").into();
        }
    }

    fn set_dark(&mut self, ctx: &egui::Context, dark: bool) {
        self.dark = dark;
        save_dark(dark);
        apply_theme(ctx, dark);
    }

    fn tick_intro(&mut self, ctx: &egui::Context) {
        if self.intro.is_none() {
            return;
        }
        ctx.request_repaint();
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, user_data, .. } => {
                    let name = user_data
                        .data
                        .as_ref()?
                        .downcast_ref::<String>()
                        .cloned()?;
                    Some((name, image.clone()))
                }
                _ => None,
            })
        });
        if let Some((name, image)) = shot {
            if let (Ok(ff), Some(intro)) = (&self.ffmpeg, &self.intro) {
                let dest = intro.dir.join(format!("{name}.png"));
                let _ = std::fs::create_dir_all(&intro.dir);
                let _ = save_color_image(ff, &image, &dest);
            }
            if let Some(intro) = &mut self.intro {
                intro.capture = None;
                intro.requested = false;
                intro.phase = intro.phase.saturating_add(1);
                intro.wait = 8;
                if intro.phase >= 6 {
                    std::process::exit(0);
                }
            }
            return;
        }
        let capture = self.intro.as_ref().and_then(|i| i.capture);
        let requested = self.intro.as_ref().is_some_and(|i| i.requested);
        if let Some(name) = capture {
            if name == "06-about" {
                if let Some(intro) = &mut self.intro {
                    intro.wait = intro.wait.saturating_add(1);
                    if intro.wait >= 18 {
                        let dest = intro.dir.join("06-about.png");
                        let _ = std::fs::create_dir_all(&intro.dir);
                        capture_named_window("About", &dest);
                        std::process::exit(0);
                    }
                }
                return;
            }
            if let Some(intro) = &mut self.intro {
                intro.wait = intro.wait.saturating_add(1);
                if intro.wait > 90 {
                    intro.capture = None;
                    intro.requested = false;
                    intro.phase = intro.phase.saturating_add(1);
                    intro.wait = 4;
                    return;
                }
            }
            if !requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                    name.to_string(),
                )));
                if let Some(intro) = &mut self.intro {
                    intro.requested = true;
                }
            }
            return;
        }
        if let Some(intro) = &mut self.intro {
            if intro.wait > 0 {
                intro.wait -= 1;
                return;
            }
        }
        let phase = self.intro.as_ref().map(|i| i.phase).unwrap_or(99);
        match phase {
            0 => {
                if let Some(intro) = &mut self.intro {
                    intro.capture = Some("01-language");
                }
            }
            1 => {
                self.lang = Some(Lang::En);
                self.status = "Open a video".into();
                if let Some(intro) = &mut self.intro {
                    intro.capture = Some("02-empty");
                }
            }
            2 => {
                if let Some(p) = self.pending_open.take() {
                    self.open_path(p, ctx);
                    self.status = "example.mp4".into();
                }
                if let Some(n) = self.decoder.as_ref().map(|d| d.info.frame_count.max(1)) {
                    self.seek_to(ctx, n / 5);
                    self.status = "example.mp4".into();
                    if let Some(intro) = &mut self.intro {
                        intro.capture = Some("03-overview");
                    }
                } else if let Some(intro) = &mut self.intro {
                    intro.wait = 10;
                }
            }
            3 => {
                if let Some(n) = self.decoder.as_ref().map(|d| d.info.frame_count.max(2)) {
                    let a = n / 6;
                    let b = n * 2 / 3;
                    self.set_loop_span(a, b);
                    self.seek_to(ctx, (a + b) / 2);
                    self.tl_zoom = 3.5;
                    center_tl(&mut self.tl_scroll, self.tl_zoom, (a + b) / 2, n);
                    self.status = "example.mp4".into();
                }
                if let Some(intro) = &mut self.intro {
                    intro.capture = Some("04-loop");
                }
            }
            4 => {
                self.zoom_mode = ZoomMode::Manual;
                self.zoom = 2.4;
                if let Some(d) = &self.decoder {
                    let n = d.info.frame_count.max(1);
                    let f = self.playhead.or(Some(d.current)).unwrap_or(n / 5);
                    self.tl_zoom = 6.5;
                    center_tl(&mut self.tl_scroll, self.tl_zoom, f, n);
                }
                if let Some(intro) = &mut self.intro {
                    intro.capture = Some("05-zoom");
                }
            }
            5 => {
                self.about_open = true;
                if let Some(intro) = &mut self.intro {
                    intro.capture = Some("06-about");
                    intro.wait = 6;
                }
            }
            _ => std::process::exit(0),
        }
    }

    fn open_path(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.stop_audio();
        self.playing = false;
        self.last_tick = None;
        self.accum = 0.0;
        self.playhead = None;
        self.scrubbing = false;
        self.last_scrub_at = None;
        self.zoom_mode = ZoomMode::Contain;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.last_scale = 0.0;
        self.loop_in = None;
        self.loop_out = None;
        self.loop_on = false;
        self.range_drag = None;
        self.tl_zoom = 1.0;
        self.tl_scroll = 0.0;
        let ffmpeg = match &self.ffmpeg {
            Ok(p) => p.clone(),
            Err(e) => {
                self.status = e.clone();
                return;
            }
        };
        match Decoder::open(ffmpeg.clone(), path.clone()) {
            Ok(dec) => {
                self.playback_fps = dec.info.fps;
                self.status = path.display().to_string();
                self.audio = AudioTrack::load(&ffmpeg, &path);
                self.decoder = Some(dec);
                self.sync_texture(ctx);
            }
            Err(e) => {
                self.decoder = None;
                self.audio = None;
                self.texture = None;
                self.status = e;
            }
        }
    }

    fn sync_texture(&mut self, ctx: &egui::Context) {
        let Some(dec) = &self.decoder else {
            return;
        };
        let size = [dec.info.width as usize, dec.info.height as usize];
        let image = ColorImage::from_rgb(size, &dec.rgb);
        match &mut self.texture {
            Some(tex) => tex.set(image, TextureOptions::NEAREST),
            None => {
                self.texture = Some(ctx.load_texture("frame", image, TextureOptions::NEAREST));
            }
        }
    }

    fn pick_file(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Video", VIDEO_EXTS)
            .pick_file()
        {
            self.open_path(path, ctx);
        }
    }

    fn start_url(&mut self) {
        if self.fetch.is_some() {
            return;
        }
        let url = self.url.trim().to_string();
        if url.is_empty() {
            return;
        }
        let lang = self.lang();
        if !url_allowed(&url) {
            self.status = lang
                .tr(
                    "Desteklenen: YouTube, Shorts, Instagram, TikTok, Facebook, X, Reddit",
                    "Supported: YouTube, Shorts, Instagram, TikTok, Facebook, X, Reddit",
                )
                .into();
            return;
        }
        self.format_pick = None;
        self.status = lang.tr("Çıkarılıyor…", "Extracting…").into();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            match list_formats(&url, lang, &tx) {
                Ok(opts) => {
                    let _ = tx.send(FetchEvent::Formats(url, opts));
                }
                Err(e) => {
                    let _ = tx.send(FetchEvent::Done(Err(e)));
                }
            }
        });
        self.fetch = Some(rx);
    }

    fn start_format_download(&mut self, url: String, spec: String, merge: &'static str) {
        if self.fetch.is_some() {
            return;
        }
        let lang = self.lang();
        let ffmpeg = self.ffmpeg.clone().ok();
        self.format_pick = None;
        self.status = lang.tr("İndiriliyor…", "Downloading…").into();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let r = download_ytdlp(&url, lang, ffmpeg.as_deref(), &spec, merge, &tx);
            let _ = tx.send(FetchEvent::Done(r));
        });
        self.fetch = Some(rx);
    }

    fn poll_fetch(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.fetch else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(FetchEvent::Status(s)) => {
                    self.status = s;
                    ctx.request_repaint();
                }
                Ok(FetchEvent::Formats(url, opts)) => {
                    self.fetch = None;
                    self.status = self.lang().tr("Kalite seç", "Pick a quality").into();
                    self.format_pick = Some((url, opts));
                    return;
                }
                Ok(FetchEvent::Done(v)) => {
                    self.fetch = None;
                    match v {
                        Ok(path) => self.open_path(path, ctx),
                        Err(e) => self.status = e,
                    }
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint();
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.fetch = None;
                    self.status = self.lang().tr("İndirme kesildi", "Download failed").into();
                    return;
                }
            }
        }
    }

    fn export_loop(&mut self) {
        if self.export.is_some() {
            return;
        }
        let Some(dec) = &self.decoder else {
            return;
        };
        let Some((a, b)) = self.loop_range() else {
            return;
        };
        let fps = dec.info.fps.max(0.001);
        let start = (a as f64 + 0.5) / fps;
        let end = start + (b + 1 - a) as f64 / fps;
        let src = dec.path().to_path_buf();
        let ffmpeg = match &self.ffmpeg {
            Ok(p) => p.clone(),
            Err(e) => {
                self.status = e.clone();
                return;
            }
        };
        let ext = src
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("mp4");
        let stem = src
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("clip");
        let suggested = format!("{stem}_loop.{ext}");
        let Some(dest) = rfd::FileDialog::new()
            .add_filter(ext, &[ext])
            .set_file_name(&suggested)
            .save_file()
        else {
            return;
        };
        if src.canonicalize().ok() == dest.canonicalize().ok() {
            self.status = self
                .lang()
                .tr("Kaynak dosyanın üzerine yazma", "Don't overwrite the source")
                .into();
            return;
        }
        let lang = self.lang();
        self.status = lang.tr("Kırpılıyor…", "Trimming…").into();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let r = export_span(&ffmpeg, &src, &dest, start, end).map(|()| dest);
            let _ = tx.send(r);
        });
        self.export = Some(rx);
    }

    fn spawn_update_check(&mut self) {
        if self.update_rx.is_some() || self.update_url.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(latest_update_url());
        });
        self.update_rx = Some(rx);
    }

    fn begin_update(&mut self) {
        let Some(tag) = self.update_url.take() else {
            return;
        };
        let lang = self.lang();
        let _ = std::fs::remove_file(update_state_path());
        match spawn_self_update() {
            Ok(()) => {
                let _ = std::fs::create_dir_all(bundle::data_dir());
                let _ = std::fs::write(just_updated_path(), "1");
                self.status = lang
                    .tr("Güncelleme indiriliyor…", "Downloading update…")
                    .into();
                self.update_modal = Some(UpdateModal::Busy(tag));
            }
            Err(e) => {
                self.update_url = Some(tag);
                self.status = e.clone();
                self.update_modal = Some(UpdateModal::Fail(e));
            }
        }
    }

    fn poll_update_state(&mut self, ctx: &egui::Context) {
        let Some(UpdateModal::Busy(tag)) = &self.update_modal else {
            return;
        };
        ctx.request_repaint();
        let Ok(s) = std::fs::read_to_string(update_state_path()) else {
            return;
        };
        if s.trim() != "fail" {
            return;
        }
        let _ = std::fs::remove_file(update_state_path());
        let _ = std::fs::remove_file(just_updated_path());
        let tag = tag.clone();
        self.update_url = Some(tag);
        let msg = self
            .lang()
            .tr("İndirme başarısız", "Download failed")
            .to_string();
        self.status = msg.clone();
        self.update_modal = Some(UpdateModal::Fail(msg));
    }

    fn show_update_modal(&mut self, ctx: &egui::Context) {
        if self.lang.is_none() {
            return;
        }
        let lang = self.lang();
        let Some(kind) = &self.update_modal else {
            return;
        };
        let mut close = false;
        egui::Window::new(lang.tr("Güncelleme", "Update"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(280.0);
                match kind {
                    UpdateModal::Busy(tag) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(lang.tr(
                                "Yeni sürüm indiriliyor…",
                                "Downloading the new version…",
                            ));
                        });
                        ui.add_space(6.0);
                        ui.strong(tag);
                        ui.weak(lang.tr(
                            "Uygulama kapanıp yeniden açılacak.",
                            "The app will close and reopen.",
                        ));
                    }
                    UpdateModal::Fail(e) => {
                        ui.label(lang.tr("Güncelleme başarısız", "Update failed"));
                        ui.weak(e);
                        ui.add_space(8.0);
                        if ui.button(lang.tr("Tamam", "OK")).clicked() {
                            close = true;
                        }
                    }
                    UpdateModal::Done => {
                        ui.heading(lang.tr("Güncellendi", "Updated"));
                        ui.add_space(4.0);
                        ui.label(lang.tr("Başarılı.", "Success."));
                        ui.strong(format!("VFX Player v{APP_VERSION}"));
                        ui.add_space(8.0);
                        if ui.button(lang.tr("Tamam", "OK")).clicked() {
                            close = true;
                        }
                    }
                }
            });
        if close {
            self.update_modal = None;
        }
    }

    fn show_format_modal(&mut self, ctx: &egui::Context) {
        if self.lang.is_none() {
            return;
        }
        let lang = self.lang();
        let Some((_, formats)) = &self.format_pick else {
            return;
        };
        let mut pick: Option<FormatOpt> = None;
        let mut cancel = false;
        egui::Window::new(lang.tr("Kalite", "Quality"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(280.0);
                ui.strong(lang.tr("Mevcut formatlar", "Available formats"));
                ui.weak("↓");
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        for f in formats {
                            if ui
                                .add_sized([260.0, 28.0], egui::Button::new(&f.label))
                                .clicked()
                            {
                                pick = Some(f.clone());
                            }
                        }
                    });
                ui.add_space(8.0);
                if ui.button(lang.tr("Vazgeç", "Cancel")).clicked() {
                    cancel = true;
                }
            });
        if cancel {
            self.format_pick = None;
            self.status = lang.tr("Video aç", "Open a video").into();
        } else if let Some(f) = pick {
            let url = self.format_pick.as_ref().map(|(u, _)| u.clone()).unwrap();
            self.start_format_download(url, f.spec, f.merge);
        }
    }

    fn poll_update(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.update_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(url) => {
                self.update_url = url;
                self.update_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => ctx.request_repaint(),
            Err(mpsc::TryRecvError::Disconnected) => self.update_rx = None,
        }
    }

    fn poll_export(&mut self, ctx: &egui::Context) {
        let msg = {
            let Some(rx) = &self.export else {
                return;
            };
            match rx.try_recv() {
                Ok(v) => v,
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint();
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err(self.lang().tr("Kırpma kesildi", "Trim failed").into())
                }
            }
        };
        self.export = None;
        match msg {
            Ok(path) => self.status = path.display().to_string(),
            Err(e) => self.status = e,
        }
    }

    fn show_about(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let lang = self.lang();
        let title = lang.tr("Hakkında", "About");
        let logo = self.logo.id();
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("about"),
            egui::ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([480.0, 460.0])
                .with_min_inner_size([400.0, 360.0])
                .with_always_on_top()
                .with_minimize_button(false)
                .with_maximize_button(false),
            |ui, _class| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.add(egui::Image::new((logo, egui::vec2(52.0, 52.0))));
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.heading("VFX Player");
                        ui.weak(lang.tr(
                            "VFX uzmanları için ileri video oynatıcı",
                            "Advanced Video Player for VFX Experts",
                        ));
                        ui.horizontal(|ui| {
                            ui.weak(format!("v{APP_VERSION}"))
                                .on_hover_text(lang.tr("Sürüm", "Version"));
                            ui.weak("·");
                            ui.weak("Windows");
                        });
                    });
                });
                ui.add_space(8.0);
                ui.label(lang.tr(
                    "Murat Kirazkaya tarafından yapıldı",
                    "Made by Murat Kirazkaya",
                ));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .button("GitHub")
                        .on_hover_text(REPO_URL)
                        .clicked()
                    {
                        open_browser(REPO_URL);
                    }
                    if ui
                        .button(lang.tr("Profil", "Profile"))
                        .on_hover_text(ABOUT_URL)
                        .clicked()
                    {
                        open_browser(ABOUT_URL);
                    }
                    if ui
                        .button(lang.tr("Sürümler", "Releases"))
                        .on_hover_text(REPO_RELEASES)
                        .clicked()
                    {
                        open_browser(REPO_RELEASES);
                    }
                    if self.update_url.is_some() {
                        if ui
                            .add(
                                egui::Button::new(lang.tr("Güncelle", "Update"))
                                    .fill(Color32::from_rgb(32, 140, 120)),
                            )
                            .on_hover_text(lang.tr(
                                "İndir, bu exe’yi değiştir, aynı argümanlarla aç",
                                "Download, replace this exe, relaunch with the same args",
                            ))
                            .clicked()
                        {
                            self.begin_update();
                        }
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.strong(lang.tr("Ne yapar", "What it does"));
                ui.add_space(4.0);
                let groups: [(&str, &str, &[usize]); 4] = [
                    ("İnceleme", "Review", &[0, 1, 2, 3, 4, 5, 6, 7, 18]),
                    ("Zaman çizelgesi", "Timeline", &[8, 9, 10, 17, 19]),
                    ("Döngü", "Loop", &[11, 12]),
                    ("Uygulama", "App", &[13, 14, 15, 16]),
                ];
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (tr, en, idxs) in groups {
                            ui.weak(lang.tr(tr, en));
                            ui.add_space(2.0);
                            for i in idxs {
                                if let Some((a, b)) = ABOUT_FEATURES.get(*i) {
                                    ui.label(format!("   {}", lang.tr(a, b)));
                                }
                            }
                            ui.add_space(8.0);
                        }
                    });
                if ui.ctx().input(|i| i.viewport().close_requested()) {
                    self.about_open = false;
                }
            },
        );
    }

    fn toggle_play(&mut self) {
        self.playing = !self.playing;
        self.last_tick = None;
        self.accum = 0.0;
        if self.playing {
            self.start_audio();
        } else {
            self.stop_audio();
        }
    }

    fn step_frames(&mut self, ctx: &egui::Context, delta: i64) {
        let wrapped = self.loop_on.then(|| self.loop_range()).flatten().and_then(|(a, b)| {
            let cur = self.decoder.as_ref()?.current;
            if cur < a || cur > b {
                return None;
            }
            Some(loop_step(cur, delta, a, b) as i64 - cur as i64)
        });
        let delta = wrapped.unwrap_or(delta);
        if let Some(Err(e)) = self.decoder.as_mut().map(|d| d.step(delta)) {
            self.status = e;
        }
        self.sync_texture(ctx);
        if self.playing {
            self.start_audio();
        }
    }

    fn seek_to(&mut self, ctx: &egui::Context, frame: u64) {
        let Some(dec) = &self.decoder else {
            return;
        };
        let last = dec.info.frame_count.saturating_sub(1);
        let frame = frame.min(last);
        if dec.current == frame {
            return;
        }
        if let Some(Err(e)) = self.decoder.as_mut().map(|d| d.seek(frame)) {
            self.status = e;
        }
        self.sync_texture(ctx);
    }

    fn scrub_to(&mut self, ctx: &egui::Context, frame: u64, force: bool) {
        if !force {
            if let Some(t) = self.last_scrub_at {
                if t.elapsed() < std::time::Duration::from_millis(40) {
                    return;
                }
            }
        }
        self.last_scrub_at = Some(Instant::now());
        self.seek_to(ctx, frame);
    }

    fn loop_range(&self) -> Option<(u64, u64)> {
        let a = self.loop_in?;
        let b = self.loop_out?;
        if a <= b { Some((a, b)) } else { Some((b, a)) }
    }

    fn set_loop_in(&mut self, frame: u64) {
        let last = self
            .decoder
            .as_ref()
            .map(|d| d.info.frame_count.saturating_sub(1))
            .unwrap_or(frame);
        let frame = frame.min(last);
        self.loop_in = Some(frame);
        if self.loop_out.is_none() {
            self.loop_out = Some(frame);
        }
        if let (Some(a), Some(b)) = (self.loop_in, self.loop_out) {
            if a > b {
                self.loop_in = Some(b);
                self.loop_out = Some(a);
            }
        }
        self.loop_on = true;
    }

    fn set_loop_out(&mut self, frame: u64) {
        let last = self
            .decoder
            .as_ref()
            .map(|d| d.info.frame_count.saturating_sub(1))
            .unwrap_or(frame);
        let frame = frame.min(last);
        self.loop_out = Some(frame);
        if self.loop_in.is_none() {
            self.loop_in = Some(frame);
        }
        if let (Some(a), Some(b)) = (self.loop_in, self.loop_out) {
            if a > b {
                self.loop_in = Some(b);
                self.loop_out = Some(a);
            }
        }
        self.loop_on = true;
    }

    fn set_loop_span(&mut self, a: u64, b: u64) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.loop_in = Some(lo);
        self.loop_out = Some(hi);
        self.loop_on = true;
    }

    fn clear_loop(&mut self) {
        self.loop_in = None;
        self.loop_out = None;
        self.loop_on = false;
        self.range_drag = None;
    }

    fn should_loop_wrap(&self) -> bool {
        if !self.loop_on {
            return false;
        }
        let Some((a, b)) = self.loop_range() else {
            return false;
        };
        let Some(cur) = self.decoder.as_ref().map(|d| d.current) else {
            return false;
        };
        (cur < a || cur >= b) && b >= a
    }

    fn stop_audio(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }

    fn start_audio(&mut self) {
        self.stop_audio();
        let (buf, channels, sample_rate, speed) = {
            let Some(audio) = &self.audio else {
                return;
            };
            let Some(dec) = &self.decoder else {
                return;
            };
            let t = dec.current as f64 / dec.info.fps.max(0.001);
            let speed = (self.playback_fps / dec.info.fps).clamp(0.05, 8.0) as f32;
            let buf = audio.pcm_from(t);
            if buf.is_empty() {
                return;
            }
            (buf, audio.channels, audio.sample_rate, speed)
        };
        if self.output.is_none() {
            self.output = rodio::OutputStreamBuilder::open_default_stream().ok();
        }
        let Some(out) = self.output.as_ref() else {
            return;
        };
        let sink = rodio::Sink::connect_new(out.mixer());
        sink.set_speed(speed);
        sink.set_volume(self.volume);
        sink.append(rodio::buffer::SamplesBuffer::new(
            channels,
            sample_rate,
            buf,
        ));
        sink.play();
        self.sink = Some(sink);
    }

    fn sync_audio_speed(&mut self) {
        let Some(dec) = &self.decoder else {
            return;
        };
        let speed = (self.playback_fps / dec.info.fps).clamp(0.05, 8.0) as f32;
        if let Some(sink) = &self.sink {
            sink.set_speed(speed);
            sink.set_volume(self.volume);
        }
    }

    fn apply_volume(&mut self) {
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Escape) && self.focus {
                self.focus = false;
            }
            if i.consume_key(Modifiers::NONE, Key::F) {
                self.focus = !self.focus;
            }
        });
        if self.decoder.is_none() {
            return;
        }
        let mut play = false;
        let mut step = 0i64;
        let mut mark_in = false;
        let mut mark_out = false;
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::CTRL, Key::ArrowLeft) {
                step = -10;
            } else if i.consume_key(Modifiers::CTRL, Key::ArrowRight) {
                step = 10;
            } else if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                step = -1;
            } else if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                step = 1;
            }
            if i.consume_key(Modifiers::NONE, Key::Space) {
                play = true;
            }
            if i.consume_key(Modifiers::NONE, Key::I) {
                mark_in = true;
            }
            if i.consume_key(Modifiers::NONE, Key::O) {
                mark_out = true;
            }
        });
        if play {
            self.toggle_play();
        }
        if step != 0 {
            self.step_frames(ctx, step);
        }
        let at = self
            .playhead
            .or_else(|| self.decoder.as_ref().map(|d| d.current));
        if mark_in {
            if let Some(f) = at {
                self.set_loop_in(f);
            }
        }
        if mark_out {
            if let Some(f) = at {
                self.set_loop_out(f);
            }
        }
    }
}

impl eframe::App for PlayerApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_fetch(ctx);
        self.poll_export(ctx);
        self.poll_update(ctx);
        self.poll_update_state(ctx);
        self.show_about(ctx);
        self.show_update_modal(ctx);
        self.show_format_modal(ctx);
        if self.intro.is_none() {
            if let Some(p) = self.pending_open.take() {
                self.open_path(p, ctx);
            }
        }
        self.tick_intro(ctx);
        if self.lang.is_some() {
            let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
            if let Some(path) = dropped {
                self.open_path(path, ctx);
            }
            self.handle_keys(ctx);
        }

        if self.lang.is_none() || self.scrubbing || !self.playing {
            return;
        }
        let now = Instant::now();
        if let Some(last) = self.last_tick {
            self.accum += now.duration_since(last).as_secs_f64();
        }
        self.last_tick = Some(now);
        let step_s = 1.0 / self.playback_fps.max(0.01);
        let mut moved = false;
        while self.accum >= step_s {
            self.accum -= step_s;
            if self.should_loop_wrap() {
                if let Some((a, _)) = self.loop_range() {
                    self.seek_to(ctx, a);
                    self.start_audio();
                    moved = true;
                    continue;
                }
            }
            match self.decoder.as_mut().map(|d| d.advance()) {
                Some(Ok(true)) => moved = true,
                Some(Ok(false)) => {
                    if self.loop_on {
                        if let Some((a, _)) = self.loop_range() {
                            self.seek_to(ctx, a);
                            self.start_audio();
                            moved = true;
                            continue;
                        }
                    }
                    self.playing = false;
                    self.last_tick = None;
                    self.stop_audio();
                    break;
                }
                Some(Err(e)) => {
                    self.status = e;
                    self.playing = false;
                    self.stop_audio();
                    break;
                }
                None => break,
            }
        }
        if moved {
            self.sync_texture(ctx);
        }
        ctx.request_repaint();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.lang.is_none() {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Dil / Language");
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space((ui.available_width() - 220.0).max(0.0) * 0.5);
                            if ui
                                .add_sized([100.0, 36.0], egui::Button::new("Türkçe"))
                                .on_hover_text("Türkçe arayüz")
                                .clicked()
                            {
                                self.set_lang(Lang::Tr);
                            }
                            if ui
                                .add_sized([100.0, 36.0], egui::Button::new("English"))
                                .on_hover_text("English interface")
                                .clicked()
                            {
                                self.set_lang(Lang::En);
                            }
                        });
                    });
                });
            });
            return;
        }
        let lang = self.lang();
        if !self.focus {
        egui::Panel::top("top").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new((self.logo.id(), egui::vec2(22.0, 22.0))))
                    .on_hover_text("VFX Player");
                if ui
                    .button(lang.tr("Aç", "Open"))
                    .on_hover_text(lang.tr("Video dosyası aç", "Open a video file"))
                    .clicked()
                {
                    self.pick_file(&ctx);
                }
                ui.separator();
                let url_hint = lang.tr("https://…", "https://…");
                let fetch_busy = self.fetch.is_some();
                ui.add_enabled_ui(!fetch_busy, |ui| {
                    let resp = ui
                        .add(
                            egui::TextEdit::singleline(&mut self.url)
                                .desired_width(200.0)
                                .hint_text(url_hint),
                        )
                        .on_hover_text(lang.tr(
                            "YouTube, Instagram, TikTok, Facebook, X, Reddit",
                            "YouTube, Instagram, TikTok, Facebook, X, Reddit",
                        ));
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        self.start_url();
                    }
                    if ui
                        .button(lang.tr("URL aç", "Open URL"))
                        .on_hover_text(lang.tr("Enter veya tıkla", "Enter or click"))
                        .clicked()
                    {
                        self.start_url();
                    }
                });
                ui.separator();
                ui.weak(&self.status)
                    .on_hover_text(lang.tr("Açık dosya / durum", "Open file / status"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(lang.tr("Hakkında", "About"))
                        .on_hover_text(lang.tr("Uygulama bilgisi", "About this app"))
                        .clicked()
                    {
                        self.about_open = true;
                    }
                    if ui
                        .button(lang.tr("Odak", "Focus"))
                        .on_hover_text(lang.tr(
                            "Sadece video, cetvel ve oynatma (F / Esc)",
                            "Video, ruler and transport only (F / Esc)",
                        ))
                        .clicked()
                    {
                        self.focus = true;
                    }
                    if self.audio.is_some()
                        && ui
                            .selectable_label(self.wave_on, lang.tr("Dalga", "Wave"))
                            .on_hover_text(lang.tr(
                                "Ses dalgasını göster / gizle",
                                "Show / hide the waveform",
                            ))
                            .clicked()
                    {
                        self.wave_on = !self.wave_on;
                    }
                    if self.update_url.is_some() {
                        if ui
                            .add(egui::Button::new(lang.tr("Güncelle", "Update")).fill(
                                Color32::from_rgb(32, 140, 120),
                            ))
                            .on_hover_text(lang.tr(
                                "İndir, bu exe’yi değiştir, aynı argümanlarla aç",
                                "Download, replace this exe, relaunch with the same args",
                            ))
                            .clicked()
                        {
                            self.begin_update();
                        }
                    }
                    if ui
                        .selectable_label(!self.dark, lang.tr("Açık", "Light"))
                        .on_hover_text(lang.tr("Açık tema", "Light theme"))
                        .clicked()
                    {
                        self.set_dark(&ctx, false);
                    }
                    if ui
                        .selectable_label(self.dark, lang.tr("Koyu", "Dark"))
                        .on_hover_text(lang.tr("Koyu tema", "Dark theme"))
                        .clicked()
                    {
                        self.set_dark(&ctx, true);
                    }
                    ui.separator();
                    if ui
                        .selectable_label(self.lang == Some(Lang::En), "EN")
                        .on_hover_text("English")
                        .clicked()
                    {
                        self.set_lang(Lang::En);
                    }
                    if ui
                        .selectable_label(self.lang == Some(Lang::Tr), "TR")
                        .on_hover_text("Türkçe")
                        .clicked()
                    {
                        self.set_lang(Lang::Tr);
                    }
                });
            });
        });
        }

        egui::Panel::bottom("controls").show_inside(ui, |ui| {
            let mut hit_frame = None;
            let mut hit_rect = None;
            let range = self.loop_range();
            if let Some((total, shown, fps)) = self.decoder.as_ref().map(|d| {
                (
                    d.info.frame_count.max(1),
                    self.playhead.unwrap_or(d.current),
                    d.info.fps,
                )
            }) {
                if self.playing {
                    keep_tl_in_view(&mut self.tl_zoom, &mut self.tl_scroll, shown, total);
                }
                if self.wave_on {
                if let Some(audio) = &self.audio {
                    if !self.focus {
                    drag_bar_height(
                        ui,
                        &mut self.wave_h,
                        24.0,
                        220.0,
                        lang.tr("Dalga yüksekliği", "Waveform height"),
                    );
                    }
                    let (rect, f) = paint_waveform(
                        ui,
                        &audio.peaks,
                        shown,
                        total,
                        range,
                        self.wave_h,
                        &mut self.tl_zoom,
                        &mut self.tl_scroll,
                    );
                    if let Some(f) = f {
                        hit_frame = Some(f);
                        hit_rect = Some(rect);
                    }
                }
                }
                if !self.focus {
                drag_bar_height(
                    ui,
                    &mut self.bar_h,
                    22.0,
                    100.0,
                    lang.tr("Cetvel yüksekliği", "Ruler height"),
                );
                }
                let (rect, f) = paint_framebar(
                    ui,
                    shown,
                    total,
                    fps,
                    range,
                    self.bar_h,
                    &mut self.tl_zoom,
                    &mut self.tl_scroll,
                );
                if let Some(f) = f {
                    hit_frame = Some(f);
                    hit_rect = Some(rect);
                }
                ui.add_space(4.0);
            }
            if let (Some(f), Some(rect)) = (hit_frame, hit_rect) {
                let total = self
                    .decoder
                    .as_ref()
                    .map(|d| d.info.frame_count.max(1))
                    .unwrap_or(1);
                let shift = ui.input(|i| i.modifiers.shift);
                let pos_x = ui.input(|i| i.pointer.latest_pos().map(|p| p.x));
                if self.range_drag.is_none() && !self.scrubbing {
                    let view = tl_view(total, self.tl_zoom, self.tl_scroll);
                    self.range_drag =
                        classify_range_drag(f, pos_x, rect, range, shift, view);
                    if self.range_drag.is_some() {
                        self.loop_on = true;
                    }
                }
                match self.range_drag {
                    Some(RangeDrag::Create { origin }) => {
                        self.set_loop_span(origin, f);
                        ui.ctx().request_repaint();
                    }
                    Some(RangeDrag::In) => {
                        self.set_loop_in(f);
                        ui.ctx().request_repaint();
                    }
                    Some(RangeDrag::Out) => {
                        self.set_loop_out(f);
                        ui.ctx().request_repaint();
                    }
                    None => {
                        self.playhead = Some(f);
                        self.stop_audio();
                        self.scrub_to(&ctx, f, !self.scrubbing);
                        self.scrubbing = true;
                        ui.ctx().request_repaint();
                    }
                }
            } else {
                self.range_drag = None;
                if self.scrubbing {
                    self.scrubbing = false;
                    if let Some(f) = self.playhead.take() {
                        self.scrub_to(&ctx, f, true);
                    }
                    if self.playing {
                        self.start_audio();
                    }
                }
            }
            {
                let enabled = self.decoder.is_some();
                let w = ui.available_width();
                let h = 34.0;
                let (row, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
                let mid = Rect::from_center_size(row.center(), egui::vec2(248.0, h));
                let left = Rect::from_min_max(row.left_top(), Pos2::new(mid.left() - 6.0, row.bottom()));
                let right = Rect::from_min_max(Pos2::new(mid.right() + 6.0, row.top()), row.right_bottom());
                if !self.focus {
                ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            let at = self
                                .playhead
                                .or_else(|| self.decoder.as_ref().map(|d| d.current));
                            let i_lab = self
                                .loop_range()
                                .map(|(a, _)| format!("I {a}"))
                                .unwrap_or_else(|| "I".into());
                            let o_lab = self
                                .loop_range()
                                .map(|(_, b)| format!("O {b}"))
                                .unwrap_or_else(|| "O".into());
                            if ui
                                .button(i_lab)
                                .on_hover_text(lang.tr("Döngü girişi (I)", "Loop in (I)"))
                                .clicked()
                            {
                                if let Some(f) = at {
                                    self.set_loop_in(f);
                                }
                            }
                            if ui
                                .button(o_lab)
                                .on_hover_text(lang.tr("Döngü çıkışı (O)", "Loop out (O)"))
                                .clicked()
                            {
                                if let Some(f) = at {
                                    self.set_loop_out(f);
                                }
                            }
                            ui.checkbox(&mut self.loop_on, "Loop").on_hover_text(
                                lang.tr("Aralıkta döngü (I/O)", "Loop the I/O range"),
                            );
                            if self.loop_range().is_some() {
                                if ui
                                    .small_button("×")
                                    .on_hover_text(lang.tr("Aralığı sil", "Clear range"))
                                    .clicked()
                                {
                                    self.clear_loop();
                                }
                                ui.add_enabled_ui(self.export.is_none(), |ui| {
                                    if ui
                                        .button(lang.tr("Kırp ve farklı kaydet", "Trim and Save As"))
                                        .on_hover_text(lang.tr(
                                            "Döngüyü aynı formatta yeni dosyaya yaz",
                                            "Write the loop to a new file in the same format",
                                        ))
                                        .clicked()
                                    {
                                        self.export_loop();
                                    }
                                });
                            } else {
                                ui.weak(lang.tr("Shift+sürükle", "Shift+drag"))
                                    .on_hover_text(lang.tr(
                                        "Cetvelde Shift+sürükle ile I/O seç",
                                        "Shift+drag on the ruler to set I/O",
                                    ));
                            }
                        });
                    });
                });
                }
                ui.scope_builder(egui::UiBuilder::new().max_rect(mid), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            if ui
                                .add_sized([36.0, 28.0], egui::Button::new("«"))
                                .on_hover_text(lang.tr("−10 kare (Ctrl+←)", "−10 frames (Ctrl+←)"))
                                .clicked()
                            {
                                self.step_frames(&ctx, -10);
                            }
                            if ui
                                .add_sized([40.0, 30.0], egui::Button::new("‹"))
                                .on_hover_text(lang.tr("−1 kare (←)", "−1 frame (←)"))
                                .clicked()
                            {
                                self.step_frames(&ctx, -1);
                            }
                            let play = if self.playing { "❚❚" } else { "▶" };
                            if ui
                                .add_sized([52.0, 32.0], egui::Button::new(play))
                                .on_hover_text(lang.tr(
                                    "Oynat / duraklat (Space)",
                                    "Play / pause (Space)",
                                ))
                                .clicked()
                            {
                                self.toggle_play();
                            }
                            if ui
                                .add_sized([40.0, 30.0], egui::Button::new("›"))
                                .on_hover_text(lang.tr("+1 kare (→)", "+1 frame (→)"))
                                .clicked()
                            {
                                self.step_frames(&ctx, 1);
                            }
                            if ui
                                .add_sized([36.0, 28.0], egui::Button::new("»"))
                                .on_hover_text(lang.tr("+10 kare (Ctrl+→)", "+10 frames (Ctrl+→)"))
                                .clicked()
                            {
                                self.step_frames(&ctx, 10);
                            }
                        });
                    });
                });
                if !self.focus {
                ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            ui.allocate_ui_with_layout(
                                egui::vec2(168.0, 20.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let mut vol_pct = (self.volume * 100.0).round();
                                    let vol_c = if vol_pct > 110.0 {
                                        Color32::from_rgb(220, 70, 60)
                                    } else if vol_pct >= 90.0 {
                                        Color32::from_rgb(210, 170, 30)
                                    } else {
                                        ui.visuals().text_color()
                                    };
                                    ui.weak("♪").on_hover_text(lang.tr("Ses", "Volume"));
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut vol_pct, 0.0..=125.0)
                                                .show_value(false)
                                                .trailing_fill(true),
                                        )
                                        .on_hover_text(lang.tr(
                                            "Ses 0–125% (%100 üstü boost)",
                                            "Volume 0–125% (boost above 100%)",
                                        ))
                                        .changed()
                                    {
                                        self.volume = (vol_pct / 100.0).clamp(0.0, 1.25);
                                        self.apply_volume();
                                    }
                                    ui.colored_label(vol_c, format!("{vol_pct:.0}%"));
                                },
                            );
                        });
                    });
                });
                }
            }

            if !self.focus {
            if let Some((current, total, src_fps)) = self.decoder.as_ref().map(|d| {
                (
                    self.playhead.unwrap_or(d.current),
                    d.info.frame_count,
                    d.info.fps,
                )
            }) {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{current} / {total}"))
                        .on_hover_text(lang.tr("Kare / toplam", "Frame / total"));
                    let ms_total = (current as f64 * 1000.0 / src_fps.max(0.001)).round().max(0.0) as u64;
                    let sec = ms_total / 1000;
                    let ms = ms_total % 1000;
                    ui.monospace(format!("{sec}"))
                        .on_hover_text(lang.tr("saniye", "seconds"));
                    ui.monospace(":");
                    ui.monospace(format!("{ms:03}"))
                        .on_hover_text(lang.tr("milisaniye", "milliseconds"));
                    ui.weak(format!("{src_fps:.3} fps"))
                        .on_hover_text(lang.tr("Kaynak kare hızı", "Source frame rate"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut pct = (self.zoom * 100.0).clamp(5.0, 1600.0);
                        if ui
                            .add(
                                egui::DragValue::new(&mut pct)
                                    .range(5.0..=1600.0)
                                    .suffix("%")
                                    .speed(1.0)
                                    .max_decimals(0),
                            )
                            .on_hover_text(lang.tr(
                                "Önizleme zoom (videoda Ctrl+tekerlek)",
                                "Preview zoom (Ctrl+wheel on the video)",
                            ))
                            .changed()
                        {
                            self.zoom_mode = ZoomMode::Manual;
                            self.zoom = (pct / 100.0).clamp(ZOOM_MIN, ZOOM_MAX);
                        }
                        if ui
                            .selectable_label(self.zoom_mode == ZoomMode::FitHeight, "Fit H")
                            .on_hover_text(lang.tr(
                                "Videoyu yüksekliğe sığdır",
                                "Fit video to height",
                            ))
                            .clicked()
                        {
                            self.zoom_mode = ZoomMode::FitHeight;
                        }
                        if ui
                            .selectable_label(self.zoom_mode == ZoomMode::FitWidth, "Fit W")
                            .on_hover_text(lang.tr(
                                "Videoyu genişliğe sığdır",
                                "Fit video to width",
                            ))
                            .clicked()
                        {
                            self.zoom_mode = ZoomMode::FitWidth;
                        }
                        ui.weak(lang.tr("Görüntü", "View"))
                            .on_hover_text(lang.tr("Önizleme sığdır / zoom", "Preview fit / zoom"));
                        ui.separator();
                        if ui
                            .small_button(lang.tr("Reset FPS", "Reset FPS"))
                            .on_hover_text(lang.tr(
                                "Oynatma hızını kaynak FPS’e al",
                                "Reset playback speed to source FPS",
                            ))
                            .clicked()
                        {
                            self.playback_fps = src_fps;
                        }
                        ui.add(
                            egui::DragValue::new(&mut self.playback_fps)
                                .range(0.01..=240.0)
                                .speed(0.01)
                                .min_decimals(0)
                                .max_decimals(2)
                                .suffix(" fps"),
                        )
                        .on_hover_text(lang.tr("Oynatma hızı (FPS)", "Playback speed (FPS)"));
                        ui.separator();
                        let zmax = total.saturating_sub(1).max(1) as f32;
                        if ui
                            .small_button("+")
                            .on_hover_text(lang.tr(
                                "Zaman çizelgesini yakınlaştır (cetvelde Ctrl+tekerlek)",
                                "Zoom timeline in (Ctrl+wheel on the ruler)",
                            ))
                            .clicked()
                        {
                            self.tl_zoom = (self.tl_zoom * 1.25).clamp(1.0, zmax);
                            center_tl(&mut self.tl_scroll, self.tl_zoom, current, total);
                        }
                        if ui
                            .small_button("−")
                            .on_hover_text(lang.tr(
                                "Zaman çizelgesini uzaklaştır (cetvelde Ctrl+tekerlek)",
                                "Zoom timeline out (Ctrl+wheel on the ruler)",
                            ))
                            .clicked()
                        {
                            self.tl_zoom = (self.tl_zoom / 1.25).clamp(1.0, zmax);
                            center_tl(&mut self.tl_scroll, self.tl_zoom, current, total);
                        }
                    });
                });
                if self.playing {
                    self.sync_audio_speed();
                }
            } else {
                ui.label(lang.tr(
                    "Video sürükle-bırak, Aç veya URL.",
                    "Drop a video, Open, or paste a URL.",
                ));
            }
            }
            if self.focus && self.playing {
                self.sync_audio_speed();
            }
            ui.add_space(if self.focus { 2.0 } else { 6.0 });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some((tex_id, w, h)) = self.texture.as_ref().zip(self.decoder.as_ref()).map(
                |(tex, dec)| (tex.id(), dec.info.width as f32, dec.info.height as f32),
            ) {
                let avail = ui.available_size();
                let (rect, resp) = ui.allocate_exact_size(avail, Sense::drag());
                if resp.hovered() {
                    let zdelta = ui.input(|i| i.zoom_delta());
                    if (zdelta - 1.0).abs() > 0.001 {
                        let current = view_scale(avail.x, avail.y, w, h, self.zoom_mode, self.zoom);
                        self.zoom_mode = ZoomMode::Manual;
                        self.zoom = (current * zdelta).clamp(ZOOM_MIN, ZOOM_MAX);
                    }
                }
                let scale = view_scale(avail.x, avail.y, w, h, self.zoom_mode, self.zoom);
                if self.zoom_mode != ZoomMode::Manual {
                    self.zoom = scale;
                }
                let size = egui::vec2(w * scale, h * scale);
                if self.last_scale > 0.0 && (scale - self.last_scale).abs() > 1e-5 {
                    let old_size = egui::vec2(w * self.last_scale, h * self.last_scale);
                    let anchor = resp
                        .hover_pos()
                        .map(|p| p - rect.min)
                        .unwrap_or(avail * 0.5);
                    self.pan = remap_pan(self.pan, old_size, size, avail, anchor);
                }
                self.last_scale = scale;
                let overflowing = size.x > avail.x + 0.5 || size.y > avail.y + 0.5;
                if overflowing && resp.dragged() {
                    self.pan -= resp.drag_delta();
                }
                self.pan = pan_from_origin(image_origin(size, avail, self.pan), size, avail);
                if overflowing {
                    let _ = resp.clone().on_hover_cursor(if resp.dragged() {
                        CursorIcon::Grabbing
                    } else {
                        CursorIcon::Grab
                    });
                }
                let origin = image_origin(size, avail, self.pan);
                let dest = Rect::from_min_size(rect.min + origin, size);
                ui.painter_at(rect).image(
                    tex_id,
                    dest,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(lang.tr("Video yok", "No video"));
                });
            }
        });
    }
}

fn capture_named_window(title: &str, dest: &Path) {
    let script = std::env::temp_dir().join("vfx_cap_window.ps1");
    let body = r#"param($Title,$Out)
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class Cap {
  public delegate bool CB(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(CB cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  public struct RECT { public int L; public int T; public int R; public int B; }
  public static string RectOf(string part) {
    string found = null;
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      var sb = new StringBuilder(256);
      GetWindowText(h, sb, 256);
      if (sb.ToString().IndexOf(part, StringComparison.OrdinalIgnoreCase) >= 0) {
        RECT r; GetWindowRect(h, out r);
        found = r.L + "," + r.T + "," + (r.R-r.L) + "," + (r.B-r.T);
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
'@
$s = [Cap]::RectOf($Title)
if (-not $s) { $s = [Cap]::RectOf('VFX Player') }
if (-not $s) { exit 1 }
$p = $s.Split(',')
$bmp = New-Object System.Drawing.Bitmap ([int]$p[2]), ([int]$p[3])
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen([int]$p[0], [int]$p[1], 0, 0, $bmp.Size)
$bmp.Save($Out)
$g.Dispose(); $bmp.Dispose()
"#;
    let _ = std::fs::write(&script, body);
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script.to_str().unwrap_or(""),
            "-Title",
            title,
            "-Out",
            dest.to_str().unwrap_or(""),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

fn save_color_image(ffmpeg: &Path, image: &ColorImage, dest: &Path) -> Result<(), String> {
    let [w, h] = image.size;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for c in &image.pixels {
        rgba.extend_from_slice(&c.to_array());
    }
    let dest = dest.to_str().ok_or("shot path")?;
    let mut child = Command::new(ffmpeg)
        .args([
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{w}x{h}"),
            "-i",
            "-",
            "-frames:v",
            "1",
            dest,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("stdin")?
        .write_all(&rgba)
        .map_err(|e| e.to_string())?;
    let ok = child.wait().map_err(|e| e.to_string())?.success();
    if ok {
        Ok(())
    } else {
        Err("ffmpeg png".into())
    }
}

fn parse_ver(s: &str) -> (u64, u64, u64) {
    let s = s.trim().trim_start_matches('v');
    let mut p = s.split('.');
    let a = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let b = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let c = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    (a, b, c)
}

fn json_u32(body: &str, key: &str) -> Option<u32> {
    json_f32(body, key).map(|n| n as u32)
}

fn json_f32(body: &str, key: &str) -> Option<f32> {
    let pat = format!("\"{key}\"");
    let i = body.find(&pat)?;
    let rest = body[i + pat.len()..].trim_start().trim_start_matches(':').trim_start();
    if rest.starts_with("null") {
        return None;
    }
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num.parse().ok()
}

fn json_top_objects(s: &str) -> Vec<&str> {
    let s = s.trim();
    let s = s.strip_prefix('[').unwrap_or(s);
    let s = s.strip_suffix(']').unwrap_or(s);
    let mut out = Vec::new();
    let mut start = None;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, c) in s.char_indices() {
        if in_str {
            if esc {
                esc = false;
                continue;
            }
            if c == '\\' {
                esc = true;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(a) = start {
                        out.push(&s[a..=i]);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn json_quoted(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = body.find(&pat)?;
    let rest = body[i + pat.len()..].trim_start().trim_start_matches(':').trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].replace("\\/", "/"))
}

fn latest_update_url() -> Option<String> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "8",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: vfx-editor",
            REPO_API,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let body = String::from_utf8_lossy(&out.stdout);
    let tag = json_quoted(&body, "tag_name")?;
    if parse_ver(&tag) <= parse_ver(APP_VERSION) {
        return None;
    }
    Some(tag)
}

fn open_browser(url: &str) {
    let _ = Command::new("cmd")
        .args(["/C", "start", "", url])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

fn update_state_path() -> PathBuf {
    std::env::temp_dir().join("vfx-editor").join("update_state.txt")
}

fn just_updated_path() -> PathBuf {
    bundle::data_dir().join("just_updated")
}

fn take_just_updated() -> bool {
    let p = just_updated_path();
    if !p.is_file() {
        return false;
    }
    let _ = std::fs::remove_file(&p);
    true
}

fn spawn_self_update() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dest = exe.to_str().ok_or("exe path")?;
    let dir = std::env::temp_dir().join("vfx-editor");
    std::fs::create_dir_all(&dir).map_err(|e| format!("tmp: {e}"))?;
    let _ = std::fs::create_dir_all(bundle::data_dir());
    let script = dir.join("update.ps1");
    let args_file = dir.join("update_args.txt");
    let extra: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--updated")
        .collect();
    std::fs::write(&args_file, extra.join("\n")).map_err(|e| format!("tmp: {e}"))?;
    std::fs::write(
        &script,
        format!(
            r#"param($AppPid, $Dest, $ArgsFile)
$st = Join-Path $env:TEMP 'vfx-editor\update_state.txt'
$flag = Join-Path $env:LOCALAPPDATA 'vfx-editor\just_updated'
$tmp = Join-Path $env:TEMP 'vfx-editor\vfx_editor_update.exe'
New-Item -ItemType Directory -Force (Split-Path $tmp) | Out-Null
& curl.exe -fsSL --max-redirs 5 -o $tmp -- '{url}'
if (-not $? -or -not (Test-Path $tmp) -or (Get-Item $tmp).Length -lt 1MB) {{ Set-Content -LiteralPath $st 'fail'; exit 1 }}
New-Item -ItemType Directory -Force (Split-Path $flag) | Out-Null
Set-Content -LiteralPath $flag '1'
& taskkill /PID $AppPid /T /F | Out-Null
$ok = $false
foreach ($i in 1..50) {{
  Start-Sleep -Milliseconds 200
  try {{ Copy-Item -LiteralPath $tmp -Destination $Dest -Force; $ok = $true; break }} catch {{}}
}}
$run = if ($ok) {{ $Dest }} else {{ $tmp }}
$extra = @('--updated')
if ($ArgsFile -and (Test-Path -LiteralPath $ArgsFile)) {{
  $extra += @(Get-Content -LiteralPath $ArgsFile -Encoding utf8 | Where-Object {{ $_ -ne '' -and $_ -ne '--updated' }})
}}
Start-Process -FilePath $run -ArgumentList $extra
"#,
            url = EXE_DOWNLOAD
        ),
    )
    .map_err(|e| format!("tmp: {e}"))?;
    let runner = dir.join("run_update.cmd");
    std::fs::write(
        &runner,
        format!(
            "powershell -NoProfile -ExecutionPolicy Bypass -File \"{}\" -AppPid {} -Dest \"{}\" -ArgsFile \"{}\"\r\n",
            script.display(),
            std::process::id(),
            dest,
            args_file.display()
        ),
    )
    .map_err(|e| format!("tmp: {e}"))?;
    // start /B: updater must not be our child or taskkill /T kills it too
    Command::new("cmd")
        .args(["/C", "start", "", "/B"])
        .arg(&runner)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
fn url_ext(url: &str) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    VIDEO_EXTS
        .iter()
        .copied()
        .find(|e| ext.as_deref() == Some(*e))
        .unwrap_or("mp4")
}

fn url_allowed(url: &str) -> bool {
    let Some(rest) = url.split("://").nth(1) else {
        return false;
    };
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return false;
    }
    let host = rest
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    const HOSTS: &[&str] = &[
        "youtube.com",
        "youtu.be",
        "m.youtube.com",
        "music.youtube.com",
        "instagram.com",
        "tiktok.com",
        "vm.tiktok.com",
        "vt.tiktok.com",
        "facebook.com",
        "fb.com",
        "fb.watch",
        "m.facebook.com",
        "x.com",
        "twitter.com",
        "mobile.twitter.com",
        "reddit.com",
        "old.reddit.com",
        "v.redd.it",
    ];
    HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

fn codec_v(s: &str) -> Option<&'static str> {
    let s = s.to_ascii_lowercase();
    if s.is_empty() || s == "none" {
        return None;
    }
    Some(if s.starts_with("avc") || s.contains("h264") {
        "H264"
    } else if s.starts_with("vp9") || s.starts_with("vp09") {
        "VP9"
    } else if s.starts_with("av01") || s.starts_with("av1") {
        "AV1"
    } else if s.starts_with("hev") || s.starts_with("hvc") || s.contains("h265") {
        "H265"
    } else {
        "Video"
    })
}

fn codec_a(s: &str) -> Option<&'static str> {
    let s = s.to_ascii_lowercase();
    if s.is_empty() || s == "none" {
        return None;
    }
    Some(if s.starts_with("mp4a") || s.contains("aac") {
        "AAC"
    } else if s.contains("opus") {
        "Opus"
    } else if s.contains("mp3") {
        "MP3"
    } else {
        "Audio"
    })
}

fn is_none_codec(s: &str) -> bool {
    s.is_empty() || s.eq_ignore_ascii_case("none")
}

fn list_formats(
    url: &str,
    lang: Lang,
    tx: &mpsc::Sender<FetchEvent>,
) -> Result<Vec<FormatOpt>, String> {
    let _ = tx.send(FetchEvent::Status(
        lang.tr("Çıkarılıyor…", "Extracting…").into(),
    ));
    let ytdlp = bundle::extract_ytdlp()?;
    let out = Command::new(&ytdlp)
        .args([
            "--skip-download",
            "--no-playlist",
            "--no-warnings",
            "--print",
            "%(formats)j",
            "--",
            url,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("yt-dlp: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err
            .lines()
            .rev()
            .find(|l| l.contains("ERROR") || !l.trim().is_empty())
            .unwrap_or("");
        return Err(if line.is_empty() {
            lang.tr("Çıkarma başarısız", "Extract failed").into()
        } else {
            line.to_string()
        });
    }
    let json = String::from_utf8_lossy(&out.stdout);
    let opts = build_format_opts(&json);
    if opts.is_empty() {
        return Err(lang.tr("Format yok", "No formats").into());
    }
    Ok(opts)
}

fn build_format_opts(json: &str) -> Vec<FormatOpt> {
    struct Raw {
        id: String,
        height: u32,
        vcodec: String,
        acodec: String,
        tbr: f32,
        note: String,
    }
    let mut raw = Vec::new();
    for obj in json_top_objects(json) {
        let Some(id) = json_quoted(obj, "format_id") else {
            continue;
        };
        let proto = json_quoted(obj, "protocol").unwrap_or_default();
        if proto.contains("mhtml") || proto.contains("storyboard") {
            continue;
        }
        let ext = json_quoted(obj, "ext").unwrap_or_default();
        if matches!(ext.as_str(), "mhtml" | "jpg" | "png" | "webp") {
            continue;
        }
        let vcodec = json_quoted(obj, "vcodec").unwrap_or_else(|| "none".into());
        let acodec = json_quoted(obj, "acodec").unwrap_or_else(|| "none".into());
        if is_none_codec(&vcodec) && is_none_codec(&acodec) {
            continue;
        }
        raw.push(Raw {
            id,
            height: json_u32(obj, "height").unwrap_or(0),
            vcodec,
            acodec,
            tbr: json_f32(obj, "tbr")
                .or_else(|| json_f32(obj, "vbr"))
                .unwrap_or(0.0),
            note: json_quoted(obj, "format_note").unwrap_or_default(),
        });
    }
    let mut audios: Vec<&Raw> = raw
        .iter()
        .filter(|f| is_none_codec(&f.vcodec) && !is_none_codec(&f.acodec))
        .collect();
    audios.sort_by(|a, b| b.tbr.partial_cmp(&a.tbr).unwrap_or(std::cmp::Ordering::Equal));
    let best_audio = |want: &str| -> Option<&Raw> {
        audios
            .iter()
            .copied()
            .find(|a| codec_a(&a.acodec) == Some(want))
            .or_else(|| audios.first().copied())
    };
    let mut best: Vec<&Raw> = Vec::new();
    for v in &raw {
        if v.height == 0 || codec_v(&v.vcodec).is_none() {
            continue;
        }
        if let Some(old) = best.iter_mut().find(|o| {
            o.height == v.height && codec_v(&o.vcodec) == codec_v(&v.vcodec)
        }) {
            if v.tbr > old.tbr {
                *old = v;
            }
        } else {
            best.push(v);
        }
    }
    best.sort_by(|a, b| b.height.cmp(&a.height));
    let mut opts = Vec::new();
    for v in best {
        let vn = codec_v(&v.vcodec).unwrap_or("Video");
        let (spec, an) = if !is_none_codec(&v.acodec) {
            (v.id.clone(), codec_a(&v.acodec).unwrap_or("Audio"))
        } else if let Some(a) = best_audio(if vn == "H264" { "AAC" } else { "Opus" }) {
            (
                format!("{}+{}", v.id, a.id),
                codec_a(&a.acodec).unwrap_or("Audio"),
            )
        } else {
            (v.id.clone(), "—")
        };
        let mut label = if an == "—" {
            format!("{}p {vn}", v.height)
        } else {
            format!("{}p {vn} + {an}", v.height)
        };
        if v.note.to_ascii_uppercase().contains("HDR") {
            label.push_str(" HDR");
        }
        if opts.iter().any(|o: &FormatOpt| o.label == label) {
            continue;
        }
        let merge = if vn == "H264" && an == "AAC" {
            "mp4"
        } else {
            "mkv"
        };
        opts.push(FormatOpt { label, spec, merge });
        if opts.len() >= 16 {
            break;
        }
    }
    opts
}

fn download_ytdlp(
    url: &str,
    lang: Lang,
    ffmpeg: Option<&Path>,
    spec: &str,
    merge: &str,
    tx: &mpsc::Sender<FetchEvent>,
) -> Result<PathBuf, String> {
    let ytdlp = bundle::extract_ytdlp()?;
    let dir = std::env::temp_dir().join("vfx-editor");
    std::fs::create_dir_all(&dir).map_err(|e| format!("tmp: {e}"))?;
    let stem = format!(
        "dl{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    let out = dir.join(format!("{stem}.%(ext)s"));
    let mut cmd = Command::new(&ytdlp);
    cmd.args([
        "--no-playlist",
        "--newline",
        "--no-mtime",
        "--restrict-filenames",
        "-f",
        spec,
        "--merge-output-format",
        merge,
        "-o",
    ])
    .arg(&out);
    if let Some(ff) = ffmpeg {
        cmd.arg("--ffmpeg-location").arg(ff);
    }
    cmd.arg("--").arg(url);
    let mut child = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("yt-dlp: {e}"))?;
    let mut last_err = String::new();
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line.contains("ERROR") {
                last_err = line.clone();
            }
            if let Some(s) = ytdlp_status(&line, lang) {
                let _ = tx.send(FetchEvent::Status(s));
            }
        }
    }
    let ok = child.wait().map_err(|e| e.to_string())?.success();
    if !ok {
        if last_err.is_empty() {
            return Err(lang.tr("İndirme başarısız", "Download failed").into());
        }
        return Err(last_err);
    }
    find_download(&dir, &stem).ok_or_else(|| lang.tr("Boş indirme", "Empty download").into())
}

fn ytdlp_status(line: &str, lang: Lang) -> Option<String> {
    let line = line.trim();
    if let Some(p) = download_pct(line) {
        let mut s = format!("{} {p}%", lang.tr("İndiriliyor", "Downloading"));
        if let Some(eta) = line.split("ETA ").nth(1) {
            let eta = eta.split_whitespace().next().unwrap_or("");
            if !eta.is_empty() {
                s.push_str(" · ");
                s.push_str(eta);
            }
        }
        return Some(s);
    }
    if line.contains("[Merger]") || line.contains("[Fixup]") || line.contains("[ExtractAudio]") {
        return Some(lang.tr("Birleştiriliyor…", "Merging…").into());
    }
    if line.contains("Extracting")
        || line.contains("Downloading webpage")
        || line.contains("Downloading player")
        || line.contains("[info]")
    {
        return Some(lang.tr("Çıkarılıyor…", "Extracting…").into());
    }
    if line.starts_with("[download]") {
        return Some(lang.tr("İndiriliyor…", "Downloading…").into());
    }
    None
}

fn download_pct(line: &str) -> Option<u32> {
    let (left, _) = line.split_once('%')?;
    let n = left.split_whitespace().last()?;
    n.parse::<f32>().ok().map(|p| p.clamp(0.0, 100.0) as u32)
}

fn find_download(dir: &Path, stem: &str) -> Option<PathBuf> {
    for ext in ["mp4", "mkv", "webm", "mov"] {
        let p = dir.join(format!("{stem}.{ext}"));
        if p.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            return Some(p);
        }
    }
    let mut best: Option<(u64, PathBuf)> = None;
    for e in std::fs::read_dir(dir).ok()? {
        let e = e.ok()?;
        let name = e.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(stem) || name.contains(".part") || name.ends_with(".ytdl") {
            continue;
        }
        let ext = Path::new(name.as_ref())
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())?;
        if !VIDEO_EXTS.iter().any(|v| *v == ext) {
            continue;
        }
        let len = e.metadata().ok()?.len();
        if len == 0 {
            continue;
        }
        if best.as_ref().map(|(n, _)| len > *n).unwrap_or(true) {
            best = Some((len, e.path()));
        }
    }
    best.map(|(_, p)| p)
}


#[derive(Clone, Copy)]
struct TimeView {
    start: f32,
    span: f32,
}

fn tl_last(total: u64) -> f32 {
    total.saturating_sub(1).max(1) as f32
}

fn tl_span(total: u64, zoom: f32) -> f32 {
    let last = tl_last(total);
    (last / zoom.clamp(1.0, last.max(1.0))).max(1.0)
}

fn tl_max_start(total: u64, zoom: f32) -> f32 {
    (tl_last(total) - tl_span(total, zoom)).max(0.0)
}

fn tl_view(total: u64, zoom: f32, scroll: f32) -> TimeView {
    TimeView {
        start: scroll.clamp(0.0, 1.0) * tl_max_start(total, zoom),
        span: tl_span(total, zoom),
    }
}

fn center_tl(scroll: &mut f32, zoom: f32, frame: u64, total: u64) {
    let max_start = tl_max_start(total, zoom);
    if max_start < 1e-3 {
        *scroll = 0.0;
        return;
    }
    *scroll = ((frame as f32 - tl_span(total, zoom) * 0.5) / max_start).clamp(0.0, 1.0);
}

fn keep_tl_in_view(zoom: &mut f32, scroll: &mut f32, frame: u64, total: u64) {
    let view = tl_view(total, *zoom, *scroll);
    let f = frame as f32;
    if f < view.start || f > view.start + view.span {
        center_tl(scroll, *zoom, frame, total);
    }
}

fn x_to_frame(x: f32, left: f32, width: f32, view: TimeView, total: u64) -> u64 {
    let last = total.saturating_sub(1);
    if width <= 1.0 || last == 0 {
        return 0;
    }
    let t = ((x - left) / width).clamp(0.0, 1.0);
    (view.start + t * view.span).round().clamp(0.0, last as f32) as u64
}

fn frame_to_x(frame: f32, left: f32, width: f32, view: TimeView) -> f32 {
    left + (frame - view.start) / view.span.max(1.0) * width
}

fn pointer_frame(
    ui: &egui::Ui,
    resp: &egui::Response,
    rect: Rect,
    view: TimeView,
    total: u64,
) -> Option<u64> {
    if !(resp.clicked() || resp.dragged() || resp.is_pointer_button_down_on()) {
        return None;
    }
    let pos = ui.input(|i| i.pointer.latest_pos())?;
    Some(x_to_frame(pos.x, rect.left(), rect.width(), view, total))
}

fn handle_tl_nav(
    ui: &egui::Ui,
    resp: &egui::Response,
    rect: Rect,
    total: u64,
    zoom: &mut f32,
    scroll: &mut f32,
) {
    if !resp.hovered() {
        return;
    }
    let zmax = tl_last(total).max(1.0);
    let zdelta = ui.input(|i| i.zoom_delta());
    let wheel = ui.input(|i| i.smooth_scroll_delta);
    if (zdelta - 1.0).abs() > 0.001 {
        let t = ui
            .input(|i| i.pointer.hover_pos())
            .map(|p| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
            .unwrap_or(0.5);
        let view = tl_view(total, *zoom, *scroll);
        let focus = view.start + t * view.span;
        *zoom = (*zoom * zdelta).clamp(1.0, zmax);
        let max_start = tl_max_start(total, *zoom);
        *scroll = if max_start < 1e-3 {
            0.0
        } else {
            ((focus - t * tl_span(total, *zoom)) / max_start).clamp(0.0, 1.0)
        };
        ui.ctx().input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
    } else if wheel != Vec2::ZERO {
        let max_start = tl_max_start(total, *zoom);
        if max_start > 1e-3 {
            let delta = (wheel.x + wheel.y) / rect.width() * tl_span(total, *zoom);
            *scroll = (*scroll - delta / max_start).clamp(0.0, 1.0);
            ui.ctx().input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
        }
    }
}

fn tl_edge_pan(
    ui: &egui::Ui,
    resp: &egui::Response,
    rect: Rect,
    total: u64,
    zoom: f32,
    scroll: &mut f32,
) {
    let dragging = resp.dragged() || resp.is_pointer_button_down_on();
    if !resp.hovered() && !dragging {
        return;
    }
    let max_start = tl_max_start(total, zoom);
    if max_start < 1e-3 {
        return;
    }
    let Some(x) = ui.input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()).map(|p| p.x)) else {
        return;
    };
    let rate = if x >= rect.right() - 6.0 {
        1.15
    } else if x >= rect.right() - 36.0 {
        0.28
    } else if x <= rect.left() + 6.0 {
        -1.15
    } else if x <= rect.left() + 36.0 {
        -0.28
    } else {
        return;
    };
    let dt = ui.input(|i| i.stable_dt).min(0.05);
    *scroll = (*scroll + rate * tl_span(total, zoom) * dt / max_start).clamp(0.0, 1.0);
    ui.ctx().request_repaint();
}

fn classify_range_drag(
    frame: u64,
    pos_x: Option<f32>,
    rect: Rect,
    range: Option<(u64, u64)>,
    shift: bool,
    view: TimeView,
) -> Option<RangeDrag> {
    if shift {
        return Some(RangeDrag::Create { origin: frame });
    }
    let x = pos_x?;
    if let Some((a, b)) = range {
        let xa = frame_to_x(a as f32, rect.left(), rect.width(), view);
        let xb = frame_to_x(b as f32, rect.left(), rect.width(), view);
        if (x - xa).abs() <= 8.0 {
            return Some(RangeDrag::In);
        }
        if (x - xb).abs() <= 8.0 {
            return Some(RangeDrag::Out);
        }
    }
    None
}

fn paint_loop_range(painter: &egui::Painter, rect: Rect, a: u64, b: u64, view: TimeView) {
    let x0 = frame_to_x(a as f32, rect.left(), rect.width(), view);
    let x1 = frame_to_x(b as f32, rect.left(), rect.width(), view);
    let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(x0, rect.top()),
            Pos2::new(x1, rect.bottom()),
        ),
        0.0,
        Color32::from_rgba_unmultiplied(32, 170, 168, 60),
    );
    for x in [x0, x1] {
        painter.rect_filled(
            Rect::from_center_size(Pos2::new(x, rect.center().y), egui::vec2(4.0, rect.height() - 2.0)),
            1.0,
            Color32::from_rgb(210, 240, 238),
        );
    }
}

fn drag_bar_height(ui: &mut egui::Ui, h: &mut f32, min: f32, max: f32, tip: &str) {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 5.0),
        Sense::click_and_drag(),
    );
    let resp = resp.on_hover_text(tip);
    let hot = resp.hovered() || resp.dragged();
    if hot {
        ui.painter().rect_filled(
            rect,
            0.0,
            ui.visuals().widgets.hovered.bg_fill,
        );
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
    }
    if resp.dragged() {
        *h = (*h - resp.drag_delta().y).clamp(min, max);
        ui.ctx().request_repaint();
    }
}

fn paint_waveform(
    ui: &mut egui::Ui,
    peaks: &[(f32, f32)],
    frame: u64,
    total: u64,
    range: Option<(u64, u64)>,
    height: f32,
    zoom: &mut f32,
    scroll: &mut f32,
) -> (Rect, Option<u64>) {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    handle_tl_nav(ui, &resp, rect, total, zoom, scroll);
    tl_edge_pan(ui, &resp, rect, total, *zoom, scroll);
    let view = tl_view(total, *zoom, *scroll);
    let resp = resp.on_hover_cursor(CursorIcon::ResizeHorizontal);
    let painter = ui.painter_at(rect);
    let bg = ui.visuals().extreme_bg_color;
    let playhead = ui.visuals().strong_text_color();
    painter.rect_filled(rect, 4.0, bg);
    if let Some((a, b)) = range {
        paint_loop_range(&painter, rect, a, b, view);
    }
    if !peaks.is_empty() && rect.width() > 1.0 {
        let mid = rect.center().y;
        let amp = rect.height() * 0.45;
        let n = peaks.len() as f32;
        let last = tl_last(total);
        let stroke = Stroke::new(1.0_f32, Color32::from_rgb(32, 170, 168));
        for (i, (lo, hi)) in peaks.iter().enumerate() {
            let fr = (i as f32 + 0.5) / n * last;
            let x = frame_to_x(fr, rect.left(), rect.width(), view);
            if x < rect.left() - 1.0 || x > rect.right() + 1.0 {
                continue;
            }
            painter.line_segment(
                [Pos2::new(x, mid - *hi * amp), Pos2::new(x, mid - *lo * amp)],
                stroke,
            );
        }
    }
    paint_playhead(&painter, rect, frame, view, playhead);
    (rect, pointer_frame(ui, &resp, rect, view, total))
}

fn loop_step(current: u64, delta: i64, a: u64, b: u64) -> u64 {
    if delta == 0 || current < a || current > b {
        return current.saturating_add_signed(delta);
    }
    if delta > 0 {
        let d = delta as u64;
        if current == b {
            return a.saturating_add(d).min(b);
        }
        current.saturating_add(d).min(b)
    } else {
        let d = delta.unsigned_abs();
        if current == a {
            return b.saturating_sub(d).max(a);
        }
        current.saturating_sub(d).max(a)
    }
}

fn nice_sec_step(px_per_sec: f32) -> u64 {
    [1u64, 2, 5, 10, 15, 30, 60, 120, 300, 600]
        .into_iter()
        .find(|&s| s as f32 * px_per_sec >= 44.0)
        .unwrap_or(600)
}

fn nice_frame_step(px_per_frame: f32) -> u64 {
    [1u64, 2, 5, 10, 15, 20, 30]
        .into_iter()
        .find(|&s| s as f32 * px_per_frame >= 36.0)
        .unwrap_or(30)
}

fn label_sec_frame(frame: u64, fps: f64) -> String {
    let fps_i = fps.round().max(1.0) as u64;
    format!("{:02}:{:02}f", frame / fps_i, frame % fps_i)
}

fn paint_framebar(
    ui: &mut egui::Ui,
    frame: u64,
    total: u64,
    fps: f64,
    range: Option<(u64, u64)>,
    height: f32,
    zoom: &mut f32,
    scroll: &mut f32,
) -> (Rect, Option<u64>) {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    handle_tl_nav(ui, &resp, rect, total, zoom, scroll);
    tl_edge_pan(ui, &resp, rect, total, *zoom, scroll);
    let view = tl_view(total, *zoom, *scroll);
    let resp = resp.on_hover_cursor(CursorIcon::ResizeHorizontal);
    let painter = ui.painter_at(rect);
    let bg = ui.visuals().extreme_bg_color;
    let tick_major = ui.visuals().strong_text_color();
    let tick_minor = ui.visuals().weak_text_color();
    let playhead = ui.visuals().strong_text_color();
    painter.rect_filled(rect, 3.0, bg);
    if let Some((a, b)) = range {
        paint_loop_range(&painter, rect, a, b, view);
    }
    let px = frame_to_x(frame as f32, rect.left(), rect.width(), view);
    painter.rect_filled(
        Rect::from_min_max(rect.left_top(), Pos2::new(px, rect.bottom())),
        3.0,
        Color32::from_rgba_unmultiplied(32, 170, 168, 50),
    );
    if rect.width() > 1.0 {
        let fps = fps.max(1.0);
        let fps_i = fps.round().max(1.0) as u64;
        let px_per_frame = rect.width() / view.span.max(1.0);
        let px_per_sec = px_per_frame * fps as f32;
        let visible_s = view.span / fps as f32;
        let frame_mode = visible_s <= 3.0;
        let half_s = visible_s > 3.0 && visible_s <= 8.0;
        let (major, minor) = if frame_mode {
            (nice_frame_step(px_per_frame), 1u64)
        } else if half_s {
            let half = (fps_i / 2).max(1);
            (half, half)
        } else {
            let sec = nice_sec_step(px_per_sec);
            (sec * fps_i, fps_i)
        };
        let last = total.saturating_sub(1);
        let vis0 = view.start.max(0.0) as u64;
        let vis1 = (view.start + view.span).ceil() as u64;
        let mut f = (vis0 / minor) * minor;
        while f <= vis1.min(last) {
            let x = frame_to_x(f as f32, rect.left(), rect.width(), view);
            if x >= rect.left() - 2.0 && x <= rect.right() + 2.0 {
                let is_major = f % major == 0;
                let h = if is_major { 10.0 } else { 5.0 };
                painter.line_segment(
                    [
                        Pos2::new(x, rect.bottom() - 1.0),
                        Pos2::new(x, rect.bottom() - 1.0 - h),
                    ],
                    Stroke::new(1.0_f32, if is_major { tick_major } else { tick_minor }),
                );
                if is_major {
                    let text = if frame_mode {
                        label_sec_frame(f, fps)
                    } else if half_s && f % fps_i != 0 {
                        format!("{:02}.5s", f / fps_i)
                    } else {
                        format!("{:02}s", f / fps_i)
                    };
                    painter.text(
                        Pos2::new(x, rect.top() + 1.0),
                        Align2::CENTER_TOP,
                        text,
                        FontId::monospace(10.0_f32),
                        tick_major,
                    );
                }
            }
            let next = f.saturating_add(minor);
            if next <= f {
                break;
            }
            f = next;
        }
    }
    paint_playhead(&painter, rect, frame, view, playhead);
    (rect, pointer_frame(ui, &resp, rect, view, total))
}

fn paint_playhead(painter: &egui::Painter, rect: Rect, frame: u64, view: TimeView, color: Color32) {
    let px = frame_to_x(frame as f32, rect.left(), rect.width(), view);
    painter.line_segment(
        [Pos2::new(px, rect.top() + 1.0), Pos2::new(px, rect.bottom() - 1.0)],
        Stroke::new(1.5_f32, color),
    );
}

#[cfg(test)]
mod url_tests {
    use super::url_ext;

    #[test]
    fn picks_known_or_mp4() {
        assert_eq!(url_ext("https://x.com/a.mkv?token=1"), "mkv");
        assert_eq!(url_ext("https://x.com/a.MOV"), "mov");
        assert_eq!(url_ext("https://x.com/noext"), "mp4");
    }
}

#[cfg(test)]
mod loop_step_tests {
    use super::loop_step;

    #[test]
    fn plus_ten_hits_out_then_wraps_from_in() {
        let mut f = 15u64;
        let seq: Vec<u64> = (0..4)
            .map(|_| {
                f = loop_step(f, 10, 15, 33);
                f
            })
            .collect();
        assert_eq!(seq, [25, 33, 25, 33]);
    }

    #[test]
    fn plus_ten_tiny_range_clamps_at_out() {
        let mut f = 15u64;
        let seq: Vec<u64> = (0..4)
            .map(|_| {
                f = loop_step(f, 10, 15, 23);
                f
            })
            .collect();
        assert_eq!(seq, [23, 23, 23, 23]);
    }
}

#[cfg(test)]
mod scrub_tests {
    use super::{x_to_frame, TimeView};

    #[test]
    fn maps_edges_and_mid() {
        let v = TimeView {
            start: 0.0,
            span: 10.0,
        };
        assert_eq!(x_to_frame(0.0, 0.0, 100.0, v, 11), 0);
        assert_eq!(x_to_frame(100.0, 0.0, 100.0, v, 11), 10);
        assert_eq!(x_to_frame(50.0, 0.0, 100.0, v, 11), 5);
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::{remap_pan, view_scale, ZoomMode, ZOOM_MAX};
    use eframe::egui::Vec2;

    #[test]
    fn fit_and_clamp() {
        assert!((view_scale(200.0, 100.0, 100.0, 100.0, ZoomMode::FitWidth, 1.0) - 2.0).abs() < 1e-5);
        assert!((view_scale(200.0, 100.0, 100.0, 100.0, ZoomMode::FitHeight, 1.0) - 1.0).abs() < 1e-5);
        assert!((view_scale(200.0, 100.0, 100.0, 100.0, ZoomMode::Contain, 1.0) - 1.0).abs() < 1e-5);
        assert_eq!(
            view_scale(8000.0, 8000.0, 100.0, 100.0, ZoomMode::FitWidth, 1.0),
            ZOOM_MAX
        );
        assert!((view_scale(200.0, 100.0, 100.0, 100.0, ZoomMode::Manual, 4.0) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn zoom_past_fit_stays_centered() {
        let avail = Vec2::splat(200.0);
        let old = Vec2::splat(100.0);
        let new = Vec2::splat(400.0);
        let pan = remap_pan(Vec2::ZERO, old, new, avail, avail * 0.5);
        assert!((pan - Vec2::splat(100.0)).length() < 1e-3);
    }
}
