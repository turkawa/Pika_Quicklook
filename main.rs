use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use evdev::Key as EvKey;
use uinput::event::keyboard::Key as UiKey;
use zbus::blocking::Connection;
use zbus::names::{BusName, InterfaceName};
use zbus::zvariant::ObjectPath;

#[derive(Serialize, Deserialize)]
struct PikaConfig {
    keyboard_name: String,
    trigger_ms: u64,
    typing_buffer_ms: u64,
    max_file_size_mb: u64,
    max_preview_width: f32,
    max_preview_height: f32,
}

impl Default for PikaConfig {
    fn default() -> Self {
        Self {
            keyboard_name: "K350".to_string(),
            trigger_ms: 200,
            typing_buffer_ms: 500,
            max_file_size_mb: 50,
            max_preview_width: 1200.0,
            max_preview_height: 800.0,
        }
    }
}

impl PikaConfig {
    fn load() -> Self {
        let config_path = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".config/pika-ql/config.toml");

        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str(&content) {
                return config;
            }
        }

        let _ = fs::create_dir_all(config_path.parent().unwrap());
        let default_config = Self::default();
        let _ = fs::write(&config_path, toml::to_string(&default_config).unwrap());
        default_config
    }
}

fn main() {
    let config = PikaConfig::load();
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let _ = run_ui(PathBuf::from(&args[1]), config);
        return;
    }
    start_daemon(config);
}

fn start_daemon(config: PikaConfig) {
    let mut device = None;
    for (_, d) in evdev::enumerate() {
        if d.name().map(|n| n.contains(&config.keyboard_name)).unwrap_or(false) {
            device = Some(d);
            break;
        }
    }
    let mut device = device.expect("Keyboard not found. Check config.toml");

    let mut vk = uinput::default().expect("uinput fail")
    .name("Pika-QL Virtual").expect("name fail")
    .event(uinput::event::Keyboard::All).expect("keys fail")
    .create().expect("create fail");

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            if let Ok(events) = device.fetch_events() {
                for event in events {
                    if let evdev::InputEventKind::Key(key) = event.kind() {
                        let _ = tx.send((key, event.value()));
                    }
                }
            }
        }
    });

    let mut space_down_since: Option<Instant> = None;
    let mut last_typing_detected = Instant::now() - Duration::from_secs(1);
    let mut active_preview: Option<Child> = None;
    let mut has_triggered = false;

    loop {
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok((key, value)) => {
                let code = key.code();
                let pressed = value == 1;
                if pressed && ((code >= 2 && code <= 11) || (code >= 16 && code <= 50) || code == 14 || code == 60) {
                    last_typing_detected = Instant::now();
                }
                if key == EvKey::KEY_SPACE {
                    if pressed {
                        space_down_since = Some(Instant::now());
                        has_triggered = false;
                    } else if value == 0 {
                        if let Some(mut child) = active_preview.take() { let _ = child.kill(); }
                        space_down_since = None;
                    }
                }
            }
            _ => {}
        }

        if let Some(start_time) = space_down_since {
            if !has_triggered && start_time.elapsed() > Duration::from_millis(config.trigger_ms) {
                if last_typing_detected.elapsed() > Duration::from_millis(config.typing_buffer_ms) {
                    if is_dolphin_focused() {
                        if let Some(path) = trigger_hold_sequence(&mut vk) {
                            has_triggered = true;
                            active_preview = Command::new(std::env::current_exe().unwrap())
                            .arg(path).spawn().ok();
                        }
                    }
                }
            }
        }
    }
}

fn trigger_hold_sequence(vk: &mut uinput::Device) -> Option<String> {
    let _ = vk.click(&UiKey::Space);
    let _ = vk.synchronize();
    std::thread::sleep(Duration::from_millis(30));
    let _ = vk.press(&UiKey::LeftControl);
    let _ = vk.press(&UiKey::LeftAlt);
    let _ = vk.click(&UiKey::C);
    let _ = vk.release(&UiKey::LeftAlt);
    let _ = vk.release(&UiKey::LeftControl);
    let _ = vk.synchronize();
    std::thread::sleep(Duration::from_millis(100));
    let out = Command::new("wl-paste").output().ok()?;
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let clean = raw.replace("file://", "").replace("%20", " ");
    if !clean.is_empty() && Path::new(&clean).exists() { Some(clean) } else { None }
}

fn is_dolphin_focused() -> bool {
    let conn = match Connection::session() { Ok(c) => c, Err(_) => return false };
    let pgrep = Command::new("pgrep").arg("dolphin").output().ok();
    let pids = match pgrep { Some(o) => String::from_utf8_lossy(&o.stdout).to_string(), None => return false };
    for pid in pids.lines() {
        let bus_str = format!("org.kde.dolphin-{}", pid.trim());
        let bus_name = match BusName::try_from(bus_str) { Ok(b) => b, Err(_) => continue };
        for i in 1..=3 {
            let path_str = format!("/dolphin/Dolphin_{}", i);
            let obj_path = match ObjectPath::try_from(path_str) { Ok(p) => p, Err(_) => continue };
            let interface = InterfaceName::from_static_str("org.kde.dolphin.MainWindow").unwrap();
            let res = conn.call_method(Some(bus_name.clone()), obj_path, Some(interface), "isActiveWindow", &());
            if let Ok(msg) = res { if let Ok(true) = msg.body().deserialize::<bool>() { return true; } }
        }
    }
    false
}

fn run_ui(path: PathBuf, config: PikaConfig) -> eframe::Result<()> {
    let is_dir = path.is_dir();
    let file_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let is_too_large = file_size > config.max_file_size_mb * 1024 * 1024;
    let is_image = !is_dir && !is_too_large && mime_guess::from_path(&path).first_or_octet_stream().type_() == "image";
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("PikaQuickView").to_string();
    let mut extension = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let is_font = extension == "ttf" || extension == "otf";

    // Default window size for non-images
    let (mut w, mut h) = (config.max_preview_width, config.max_preview_height);

    if is_image {
        if let Ok((iw, ih)) = image::image_dimensions(&path) {
            let (iw, ih) = (iw as f32, ih as f32);
            // Proportional scaling math (Fit within max box)
            let ratio_w = config.max_preview_width / iw;
            let ratio_h = config.max_preview_height / ih;
            let scale = ratio_w.min(ratio_h);
            w = iw * scale;
            h = ih * scale;
        }
    }

    let window_title = filename.clone();
    eframe::run_native(&window_title,
                       eframe::NativeOptions {
                           viewport: egui::ViewportBuilder::default()
                           .with_decorations(false)
                           .with_always_on_top()
                           .with_inner_size([w, h]),
                       ..Default::default()
                       },
                       Box::new(move |cc| {
                           egui_extras::install_image_loaders(&cc.egui_ctx);
                           if extension == "desktop" { extension = "ini".to_string(); }
                           if extension == "md" { extension = "markdown".to_string(); }
                           if filename.contains("bashrc") || filename.contains("zshrc") || extension == "sh" { extension = "sh".to_string(); }

                           let mut font_data = None;
                           let content = if is_too_large {
                               format!("File too large ({:.1} MB)", file_size as f32 / 1_048_576.0)
                           } else if is_font {
                               font_data = fs::read(&path).ok();
                               String::new()
                           } else if is_dir {
                               fs::read_dir(&path).map(|rd| {
                                   rd.filter_map(|e| e.ok()).map(|e| {
                                       let prefix = if e.path().is_dir() { "📁" } else { "📄" };
                                       format!("{} {}", prefix, e.file_name().to_string_lossy())
                                   }).collect::<Vec<_>>().join("\n")
                               }).unwrap_or_default()
                           } else if !is_image {
                               match fs::read_to_string(&path) {
                                   Ok(s) => s.chars().take(3500).collect(),
                                Err(_) => {
                                    if let Ok(mut f) = fs::File::open(&path) {
                                        use std::io::Read;
                                        let mut buffer = [0u8; 512];
                                        let n = f.read(&mut buffer).unwrap_or(0);
                                        let mut hex = String::from("--- BINARY HEX VIEW ---\n\n");
                                        for chunk in buffer[..n].chunks(16) {
                                            for b in chunk { hex.push_str(&format!("{:02x} ", b)); }
                                            hex.push('\n');
                                        }
                                        hex
                                    } else { "Error reading file".to_string() }
                                }
                               }
                           } else { String::new() };

                           Box::new(QuickView { path, is_image, is_font, content, extension, filename, font_data })
                       })
    )
}

struct QuickView {
    path: PathBuf, is_image: bool, is_font: bool,
    content: String, extension: String, filename: String,
    font_data: Option<Vec<u8>>
}

impl eframe::App for QuickView {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if self.is_font {
            if let Some(data) = self.font_data.take() {
                let mut fonts = egui::FontDefinitions::default();
                fonts.font_data.insert("preview".to_owned(), egui::FontData::from_owned(data));
                fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "preview".to_owned());
                fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().insert(0, "preview".to_owned());
                ctx.set_fonts(fonts);
            }
        }

        let frame = egui::Frame::none().fill(egui::Color32::from_rgb(25, 25, 25)).inner_margin(0.0);

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if self.is_image {
                ui.add(egui::Image::new(format!("file://{}", self.path.to_str().unwrap())).shrink_to_fit());
            } else if self.is_font {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new(&self.filename).size(20.0).color(egui::Color32::GRAY));
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("The quick brown fox jumps over the lazy dog").size(24.0));
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("ABCDEFGHIJKLMNOPQRSTUVWXYZ").size(32.0));
                        ui.label(egui::RichText::new("abcdefghijklmnopqrstuvwxyz").size(32.0));
                        ui.label(egui::RichText::new("1234567890 !@#$%^&*()").size(32.0));
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("Large Specimen Size").size(64.0));
                    });
                });
            } else {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(&self.filename).color(egui::Color32::from_rgb(150, 150, 150)).size(12.0).strong());
                ui.add_space(5.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx());
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut layout_job = egui_extras::syntax_highlighting::highlight(ui.ctx(), &theme, string, &self.extension);
                        layout_job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(layout_job))
                    };
                    ui.add(egui::TextEdit::multiline(&mut self.content)
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .lock_focus(true)
                    .desired_width(f32::INFINITY)
                    .layouter(&mut layouter));
                });
            }
        });
    }
}
