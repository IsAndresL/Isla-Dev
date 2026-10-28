use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::actions::{self, GitInfo, WorkMsg, WM_WORKER};
use crate::config::{self, hex, Config, Launcher, Library, Project, Template, APP_AUTHOR, APP_VERSION};
use crate::gfx::Gfx;

pub const WIN_W: f32 = 590.0;
pub const WIN_H: f32 = 760.0;
const TOP: f32 = 0.0;
pub const WM_TRAY: u32 = WM_APP + 2;
const WM_MOUSELEAVE: u32 = 0x02A3;
const TIMER_ANIM: usize = 1;
const TIMER_POLL: usize = 2;
const HOTKEY_ID: i32 = 1;
const MAX_ROWS: usize = 6;

// Paleta Black & White minimalista (Apple / Vercel style) + indicadores semánticos
const WHITE: u32 = 0xFFFFFF;
const GRAY: u32 = 0xA1A1AA;
const DIM: u32 = 0x52525B;
const ACCENT: u32 = 0xFFFFFF;
const BODY: u32 = 0x000000;
const CARD: u32 = 0x121214;
const CARD_HOV: u32 = 0x1C1C20;
const BORDER: u32 = 0x27272A;
const RED: u32 = 0xEF4444;
const GREEN: u32 = 0x22C55E;
const AMBER: u32 = 0xF59E0B;

// Glifos Segoe Fluent Icons / MDL2
const G_CHEV: &str = "\u{E76C}";
const G_BACK: &str = "\u{E72B}";
const G_ADD: &str = "\u{E710}";
const G_GEAR: &str = "\u{E713}";
const G_CLOSE: &str = "\u{E711}";
const G_FOLDER: &str = "\u{E8B7}";
const G_CHECK: &str = "\u{E73E}";
const G_POWER: &str = "\u{E7E8}";
const G_EDIT: &str = "\u{E70F}";
const G_TERM: &str = "\u{E756}";
const G_ERR: &str = "\u{E783}";
const G_EXIT: &str = "\u{E8BB}";
const G_SEARCH: &str = "\u{E721}";
const G_PLAY: &str = "\u{E768}";
const G_STOP: &str = "\u{E71A}";
const G_PIN: &str = "\u{E718}";
const G_TRASH: &str = "\u{E74D}";
const G_UPLOAD: &str = "\u{E898}";
const G_DOWNLOAD: &str = "\u{E896}";
const G_GLOBE: &str = "\u{E774}";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    Pill,
    Close,
    Settings,
    Back,
    SearchBox,
    ClearSearch,
    PortBadge(u16),
    PortTerm(u16),
    KillPort(u16),
    Project(usize),
    ProjServer(usize),
    ProjServerTerm(usize),
    ProjPort(usize, u16),
    ProjGit(usize),
    ProjTerm(usize),
    ProjExplorer(usize),
    ProjPin(usize),
    NewProject,
    // Wizard
    Template(usize),
    Library(usize),
    Launcher(usize),
    WizToggleGithub,
    WizOpenLibEditor,
    WizOpenLauncherEditor,
    Next,
    Input,
    // Settings
    ToggleVisibility,
    ToggleFullscreenHide,
    CycleMonitor,
    Startup,
    DefaultLauncher,
    BaseDir,
    OpenCustomEditor,
    AddExisting,
    EditConfig,
    Quit,
    // Git Modal
    GitCommitInput,
    GitTogglePublic,
    GitDoCommitPush,
    GitDoPull,
    GitOpenRepoWeb,
    GitOpenLogTerm,
    // Custom Visual Editor
    EdTab(usize),
    EdField(usize),
    EdPickExe,
    EdPrevTpl,
    EdNextTpl,
    EdCycleStack,
    EdSave,
    EdDeleteItem(usize),
    EdToggleLibDefault(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Compact,
    Expanded,
    Settings,
    Wizard,
    GitModal,
    CustomEditor,
    Status,
}

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Name,
    Libraries,
    Launcher,
}

#[derive(Clone, Copy, PartialEq)]
enum SKind {
    Opening,
    Done,
    Creating,
    Error,
}

#[derive(Clone, Copy)]
struct Spring {
    x: f32,
    v: f32,
    t: f32,
}

impl Spring {
    fn new(x: f32) -> Self {
        Self { x, v: 0.0, t: x }
    }
    fn step(&mut self, dt: f32, k: f32, d: f32, eps: f32) -> bool {
        let mut rem = dt;
        while rem > 0.0 {
            let h = rem.min(1.0 / 240.0);
            let a = -k * (self.x - self.t) - d * self.v;
            self.v += a * h;
            self.x += self.v * h;
            rem -= h;
        }
        if (self.x - self.t).abs() < eps && self.v.abs() < eps * 10.0 {
            self.x = self.t;
            self.v = 0.0;
            false
        } else {
            true
        }
    }
}

struct Pending {
    name: String,
    stack: String,
    color: String,
    launcher: String,
}

#[derive(Clone, Debug)]
struct RunningServer {
    pid: u32,
    port: Option<u16>,
    log_path: PathBuf,
}

pub struct App {
    hwnd: HWND,
    gfx: Gfx,
    cfg: Config,
    cfg_mtime: Option<SystemTime>,
    view: View,
    resume_view: Option<View>,
    hover: Option<Hit>,
    hits: RefCell<Vec<(f32, f32, f32, f32, Hit)>>,
    sw: Spring,
    sh: Spring,
    sr: Spring,
    sa: Spring,
    last: Instant,
    timer: bool,
    tracking: bool,
    spin: f32,
    glow_anim: f32,
    scroll: usize,
    pos: (i32, i32),
    visible: bool,
    auto_hidden_fs: bool,
    startup: bool,
    // buscador instantáneo ("Spotlight")
    search: String,
    sel_idx: usize,
    // estado de Git y servidores en tiempo real
    git_states: HashMap<String, GitInfo>,
    servers: HashMap<String, RunningServer>,
    active_ports: Vec<u16>,
    // asistente de creación
    step: Step,
    wiz_name: String,
    wiz_tpl: usize,
    wiz_tpl_scroll: usize,
    wiz_libs: Vec<bool>,
    wiz_lib_scroll: usize,
    wiz_launcher: usize,
    wiz_github: bool,
    wiz_err: Option<String>,
    // modal visual de Git & GitHub
    git_proj_idx: usize,
    git_commit_msg: String,
    git_public: bool,
    // editor visual de Librerías, Programas y Frameworks
    ed_tab: usize, // 0 = Librerías, 1 = Programas, 2 = Frameworks
    ed_focus: usize,
    ed_tpl_idx: usize,
    ed_f1: String,
    ed_f2: String,
    ed_f3: String,
    ed_scroll: usize,
    ed_msg: Option<(String, bool)>,
    // estado / notificaciones
    skind: SKind,
    s_title: String,
    s_sub: String,
    s_color: u32,
    s_initial: String,
    s_next: String,
    s_deadline: Option<Instant>,
    s_log_path: Option<PathBuf>,
    s_proj_path: Option<PathBuf>,
    pending: Option<Pending>,
    tx: Sender<WorkMsg>,
    rx: Receiver<WorkMsg>,
    tray_icon: HICON,
}

fn initial(s: &str) -> String {
    s.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or("·".into())
}

const STACK_PRESETS: &[(&str, &str)] = &[
    ("React + Vite", "#3B82F6"),
    ("Next.js", "#71717A"),
    ("Astro", "#FF5D01"),
    ("Vue + Vite", "#42B883"),
    ("Svelte", "#FF3E00"),
    ("Angular", "#DD0031"),
    ("Node.js", "#22C55E"),
    ("Python", "#3776AB"),
    ("Rust", "#CE422B"),
    ("TypeScript", "#3178C6"),
];

impl App {
    pub fn new(hwnd: HWND) -> Result<Self> {
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
        let gfx = Gfx::new(scale, WIN_W, WIN_H)?;
        let (cfg, err) = config::load();
        let (tx, rx) = channel();
        let startup = actions::startup_enabled();
        let mut a = App {
            hwnd,
            gfx,
            cfg,
            cfg_mtime: config::mtime(),
            view: View::Compact,
            resume_view: None,
            hover: None,
            hits: RefCell::new(Vec::new()),
            sw: Spring::new(48.0),
            sh: Spring::new(10.0),
            sr: Spring::new(5.0),
            sa: Spring::new(0.0),
            last: Instant::now(),
            timer: false,
            tracking: false,
            spin: 0.0,
            glow_anim: 0.0,
            scroll: 0,
            pos: (0, 0),
            visible: true,
            auto_hidden_fs: false,
            startup,
            search: String::new(),
            sel_idx: 0,
            git_states: HashMap::new(),
            servers: HashMap::new(),
            active_ports: Vec::new(),
            step: Step::Name,
            wiz_name: String::new(),
            wiz_tpl: 0,
            wiz_tpl_scroll: 0,
            wiz_libs: Vec::new(),
            wiz_lib_scroll: 0,
            wiz_launcher: 0,
            wiz_github: false,
            wiz_err: None,
            git_proj_idx: 0,
            git_commit_msg: String::from("feat: actualización"),
            git_public: false,
            ed_tab: 0,
            ed_focus: 0,
            ed_tpl_idx: 0,
            ed_f1: String::new(),
            ed_f2: String::new(),
            ed_f3: String::from("React + Vite"),
            ed_scroll: 0,
            ed_msg: None,
            skind: SKind::Done,
            s_title: String::new(),
            s_sub: String::new(),
            s_color: ACCENT,
            s_initial: String::new(),
            s_next: String::new(),
            s_deadline: None,
            s_log_path: None,
            s_proj_path: None,
            pending: None,
            tx,
            rx,
            tray_icon: HICON::default(),
        };
        a.reset_libs();
        a.wiz_launcher = a.cfg.launcher_index(&a.cfg.default_launcher);
        a.sa.t = 1.0;
        if let Some(e) = err {
            a.error("Revisa tu config.toml", &e, None);
        }
        Ok(a)
    }

    pub fn init(&mut self) {
        self.scan_and_sync();
        self.refresh_live_state();
        self.reposition();
        self.register_hotkey();
        self.add_tray();
        self.set_visible(true);
        unsafe {
            DragAcceptFiles(self.hwnd, true);
            SetTimer(self.hwnd, TIMER_POLL, 3500, None);
        }
        self.kick();
    }

    fn scan_and_sync(&mut self) {
        let scanned = actions::scan_base_dir(Path::new(&self.cfg.base_dir));
        if self.cfg.sync_scanned_projects(scanned) {
            self.save();
        }
        for p in &self.cfg.projects {
            if !self.git_states.contains_key(&p.path) {
                if let Some(branch) = actions::read_git_branch_fast(Path::new(&p.path)) {
                    let remote_url = actions::read_git_remote(Path::new(&p.path));
                    self.git_states.insert(
                        p.path.clone(),
                        GitInfo {
                            branch,
                            dirty: false,
                            changed_count: 0,
                            remote_url,
                        },
                    );
                }
            }
        }
    }

    fn refresh_live_state(&self) {
        let paths: Vec<String> = self.cfg.projects.iter().map(|p| p.path.clone()).collect();
        let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
        let hw = self.hwnd.0 as isize;
        actions::inspect_git_all(paths, self.tx.clone(), hw);
        actions::scan_open_ports(extra_ports, self.tx.clone(), hw);
    }

    fn poll_periodic(&mut self) {
        // 1. Verificar auto-ocultamiento en pantalla completa y soporte multi-monitor
        let mon_rc = actions::target_monitor_rect(&self.cfg.monitor_mode);
        let new_x = mon_rc.left + ((mon_rc.right - mon_rc.left) - self.gfx.wpx) / 2;
        let new_y = mon_rc.top;
        if (new_x, new_y) != self.pos {
            self.pos = (new_x, new_y);
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    self.pos.0,
                    self.pos.1,
                    self.gfx.wpx,
                    self.gfx.hpx,
                    SWP_NOACTIVATE,
                );
            }
            self.kick();
        }

        if self.cfg.hide_on_fullscreen && self.visible {
            let fs_now = actions::is_other_fullscreen(self.hwnd, mon_rc);
            if fs_now && !self.auto_hidden_fs {
                self.auto_hidden_fs = true;
                unsafe {
                    let _ = ShowWindow(self.hwnd, SW_HIDE);
                }
            } else if !fs_now && self.auto_hidden_fs {
                self.auto_hidden_fs = false;
                unsafe {
                    let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                }
                self.kick();
            }
        } else if self.auto_hidden_fs && self.visible {
            self.auto_hidden_fs = false;
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }

        // 2. Escanear puertos activos
        let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
        actions::scan_open_ports(extra_ports, self.tx.clone(), self.hwnd.0 as isize);
    }

    fn active_server_count(&self) -> usize {
        let mut ports = self.active_ports.clone();
        let mut extra = 0usize;
        for srv in self.servers.values() {
            if let Some(p) = srv.port {
                if !ports.contains(&p) {
                    ports.push(p);
                }
            } else {
                extra += 1;
            }
        }
        ports.len() + extra
    }

    fn has_live_server(&self) -> bool {
        self.active_server_count() > 0
    }

    fn primary_live_port(&self) -> Option<u16> {
        self.servers
            .values()
            .find_map(|s| s.port)
            .or_else(|| self.active_ports.first().copied())
    }

    fn waterdrop_dot_point(cx: f32, w: f32, h: f32, u: f32, dot_r: f32) -> (f32, f32) {
        let r_cup = (h * 0.46).clamp(8.0, 15.0);
        let cy = (h - r_cup).max(4.0);
        let t = u.abs().clamp(0.0, 1.0);
        let (x_off, y_raw) = if t <= 0.5 {
            let theta = (t / 0.5) * std::f32::consts::FRAC_PI_2;
            (r_cup * theta.sin(), cy + r_cup * theta.cos() - 0.5)
        } else {
            let s = (t - 0.5) / 0.5;
            let m = 1.0 - s;
            let fillet_x = (w * 0.5 - r_cup) * 0.55;
            let fillet_y = cy * 0.55;
            let p0x = r_cup;
            let p0y = cy;
            let p1x = r_cup;
            let p1y = cy - fillet_y;
            let p2x = w * 0.5 - fillet_x;
            let p2y = 0.0;
            let p3x = w * 0.5;
            let p3y = 0.0;
            let bx = m * m * m * p0x + 3.0 * m * m * s * p1x + 3.0 * m * s * s * p2x + s * s * s * p3x;
            let by = m * m * m * p0y + 3.0 * m * m * s * p1y + 3.0 * m * s * s * p2y + s * s * s * p3y;
            (bx, by)
        };
        let x = if u < 0.0 { cx - x_off } else { cx + x_off };
        let y = y_raw.max(dot_r + 0.4);
        (x, y)
    }

    fn waterdrop_dot_positions(cx: f32, w: f32, h: f32, count: usize, dot_r: f32) -> Vec<(f32, f32)> {
        let us: Vec<f32> = match count {
            0 => Vec::new(),
            1 => vec![0.0],
            2 => vec![-0.28, 0.28],
            3 => vec![-0.72, 0.0, 0.72],
            4 => vec![-0.75, -0.27, 0.27, 0.75],
            5 => vec![-0.80, -0.38, 0.0, 0.38, 0.80],
            n => {
                let cap = n.min(8);
                (0..cap)
                    .map(|i| -0.82 + 1.64 * (i as f32 / (cap - 1) as f32))
                    .collect()
            }
        };
        us.into_iter()
            .map(|u| Self::waterdrop_dot_point(cx, w, h, u, dot_r))
            .collect()
    }

    fn filtered_projects(&self) -> Vec<usize> {
        let q = self.search.trim().to_lowercase();
        if q.is_empty() {
            (0..self.cfg.projects.len()).collect()
        } else {
            self.cfg
                .projects
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    let branch = self
                        .git_states
                        .get(&p.path)
                        .map(|g| g.branch.to_lowercase())
                        .unwrap_or_default();
                    p.name.to_lowercase().contains(&q)
                        || p.stack.to_lowercase().contains(&q)
                        || branch.contains(&q)
                })
                .map(|(i, _)| i)
                .collect()
        }
    }

    // ---------- ventana / sistema ----------

    fn reposition(&mut self) {
        let mon_rc = actions::target_monitor_rect(&self.cfg.monitor_mode);
        self.pos = (
            mon_rc.left + ((mon_rc.right - mon_rc.left) - self.gfx.wpx) / 2,
            mon_rc.top,
        );
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                self.pos.0,
                self.pos.1,
                self.gfx.wpx,
                self.gfx.hpx,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    fn register_hotkey(&self) {
        unsafe {
            let _ = UnregisterHotKey(self.hwnd, HOTKEY_ID);
            if let Some((m, vk)) = actions::parse_hotkey(&self.cfg.hotkey) {
                let _ = RegisterHotKey(self.hwnd, HOTKEY_ID, m, vk);
            }
        }
    }

    fn tray_data(&self) -> NOTIFYICONDATAW {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: self.tray_icon,
            ..Default::default()
        };
        let tip: Vec<u16> = format!("Isla Dev ({})", self.cfg.hotkey)
            .encode_utf16()
            .take(127)
            .collect();
        nid.szTip[..tip.len()].copy_from_slice(&tip);
        nid
    }

    fn add_tray(&mut self) {
        self.tray_icon = self
            .gfx
            .make_app_icon(32)
            .unwrap_or_else(|_| unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() });
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &self.tray_data());
        }
    }

    fn quit(&mut self) {
        for srv in self.servers.values() {
            actions::stop_dev_server(srv.pid);
        }
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_POLL);
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.tray_data());
            let _ = UnregisterHotKey(self.hwnd, HOTKEY_ID);
            PostQuitMessage(0);
        }
    }

    fn focus(&self) {
        unsafe {
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(self.hwnd);
        }
    }

    fn set_visible(&mut self, v: bool) {
        self.visible = v;
        self.auto_hidden_fs = false;
        unsafe {
            let _ = ShowWindow(self.hwnd, if v { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
        if v {
            self.kick();
        }
    }

    fn reload_if_changed(&mut self) {
        let m = config::mtime();
        if m != self.cfg_mtime {
            self.cfg_mtime = m;
            let (c, err) = config::load();
            self.cfg = c;
            self.register_hotkey();
            if let Some(e) = err {
                self.error("Revisa tu config.toml", &e, Some(config::config_path()));
            }
        }
    }

    fn save(&mut self) {
        config::save(&self.cfg);
        self.cfg_mtime = config::mtime();
    }

    // ---------- animación ----------

    fn kick(&mut self) {
        if !self.timer {
            self.last = Instant::now();
            unsafe {
                SetTimer(self.hwnd, TIMER_ANIM, 8, None);
            }
            self.timer = true;
        }
        self.tick();
    }

    fn target(&self) -> (f32, f32, f32) {
        let (w, h, r) = match self.view {
            View::Compact => {
                if self.hov(Hit::Pill) {
                    if self.has_live_server() {
                        (96.0, 24.0, 12.0)
                    } else {
                        (78.0, 24.0, 12.0)
                    }
                } else if self.has_live_server() {
                    (54.0, 10.0, 5.0)
                } else {
                    (48.0, 10.0, 5.0)
                }
            }
            View::Expanded => {
                let filtered_len = self.filtered_projects().len();
                let n = filtered_len.clamp(1, MAX_ROWS) as f32;
                let ports_bar = if self.active_ports.is_empty() { 0.0 } else { 32.0 };
                let search_bar = 44.0;
                (556.0, 64.0 + ports_bar + search_bar + n * 54.0 + 54.0 + 16.0, 26.0)
            }
            View::Settings => (548.0, 64.0 + 9.0 * 48.0 + 38.0, 26.0),
            View::Wizard => {
                let rows = match self.step {
                    Step::Name => self.cfg.templates.len().min(5) as f32 * 46.0 + 118.0,
                    Step::Libraries => self.cur_libs_len().clamp(1, 6) as f32 * 46.0 + 48.0,
                    Step::Launcher => self.cfg.launchers.len().min(6) as f32 * 46.0 + 48.0,
                };
                (548.0, 64.0 + rows + 68.0, 26.0)
            }
            View::GitModal => (548.0, 340.0, 26.0),
            View::CustomEditor => (548.0, 495.0, 26.0),
            View::Status => {
                if self.skind == SKind::Creating || self.skind == SKind::Error {
                    (480.0, 64.0, 28.0)
                } else {
                    (410.0, 58.0, 28.0)
                }
            }
        };
        (w, h.min(WIN_H - 10.0), r)
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.05);
        self.last = now;
        let (tw, th, tr) = self.target();
        self.sw.t = tw;
        self.sh.t = th;
        self.sr.t = tr;
        let mut moving = self.sw.step(dt, 420.0, 30.0, 0.05)
            | self.sh.step(dt, 420.0, 30.0, 0.05)
            | self.sr.step(dt, 420.0, 30.0, 0.05)
            | self.sa.step(dt, 220.0, 30.0, 0.002);

        let is_hov = self.view == View::Compact && self.hov(Hit::Pill);
        if is_hov {
            self.glow_anim = (self.glow_anim + dt * 6.0) % std::f32::consts::TAU;
            moving = true;
        } else if self.glow_anim > 0.0 {
            self.glow_anim = (self.glow_anim - dt * 4.0).max(0.0);
            moving = true;
        }

        if self.view == View::Status {
            moving = true;
            self.spin += dt;
            if let Some(d) = self.s_deadline {
                if now >= d {
                    self.s_deadline = None;
                    match self.skind {
                        SKind::Opening => {
                            self.skind = SKind::Done;
                            self.s_title = std::mem::take(&mut self.s_sub);
                            self.s_sub = self.s_next.clone();
                            self.s_deadline = Some(now + Duration::from_millis(1600));
                            self.fade();
                        }
                        _ => self.set_view(View::Compact),
                    }
                }
            }
        }
        self.render();
        if !moving && self.timer {
            unsafe {
                let _ = KillTimer(self.hwnd, TIMER_ANIM);
            }
            self.timer = false;
        }
    }

    fn fade(&mut self) {
        self.sa.x = 0.0;
        self.sa.v = 0.0;
    }

    /// Cambia la vista activa sin destruir el progreso de creación, búsqueda o formularios.
    fn set_view(&mut self, v: View) {
        if v == self.view {
            return;
        }
        self.view = v;
        self.hover = None;
        self.fade();
        if v == View::Expanded {
            self.refresh_live_state();
        }
        if matches!(
            v,
            View::Expanded | View::Settings | View::Wizard | View::GitModal | View::CustomEditor
        ) {
            self.focus();
        } else if v == View::Compact {
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
        self.kick();
    }

    /// Minimiza la isla a la gota guardando la vista actual para restaurarla intacta al volver a abrirla.
    fn minimize_preserving_state(&mut self) {
        if matches!(
            self.view,
            View::Expanded | View::Settings | View::Wizard | View::GitModal | View::CustomEditor
        ) {
            self.resume_view = Some(self.view);
        }
        self.set_view(View::Compact);
    }

    /// Restaura la última vista en la que estaba trabajando el usuario (o `View::Expanded` por defecto).
    fn reopen_preserved_view(&mut self) {
        let target = self.resume_view.take().unwrap_or(View::Expanded);
        self.set_view(target);
    }

    // ---------- dibujo ----------

    fn hit(&self, x: f32, y: f32, w: f32, h: f32, id: Hit) {
        self.hits.borrow_mut().push((x, y, w, h, id));
    }

    fn hov(&self, id: Hit) -> bool {
        self.hover == Some(id)
    }

    fn is_row_hov(&self, i: usize) -> bool {
        matches!(
            self.hover,
            Some(Hit::Project(k))
                | Some(Hit::ProjServer(k))
                | Some(Hit::ProjServerTerm(k))
                | Some(Hit::ProjPort(k, _))
                | Some(Hit::ProjGit(k))
                | Some(Hit::ProjTerm(k))
                | Some(Hit::ProjExplorer(k))
                | Some(Hit::ProjPin(k))
                if k == i
        )
    }

    fn hit_at(&self, x: f32, y: f32) -> Option<Hit> {
        self.hits
            .borrow()
            .iter()
            .rev()
            .find(|h| x >= h.0 && x < h.0 + h.2 && y >= h.1 && y < h.1 + h.3)
            .map(|h| h.4)
    }

    fn render(&mut self) {
        self.hits.borrow_mut().clear();
        if self.gfx.begin().is_err() {
            return;
        }
        self.draw();
        let (x, y) = self.pos;
        self.gfx.present(self.hwnd, x, y);
    }

    fn draw(&self) {
        let g = &self.gfx;
        let (w, h, r) = (self.sw.x.max(30.0), self.sh.x.max(8.0), self.sr.x.max(3.0));
        let cx = WIN_W / 2.0;
        let y = TOP;
        g.alpha.set(1.0);

        if self.view == View::Compact {
            let is_hov = self.hov(Hit::Pill);
            if let Ok(sp) = g.waterdrop_path(cx, w + 4.0, h + 3.0) {
                g.fill_path(&sp, 0x000000, 0.35);
            }
            if let Ok(sp2) = g.waterdrop_path(cx, w + 8.0, h + 6.0) {
                g.fill_path(&sp2, 0x000000, 0.15);
            }

            if let Ok(notch) = g.waterdrop_path(cx, w, h) {
                g.fill_path(&notch, BODY, 1.0);
            }

            let pulse = if is_hov {
                0.70 + 0.30 * self.glow_anim.sin()
            } else if self.glow_anim > 0.0 {
                self.glow_anim * 0.7
            } else {
                0.0
            };
            g.draw_glow_ring(cx, w, h, pulse);

            if let Ok(curve) = g.waterdrop_curve_path(cx, w, h) {
                let stroke_alpha = if is_hov { 0.85 } else { 0.18 };
                g.stroke_path(&curve, WHITE, stroke_alpha, 1.2);
            }

            let srv_count = self.active_server_count();
            if srv_count > 0 {
                if is_hov && h > 16.0 {
                    let port_txt = if srv_count == 1 {
                        self.primary_live_port()
                            .map(|p| format!(":{p}"))
                            .unwrap_or_else(|| "1 LIVE".into())
                    } else {
                        format!("{srv_count} LIVE")
                    };
                    g.text(&port_txt, &g.f.small_c, cx - 28.0, 1.0, 56.0, h - 4.0, GREEN, 0.95);
                }
                let dot_r = 3.0;
                for (dx, dy) in Self::waterdrop_dot_positions(cx, w, h, srv_count, dot_r) {
                    g.circle(dx, dy, dot_r + 1.8, GREEN, 0.25);
                    g.circle(dx, dy, dot_r + 0.8, 0x000000, 0.75);
                    g.circle(dx, dy, dot_r, GREEN, 1.0);
                }
            }

            self.hit(cx - w * 0.5, 0.0, w, h + 6.0, Hit::Pill);
        } else {
            let x = cx - w / 2.0;
            for i in 0..8 {
                let e = i as f32 * 2.0;
                let alpha = 0.04 * (1.0 - i as f32 / 8.0);
                if let Ok(sp) = g.top_panel_path(x - e, w + e * 2.0, h + e * 1.5, r + e) {
                    g.fill_path(&sp, 0x000000, alpha);
                }
            }
            if let Ok(panel) = g.top_panel_path(x, w, h, r) {
                g.fill_path(&panel, BODY, 0.98);
            }
            if let Ok(panel_curve) = g.top_panel_curve_path(x, w, h, r) {
                g.stroke_path(&panel_curve, WHITE, 0.12, 1.0);
            }

            g.clip(x, y, w, h);
            g.alpha.set(self.sa.x.clamp(0.0, 1.0));
            let (tw, th, _) = self.target();
            let lx = WIN_W / 2.0 - tw / 2.0;
            match self.view {
                View::Compact => {}
                View::Expanded => self.draw_expanded(lx, y, tw),
                View::Settings => self.draw_settings(lx, y, tw),
                View::Wizard => self.draw_wizard(lx, y, tw, th),
                View::GitModal => self.draw_git_modal(lx, y, tw, th),
                View::CustomEditor => self.draw_custom_editor(lx, y, tw, th),
                View::Status => self.draw_status(lx, y, tw, h),
            }
            g.alpha.set(1.0);
            g.unclip();
        }
    }

    fn icon_btn(&self, x: f32, y: f32, s: f32, glyph: &str, id: Hit) {
        let g = &self.gfx;
        let hv = self.hov(id);
        if hv {
            g.circle(x + s / 2.0, y + s / 2.0, s / 2.0, WHITE, 0.1);
        }
        g.text(glyph, &g.f.icon_sm, x, y, s, s, if hv { WHITE } else { GRAY }, 1.0);
        self.hit(x, y, s, s, id);
    }

    fn mini_action_btn(&self, x: f32, y: f32, s: f32, glyph: &str, color: u32, active: bool, id: Hit) {
        let g = &self.gfx;
        let hv = self.hov(id);
        let bg_alpha = if hv { 0.20 } else if active { 0.12 } else { 0.06 };
        g.rrect(x, y, s, s, 7.0, WHITE, bg_alpha);
        if hv || active {
            g.rrect_stroke(x + 0.5, y + 0.5, s - 1.0, s - 1.0, 7.0, color, 0.50, 1.0);
        }
        let fg = if hv || active { color } else { GRAY };
        g.text(glyph, &g.f.icon_sm, x, y, s, s, fg, 1.0);
        self.hit(x, y, s, s, id);
    }

    fn pill_action_btn(&self, x: f32, y: f32, w: f32, h: f32, label: &str, color: u32, active: bool, id: Hit) {
        let g = &self.gfx;
        let hv = self.hov(id);
        let bg_a = if hv { 0.22 } else if active { 0.13 } else { 0.06 };
        g.rrect(x, y, w, h, 7.0, if active || hv { color } else { WHITE }, bg_a);
        g.rrect_stroke(
            x + 0.5,
            y + 0.5,
            w - 1.0,
            h - 1.0,
            7.0,
            if active || hv { color } else { BORDER },
            if active || hv { 0.55 } else { 0.7 },
            1.0,
        );
        let fg = if active || hv { color } else { GRAY };
        g.text(label, &g.f.small_c, x, y, w, h, fg, 1.0);
        self.hit(x, y, w, h, id);
    }

    fn chip(&self, x: f32, y: f32, w: f32, s: &str) {
        let g = &self.gfx;
        g.rrect(x, y, w, 22.0, 6.0, WHITE, 0.07);
        g.text(s, &g.f.small_c, x, y, w, 22.0, GRAY, 1.0);
    }

    fn header(&self, lx: f32, y: f32, tw: f32, title: &str, sub: &str, back: bool) {
        let g = &self.gfx;
        if back {
            self.icon_btn(lx + 16.0, y + 16.0, 32.0, G_BACK, Hit::Back);
        } else {
            g.draw_logo(lx + 16.0, y + 14.0, 36.0, 36.0, 1.0);
        }
        g.text(title, &g.f.title, lx + 62.0, y + 13.0, tw - 160.0, 20.0, WHITE, 1.0);
        g.text(sub, &g.f.sub, lx + 62.0, y + 32.0, tw - 160.0, 18.0, GRAY, 1.0);
        if !back {
            self.icon_btn(lx + tw - 86.0, y + 16.0, 32.0, G_GEAR, Hit::Settings);
        }
        self.icon_btn(lx + tw - 48.0, y + 16.0, 32.0, G_CLOSE, Hit::Close);
        g.line(lx + 16.0, y + 63.5, lx + tw - 16.0, y + 63.5, WHITE, 0.08, 1.0);
    }

    fn count_text(&self) -> String {
        let base = match self.cfg.projects.len() {
            0 => "Sin proyectos aún".into(),
            1 => "1 proyecto".into(),
            n => format!("{n} proyectos"),
        };
        let active_cnt = self.active_server_count();
        if active_cnt > 0 {
            format!(
                "{base} · {active_cnt} servidor{} activo{}",
                if active_cnt == 1 { "" } else { "es" },
                if active_cnt == 1 { "" } else { "s" }
            )
        } else {
            base
        }
    }

    fn row_bg(&self, lx: f32, ry: f32, tw: f32, h: f32, id: Hit, selected: bool) {
        let g = &self.gfx;
        let is_hov = match id {
            Hit::Project(i) => self.is_row_hov(i),
            _ => self.hov(id),
        };
        if selected {
            g.rrect(lx + 8.0, ry + 2.0, tw - 16.0, h - 4.0, 12.0, CARD_HOV, 1.0);
            g.rrect_stroke(lx + 8.5, ry + 2.5, tw - 17.0, h - 5.0, 12.0, WHITE, 0.45, 1.0);
        } else if is_hov {
            g.rrect(lx + 8.0, ry + 2.0, tw - 16.0, h - 4.0, 12.0, CARD_HOV, 1.0);
            g.rrect_stroke(lx + 8.5, ry + 2.5, tw - 17.0, h - 5.0, 12.0, BORDER, 0.85, 1.0);
        } else {
            g.rrect(lx + 8.0, ry + 2.0, tw - 16.0, h - 4.0, 12.0, CARD, 0.6);
            g.rrect_stroke(lx + 8.5, ry + 2.5, tw - 17.0, h - 5.0, 12.0, BORDER, 0.4, 1.0);
        }
        self.hit(lx + 8.0, ry + 2.0, tw - 16.0, h - 4.0, id);
    }

    fn draw_expanded(&self, lx: f32, y: f32, tw: f32) {
        let g = &self.gfx;
        self.header(lx, y, tw, "Mis Proyectos", &self.count_text(), false);
        let mut ry = y + 66.0;

        // 1. Barra de puertos locales activos (abrir en navegador, ir a su terminal o matar proceso)
        if !self.active_ports.is_empty() {
            g.circle(lx + 20.0, ry + 13.0, 3.5, GREEN, 1.0);
            g.text("PUERTOS:", &g.f.small_c, lx + 26.0, ry + 2.0, 62.0, 22.0, GRAY, 1.0);
            let mut px = lx + 92.0;
            for &port in self.active_ports.iter().take(4) {
                let pw = 94.0;
                let hv_open = self.hov(Hit::PortBadge(port));
                let hv_term = self.hov(Hit::PortTerm(port));
                let hv_kill = self.hov(Hit::KillPort(port));
                g.rrect(px, ry + 2.0, pw, 22.0, 6.0, GREEN, if hv_open { 0.22 } else { 0.12 });
                g.rrect_stroke(px + 0.5, ry + 2.5, pw - 1.0, 21.0, 6.0, GREEN, 0.45, 1.0);
                g.text(&format!(":{port}"), &g.f.small_c, px + 4.0, ry + 2.0, pw - 40.0, 22.0, GREEN, 1.0);
                self.hit(px, ry + 2.0, pw - 40.0, 22.0, Hit::PortBadge(port));

                if hv_term {
                    g.rrect(px + pw - 38.0, ry + 4.0, 18.0, 18.0, 4.0, GREEN, 0.32);
                }
                g.text(
                    G_TERM,
                    &g.f.icon_sm,
                    px + pw - 38.0,
                    ry + 2.0,
                    18.0,
                    22.0,
                    if hv_term { WHITE } else { GREEN },
                    1.0,
                );
                self.hit(px + pw - 38.0, ry + 2.0, 19.0, 22.0, Hit::PortTerm(port));

                if hv_kill {
                    g.rrect(px + pw - 19.0, ry + 4.0, 17.0, 18.0, 4.0, RED, 0.35);
                }
                g.text(
                    G_CLOSE,
                    &g.f.icon_sm,
                    px + pw - 19.0,
                    ry + 2.0,
                    17.0,
                    22.0,
                    if hv_kill { RED } else { GRAY },
                    1.0,
                );
                self.hit(px + pw - 19.0, ry + 2.0, 19.0, 22.0, Hit::KillPort(port));
                px += pw + 6.0;
            }
            ry += 30.0;
        }

        // 2. Buscador Instantáneo ("Spotlight" dentro de la Isla)
        let sbx = lx + 10.0;
        let sbw = tw - 20.0;
        let sbh = 36.0;
        let has_q = !self.search.is_empty();
        g.rrect(sbx, ry + 2.0, sbw, sbh, 10.0, WHITE, if has_q { 0.08 } else { 0.04 });
        g.rrect_stroke(
            sbx + 0.5,
            ry + 2.5,
            sbw - 1.0,
            sbh - 1.0,
            10.0,
            if has_q { WHITE } else { BORDER },
            if has_q { 0.45 } else { 0.7 },
            1.0,
        );
        g.text(G_SEARCH, &g.f.icon_sm, sbx + 10.0, ry + 2.0, 24.0, sbh, if has_q { WHITE } else { DIM }, 1.0);
        if self.search.is_empty() {
            g.text(
                "Buscar proyecto, tecnología o rama… (escribe directo o arrastra una carpeta a la isla)",
                &g.f.sub,
                sbx + 36.0,
                ry + 2.0,
                sbw - 48.0,
                sbh,
                DIM,
                1.0,
            );
        } else {
            g.text(&self.search, &g.f.body, sbx + 36.0, ry + 2.0, sbw - 70.0, sbh, WHITE, 1.0);
            let cur_x = (sbx + 36.0 + g.measure(&self.search, &g.f.body) + 1.5).min(sbx + sbw - 36.0);
            g.line(cur_x, ry + 11.0, cur_x, ry + 29.0, WHITE, 0.9, 1.4);
            self.icon_btn(sbx + sbw - 32.0, ry + 6.0, 28.0, G_CLOSE, Hit::ClearSearch);
        }
        self.hit(sbx, ry + 2.0, sbw - 34.0, sbh, Hit::SearchBox);
        ry += 44.0;

        // 3. Lista de proyectos filtrados (con botones visibles en la barra ancha)
        let filtered = self.filtered_projects();
        if filtered.is_empty() {
            let msg = if self.cfg.projects.is_empty() {
                "Crea uno nuevo o agrega uno existente en Ajustes"
            } else {
                "No hay proyectos que coincidan con tu búsqueda"
            };
            g.text(msg, &g.f.sub, lx + 24.0, ry, tw - 48.0, 54.0, DIM, 1.0);
            ry += 54.0;
        }

        let max_scroll = filtered.len().saturating_sub(MAX_ROWS);
        let cur_scroll = self.scroll.min(max_scroll);

        for (vis_idx, &proj_idx) in filtered.iter().enumerate().skip(cur_scroll).take(MAX_ROWS) {
            let Some(p) = self.cfg.projects.get(proj_idx) else { continue };
            let id = Hit::Project(proj_idx);
            let is_kbd_sel = !self.search.is_empty() && vis_idx == self.sel_idx.min(filtered.len().saturating_sub(1));
            self.row_bg(lx, ry, tw, 54.0, id, is_kbd_sel);

            g.draw_stack_badge(&p.stack, lx + 18.0, ry + 9.0, 36.0);

            let title_x = lx + 64.0;
            let right_reserve = 250.0;
            let max_title_w = tw - 64.0 - right_reserve;
            let display_name = if p.pinned {
                format!("📌 {}", p.name)
            } else {
                p.name.clone()
            };
            g.text(&display_name, &g.f.title, title_x, ry + 7.0, max_title_w, 20.0, WHITE, 1.0);

            let git_opt = self.git_states.get(&p.path);
            if let Some(git) = git_opt {
                let sub_base = format!("{}  ·  ⎇ {}", p.stack, git.branch);
                g.text(&sub_base, &g.f.sub, title_x, ry + 27.0, max_title_w, 18.0, GRAY, 1.0);
            } else {
                g.text(&p.stack, &g.f.sub, title_x, ry + 27.0, max_title_w, 18.0, GRAY, 1.0);
            }

            // Controles siempre visibles a la derecha de cada fila (sin depender del clic derecho)
            let mut bx = lx + tw - 40.0;
            // 1. Fijar arriba (Pin)
            self.mini_action_btn(bx, ry + 13.0, 26.0, G_PIN, WHITE, p.pinned, Hit::ProjPin(proj_idx));
            bx -= 30.0;
            // 2. Abrir en Explorador
            self.mini_action_btn(bx, ry + 13.0, 26.0, G_FOLDER, WHITE, false, Hit::ProjExplorer(proj_idx));
            bx -= 30.0;
            // 3. Abrir Terminal en carpeta
            self.mini_action_btn(bx, ry + 13.0, 26.0, G_TERM, WHITE, false, Hit::ProjTerm(proj_idx));

            // 4. Botón visible de Git / Commit & Push / GitHub
            let git_dirty = git_opt.map(|g| g.dirty).unwrap_or(false);
            let git_cnt = git_opt.map(|g| g.changed_count).unwrap_or(0);
            let git_lbl = if git_dirty {
                format!("⎇ Commit ({git_cnt})")
            } else if git_opt.is_some() {
                "⎇ Git".into()
            } else {
                "⎇ GitHub".into()
            };
            let git_w = if git_dirty { 76.0 } else { 56.0 };
            bx -= git_w + 4.0;
            self.pill_action_btn(
                bx,
                ry + 14.0,
                git_w,
                26.0,
                &git_lbl,
                if git_dirty { AMBER } else { WHITE },
                git_dirty,
                Hit::ProjGit(proj_idx),
            );

            // 5. Controles de Servidor en Vivo (Play / Puerto + Terminal de servidor + Stop)
            let srv_state = self.servers.get(&p.path);
            let has_dev = actions::detect_dev_command(Path::new(&p.path), &p.stack).is_some();

            if let Some(srv) = srv_state {
                bx -= 30.0;
                self.mini_action_btn(bx, ry + 13.0, 26.0, G_STOP, RED, true, Hit::ProjServer(proj_idx));

                bx -= 30.0;
                self.mini_action_btn(bx, ry + 13.0, 26.0, G_TERM, GREEN, true, Hit::ProjServerTerm(proj_idx));

                let port_num = srv.port.unwrap_or(0);
                let badge_w = 58.0;
                bx -= badge_w + 4.0;
                let hv_p = self.hov(Hit::ProjPort(proj_idx, port_num));
                g.rrect(bx, ry + 14.0, badge_w, 26.0, 7.0, GREEN, if hv_p { 0.25 } else { 0.14 });
                g.rrect_stroke(bx + 0.5, ry + 14.5, badge_w - 1.0, 25.0, 7.0, GREEN, 0.55, 1.0);
                let p_lbl = if port_num > 0 {
                    format!("● :{port_num}")
                } else {
                    "● ...".into()
                };
                g.text(&p_lbl, &g.f.small_c, bx, ry + 14.0, badge_w, 26.0, GREEN, 1.0);
                if port_num > 0 {
                    self.hit(bx, ry + 14.0, badge_w, 26.0, Hit::ProjPort(proj_idx, port_num));
                } else {
                    self.hit(bx, ry + 14.0, badge_w, 26.0, Hit::ProjServerTerm(proj_idx));
                }
            } else if has_dev {
                bx -= 30.0;
                self.mini_action_btn(bx, ry + 13.0, 26.0, G_PLAY, GREEN, false, Hit::ProjServer(proj_idx));
            }

            ry += 54.0;
        }

        let id = Hit::NewProject;
        self.row_bg(lx, ry + 4.0, tw, 54.0, id, false);
        g.rrect(lx + 18.0, ry + 13.0, 36.0, 36.0, 10.0, WHITE, 0.08);
        g.text(G_ADD, &g.f.icon_sm, lx + 18.0, ry + 13.0, 36.0, 36.0, WHITE, 1.0);
        g.text(
            "Crear nuevo proyecto",
            &g.f.title,
            lx + 64.0,
            ry + 4.0,
            tw - 160.0,
            54.0,
            WHITE,
            1.0,
        );
        self.chip(lx + tw - 72.0, ry + 20.0, 54.0, "Ctrl N");
    }

    fn monitor_label(&self) -> String {
        match self.cfg.monitor_mode.as_str() {
            "cursor" => "Seguir cursor".into(),
            s if s.starts_with("monitor:") => {
                let n = s.trim_start_matches("monitor:").parse::<usize>().unwrap_or(0) + 1;
                format!("Monitor {n}")
            }
            _ => "Principal".into(),
        }
    }

    fn draw_settings(&self, lx: f32, y: f32, tw: f32) {
        let g = &self.gfx;
        self.header(lx, y, tw, "Ajustes", "Configuración de Isla Dev", true);
        let launcher = self
            .cfg
            .launcher(&self.cfg.default_launcher)
            .map(|l| l.name.clone())
            .unwrap_or_default();
        let mon_lbl = self.monitor_label();
        let items: [(&str, &str, Hit); 9] = [
            (G_POWER, "Mostrar isla en pantalla", Hit::ToggleVisibility),
            (G_POWER, "Ocultar en pantalla completa (Juegos / Video)", Hit::ToggleFullscreenHide),
            (G_GLOBE, "Monitor de la isla", Hit::CycleMonitor),
            (G_POWER, "Iniciar con Windows", Hit::Startup),
            (G_TERM, "Abrir con (predeterminado)", Hit::DefaultLauncher),
            (G_FOLDER, "Carpeta de proyectos", Hit::BaseDir),
            (G_EDIT, "Personalizar Programas, Frameworks y Librerías", Hit::OpenCustomEditor),
            (G_ADD, "Agregar proyecto existente", Hit::AddExisting),
            (G_EXIT, "Salir de Isla Dev", Hit::Quit),
        ];
        let mut ry = y + 66.0;
        for (glyph, label, id) in items {
            self.row_bg(lx, ry, tw, 48.0, id, false);
            let c = if id == Hit::Quit { RED } else { WHITE };
            g.text(
                glyph,
                &g.f.icon_sm,
                lx + 22.0,
                ry,
                28.0,
                48.0,
                if id == Hit::Quit { RED } else { GRAY },
                1.0,
            );
            g.text(label, &g.f.body, lx + 58.0, ry, tw - 210.0, 48.0, c, 1.0);
            let vx = lx + tw - 210.0;
            match id {
                Hit::ToggleVisibility => {
                    let on = self.visible;
                    let tx = lx + tw - 66.0;
                    g.rrect(tx, ry + 13.0, 42.0, 22.0, 11.0, if on { WHITE } else { 0x27272A }, 1.0);
                    g.circle(
                        if on { tx + 31.0 } else { tx + 11.0 },
                        ry + 24.0,
                        8.0,
                        if on { 0x000000 } else { 0x71717A },
                        1.0,
                    );
                }
                Hit::ToggleFullscreenHide => {
                    let on = self.cfg.hide_on_fullscreen;
                    let tx = lx + tw - 66.0;
                    g.rrect(tx, ry + 13.0, 42.0, 22.0, 11.0, if on { WHITE } else { 0x27272A }, 1.0);
                    g.circle(
                        if on { tx + 31.0 } else { tx + 11.0 },
                        ry + 24.0,
                        8.0,
                        if on { 0x000000 } else { 0x71717A },
                        1.0,
                    );
                }
                Hit::CycleMonitor => {
                    g.text(&mon_lbl, &g.f.sub_r, vx, ry, 170.0, 48.0, GRAY, 1.0);
                    g.text(G_CHEV, &g.f.icon_sm, lx + tw - 36.0, ry, 20.0, 48.0, DIM, 1.0);
                }
                Hit::Startup => {
                    let on = self.startup;
                    let tx = lx + tw - 66.0;
                    g.rrect(tx, ry + 13.0, 42.0, 22.0, 11.0, if on { WHITE } else { 0x27272A }, 1.0);
                    g.circle(
                        if on { tx + 31.0 } else { tx + 11.0 },
                        ry + 24.0,
                        8.0,
                        if on { 0x000000 } else { 0x71717A },
                        1.0,
                    );
                }
                Hit::DefaultLauncher => {
                    g.text(&launcher, &g.f.sub_r, vx, ry, 170.0, 48.0, GRAY, 1.0);
                    g.text(G_CHEV, &g.f.icon_sm, lx + tw - 36.0, ry, 20.0, 48.0, DIM, 1.0);
                }
                Hit::BaseDir => {
                    g.text(&self.cfg.base_dir, &g.f.sub_r, vx, ry, 170.0, 48.0, GRAY, 1.0);
                    g.text(G_CHEV, &g.f.icon_sm, lx + tw - 36.0, ry, 20.0, 48.0, DIM, 1.0);
                }
                Hit::OpenCustomEditor => {
                    g.text("Abrir editor visual", &g.f.sub_r, vx, ry, 170.0, 48.0, GRAY, 1.0);
                    g.text(G_CHEV, &g.f.icon_sm, lx + tw - 36.0, ry, 20.0, 48.0, DIM, 1.0);
                }
                _ => {}
            }
            ry += 48.0;
        }
        // Pie de firma y versión del sistema
        g.line(lx + 20.0, ry + 4.0, lx + tw - 20.0, ry + 4.0, WHITE, 0.06, 1.0);
        g.text(
            &format!("{APP_AUTHOR}  ·  Isla Dev {APP_VERSION}"),
            &g.f.small_c,
            lx + 16.0,
            ry + 8.0,
            tw - 32.0,
            22.0,
            GRAY,
            0.85,
        );
    }

    fn cur_libs_len(&self) -> usize {
        self.cfg
            .templates
            .get(self.wiz_tpl)
            .map(|t| t.libraries.len())
            .unwrap_or(0)
    }

    fn draw_wizard(&self, lx: f32, y: f32, tw: f32, th: f32) {
        let g = &self.gfx;
        let has_libs = self.cur_libs_len() > 0;
        let total = if has_libs { 3 } else { 2 };
        let (n, label) = match self.step {
            Step::Name => (1, "Nombre, GitHub y framework"),
            Step::Libraries => (2, "Librerías compatibles"),
            Step::Launcher => (total, "Abrir con"),
        };
        self.header(lx, y, tw, "Nuevo proyecto", &format!("Paso {n} de {total} · {label}"), true);
        let mut ry = y + 70.0;
        match self.step {
            Step::Name => {
                let bx = lx + 16.0;
                let bw = tw - 32.0;
                g.rrect(bx, ry, bw, 42.0, 10.0, WHITE, 0.06);
                g.rrect_stroke(bx + 0.5, ry + 0.5, bw - 1.0, 41.0, 10.0, WHITE, 0.6, 1.0);
                if self.wiz_name.is_empty() {
                    g.text("nombre-del-proyecto", &g.f.input, bx + 14.0, ry, bw - 28.0, 42.0, DIM, 1.0);
                } else {
                    g.text(&self.wiz_name, &g.f.input, bx + 14.0, ry, bw - 28.0, 42.0, WHITE, 1.0);
                }
                let cx = bx + 14.0 + g.measure(&self.wiz_name, &g.f.input) + 1.0;
                g.line(cx, ry + 11.0, cx, ry + 31.0, WHITE, 1.0, 1.5);
                self.hit(bx, ry, bw, 42.0, Hit::Input);
                ry += 46.0;

                let (txt, c) = match &self.wiz_err {
                    Some(e) => (e.clone(), RED),
                    None => {
                        let nm = if self.wiz_name.is_empty() { "…" } else { &self.wiz_name };
                        (format!("{}\\{}", self.cfg.base_dir, nm), DIM)
                    }
                };
                g.text(&txt, &g.f.sub, bx + 4.0, ry, bw - 8.0, 20.0, c, 1.0);
                ry += 24.0;

                // Opción directa para subir a GitHub automáticamente al crear el proyecto
                let gh_hov = self.hov(Hit::WizToggleGithub);
                g.rrect(bx, ry, bw, 38.0, 10.0, WHITE, if gh_hov { 0.09 } else { 0.05 });
                g.rrect_stroke(
                    bx + 0.5,
                    ry + 0.5,
                    bw - 1.0,
                    37.0,
                    10.0,
                    if self.wiz_github { GREEN } else { BORDER },
                    0.7,
                    1.0,
                );
                if self.wiz_github {
                    g.rrect(bx + 12.0, ry + 9.0, 20.0, 20.0, 6.0, GREEN, 1.0);
                    g.text(G_CHECK, &g.f.icon_sm, bx + 12.0, ry + 9.0, 20.0, 20.0, 0x000000, 1.0);
                } else {
                    g.rrect_stroke(bx + 12.5, ry + 9.5, 19.0, 19.0, 6.0, GRAY, 0.8, 1.2);
                }
                g.text(
                    "Subir a GitHub automáticamente al crear (git init + commit + push)",
                    &g.f.body,
                    bx + 42.0,
                    ry,
                    bw - 52.0,
                    38.0,
                    WHITE,
                    1.0,
                );
                self.hit(bx, ry, bw, 38.0, Hit::WizToggleGithub);
                ry += 44.0;

                let max_vis = 5usize;
                let max_sc = self.cfg.templates.len().saturating_sub(max_vis);
                let sc = self.wiz_tpl_scroll.min(max_sc);
                for (i, t) in self.cfg.templates.iter().enumerate().skip(sc).take(max_vis) {
                    let sel = i == self.wiz_tpl;
                    self.row_bg(lx, ry, tw, 46.0, Hit::Template(i), sel);
                    g.draw_stack_badge(&t.stack, lx + 20.0, ry + 5.0, 36.0);
                    g.text(&t.name, &g.f.body, lx + 66.0, ry, tw - 220.0, 46.0, WHITE, 1.0);
                    g.text(&t.stack, &g.f.sub_r, lx + tw - 200.0, ry, 148.0, 46.0, GRAY, 1.0);
                    if sel {
                        g.text(G_CHECK, &g.f.icon_sm, lx + tw - 44.0, ry, 20.0, 46.0, WHITE, 1.0);
                    }
                    ry += 46.0;
                }
            }
            Step::Libraries => {
                if let Some(t) = self.cfg.templates.get(self.wiz_tpl) {
                    let max_vis = 6usize;
                    let max_sc = t.libraries.len().saturating_sub(max_vis);
                    let sc = self.wiz_lib_scroll.min(max_sc);
                    for (i, l) in t.libraries.iter().enumerate().skip(sc).take(max_vis) {
                        let on = self.wiz_libs.get(i).copied().unwrap_or(false);
                        self.row_bg(lx, ry, tw, 46.0, Hit::Library(i), false);
                        if on {
                            g.rrect(lx + 24.0, ry + 13.0, 20.0, 20.0, 6.0, WHITE, 1.0);
                            g.text(G_CHECK, &g.f.icon_sm, lx + 24.0, ry + 13.0, 20.0, 20.0, 0x000000, 1.0);
                        } else {
                            g.rrect_stroke(lx + 24.5, ry + 13.5, 19.0, 19.0, 6.0, GRAY, 0.8, 1.2);
                        }
                        g.text(&l.name, &g.f.body, lx + 58.0, ry, 190.0, 46.0, WHITE, 1.0);
                        g.text(&l.command, &g.f.sub_r, lx + 245.0, ry, tw - 268.0, 46.0, DIM, 1.0);
                        ry += 46.0;
                    }
                }
                // Botón directo para configurar o añadir nuevas librerías
                self.row_bg(lx, ry + 2.0, tw, 44.0, Hit::WizOpenLibEditor, false);
                g.text(G_ADD, &g.f.icon_sm, lx + 22.0, ry + 2.0, 28.0, 44.0, GREEN, 1.0);
                g.text(
                    "Configurar o agregar nueva librería a este framework…",
                    &g.f.body,
                    lx + 58.0,
                    ry + 2.0,
                    tw - 80.0,
                    44.0,
                    GREEN,
                    1.0,
                );
            }
            Step::Launcher => {
                for (i, l) in self.cfg.launchers.iter().enumerate().take(6) {
                    let sel = i == self.wiz_launcher;
                    self.row_bg(lx, ry, tw, 46.0, Hit::Launcher(i), sel);
                    g.ring(lx + 34.0, ry + 23.0, 8.0, if sel { WHITE } else { GRAY }, 1.0, 1.5);
                    if sel {
                        g.circle(lx + 34.0, ry + 23.0, 4.5, WHITE, 1.0);
                    }
                    g.text(&l.name, &g.f.body, lx + 58.0, ry, 200.0, 46.0, WHITE, 1.0);
                    g.text(&l.command, &g.f.sub_r, lx + 250.0, ry, tw - 274.0, 46.0, DIM, 1.0);
                    ry += 46.0;
                }
                self.row_bg(lx, ry + 2.0, tw, 44.0, Hit::WizOpenLauncherEditor, false);
                g.text(G_ADD, &g.f.icon_sm, lx + 22.0, ry + 2.0, 28.0, 44.0, GREEN, 1.0);
                g.text(
                    "Agregar otro programa (.exe) para abrir proyectos…",
                    &g.f.body,
                    lx + 58.0,
                    ry + 2.0,
                    tw - 80.0,
                    44.0,
                    GREEN,
                    1.0,
                );
            }
        }
        let by = y + th - 58.0;
        g.line(lx + 16.0, by, lx + tw - 16.0, by, WHITE, 0.08, 1.0);
        g.text("Enter continuar · Esc volver", &g.f.sub, lx + 22.0, by + 8.0, 220.0, 44.0, DIM, 1.0);
        let last = self.step == Step::Launcher;
        let bw = if last { 160.0 } else { 130.0 };
        let bx = lx + tw - 16.0 - bw;
        let hv = self.hov(Hit::Next);
        g.rrect(bx, by + 12.0, bw, 36.0, 18.0, WHITE, if hv { 1.0 } else { 0.92 });
        g.text(
            if last { "Crear y abrir" } else { "Siguiente" },
            &g.f.btn,
            bx,
            by + 12.0,
            bw,
            36.0,
            0x000000,
            1.0,
        );
        self.hit(bx, by + 12.0, bw, 36.0, Hit::Next);
    }

    fn draw_git_modal(&self, lx: f32, y: f32, tw: f32, _th: f32) {
        let g = &self.gfx;
        let Some(p) = self.cfg.projects.get(self.git_proj_idx) else { return };
        let git_opt = self.git_states.get(&p.path);
        let branch = git_opt.map(|x| x.branch.as_str()).unwrap_or("main");
        let dirty_cnt = git_opt.map(|x| x.changed_count).unwrap_or(0);
        let has_remote = git_opt.and_then(|x| x.remote_url.as_ref()).is_some();

        let sub = if has_remote {
            format!("Rama ⎇ {branch} · {dirty_cnt} cambios pendientes · Conectado a GitHub")
        } else {
            format!("Rama ⎇ {branch} · Se creará y subirá a tu cuenta de GitHub")
        };
        self.header(lx, y, tw, &format!("Git & GitHub · {}", p.name), &sub, true);

        let mut ry = y + 76.0;
        let bx = lx + 16.0;
        let bw = tw - 32.0;

        g.text("Mensaje del Commit:", &g.f.sub, bx, ry, bw, 20.0, GRAY, 1.0);
        ry += 22.0;

        g.rrect(bx, ry, bw, 42.0, 10.0, WHITE, 0.07);
        g.rrect_stroke(bx + 0.5, ry + 0.5, bw - 1.0, 41.0, 10.0, WHITE, 0.65, 1.0);
        if self.git_commit_msg.is_empty() {
            g.text(
                "Escribe el nombre/mensaje del commit…",
                &g.f.input,
                bx + 14.0,
                ry,
                bw - 28.0,
                42.0,
                DIM,
                1.0,
            );
        } else {
            g.text(&self.git_commit_msg, &g.f.input, bx + 14.0, ry, bw - 28.0, 42.0, WHITE, 1.0);
        }
        let cur_x = (bx + 14.0 + g.measure(&self.git_commit_msg, &g.f.input) + 1.5).min(bx + bw - 16.0);
        g.line(cur_x, ry + 11.0, cur_x, ry + 31.0, WHITE, 1.0, 1.5);
        self.hit(bx, ry, bw, 42.0, Hit::GitCommitInput);
        ry += 52.0;

        if !has_remote {
            let vis_hov = self.hov(Hit::GitTogglePublic);
            g.rrect(bx, ry, bw, 36.0, 9.0, WHITE, if vis_hov { 0.09 } else { 0.05 });
            g.rrect_stroke(bx + 0.5, ry + 0.5, bw - 1.0, 35.0, 9.0, BORDER, 0.8, 1.0);
            let vis_txt = if self.git_public {
                "Visibilidad en GitHub: 🌐 Público (clic para cambiar a Privado)"
            } else {
                "Visibilidad en GitHub: 🔒 Privado (clic para cambiar a Público)"
            };
            g.text(vis_txt, &g.f.body, bx + 14.0, ry, bw - 28.0, 36.0, WHITE, 1.0);
            self.hit(bx, ry, bw, 36.0, Hit::GitTogglePublic);
            ry += 44.0;
        }

        // Botón principal: Hacer Commit y Push / Subir a GitHub
        let hv_push = self.hov(Hit::GitDoCommitPush);
        g.rrect(bx, ry, bw, 44.0, 11.0, WHITE, if hv_push { 1.0 } else { 0.92 });
        let btn_lbl = if has_remote {
            format!("{G_UPLOAD}  Hacer Commit y Push a GitHub")
        } else {
            format!("{G_UPLOAD}  Inicializar, Hacer Commit y Subir a GitHub")
        };
        g.text(&btn_lbl, &g.f.btn, bx, ry, bw, 44.0, 0x000000, 1.0);
        self.hit(bx, ry, bw, 44.0, Hit::GitDoCommitPush);
        ry += 54.0;

        // Botones secundarios: Git Pull, Abrir en GitHub, Ver log en terminal
        let third_w = (bw - 16.0) / 3.0;
        self.pill_action_btn(bx, ry, third_w, 36.0, &format!("{G_DOWNLOAD} Git Pull"), WHITE, false, Hit::GitDoPull);
        self.pill_action_btn(
            bx + third_w + 8.0,
            ry,
            third_w,
            36.0,
            &format!("{G_GLOBE} Abrir en GitHub"),
            WHITE,
            has_remote,
            Hit::GitOpenRepoWeb,
        );
        self.pill_action_btn(
            bx + (third_w + 8.0) * 2.0,
            ry,
            third_w,
            36.0,
            &format!("{G_TERM} Ver terminal Git"),
            GREEN,
            false,
            Hit::GitOpenLogTerm,
        );
    }

    fn draw_custom_editor(&self, lx: f32, y: f32, tw: f32, _th: f32) {
        let g = &self.gfx;
        self.header(
            lx,
            y,
            tw,
            "Personalizar Isla Dev",
            "Agrega librerías, programas (.exe) o frameworks sin editar archivos",
            true,
        );
        let mut ry = y + 72.0;
        let bx = lx + 16.0;
        let bw = tw - 32.0;

        // 3 Pestañas superiores
        let tabs = ["1. Librerías", "2. Programas (Abrir con)", "3. Frameworks"];
        let tab_w = (bw - 16.0) / 3.0;
        for (i, t_lbl) in tabs.iter().enumerate() {
            let tx = bx + i as f32 * (tab_w + 8.0);
            let active = self.ed_tab == i;
            self.pill_action_btn(tx, ry, tab_w, 32.0, t_lbl, WHITE, active, Hit::EdTab(i));
        }
        ry += 42.0;

        match self.ed_tab {
            0 => {
                // Pestaña 0: Configurar Librerías por Framework
                let tpl_idx = self.ed_tpl_idx.min(self.cfg.templates.len().saturating_sub(1));
                let tpl_name = self
                    .cfg
                    .templates
                    .get(tpl_idx)
                    .map(|t| format!("Framework: {} ({})", t.name, t.stack))
                    .unwrap_or_else(|| "Sin plantillas".into());

                self.pill_action_btn(bx, ry, 42.0, 32.0, "◀", WHITE, false, Hit::EdPrevTpl);
                g.rrect(bx + 48.0, ry, bw - 96.0, 32.0, 8.0, CARD, 0.9);
                g.text(&tpl_name, &g.f.small_c, bx + 48.0, ry, bw - 96.0, 32.0, WHITE, 1.0);
                self.pill_action_btn(bx + bw - 42.0, ry, 42.0, 32.0, "▶", WHITE, false, Hit::EdNextTpl);
                ry += 40.0;

                // Campos para agregar nueva librería
                let f1_w = 180.0;
                let f2_w = bw - f1_w - 118.0;
                self.draw_input_box(bx, ry, f1_w, 36.0, "Nombre (ej. Prisma)", &self.ed_f1, self.ed_focus == 0, Hit::EdField(0));
                self.draw_input_box(
                    bx + f1_w + 8.0,
                    ry,
                    f2_w,
                    36.0,
                    "Comando (ej. npm i prisma)",
                    &self.ed_f2,
                    self.ed_focus == 1,
                    Hit::EdField(1),
                );
                self.pill_action_btn(
                    bx + f1_w + f2_w + 16.0,
                    ry,
                    102.0,
                    36.0,
                    "+ Agregar",
                    GREEN,
                    true,
                    Hit::EdSave,
                );
                ry += 44.0;

                if let Some((msg, is_err)) = &self.ed_msg {
                    g.text(msg, &g.f.sub, bx + 4.0, ry - 4.0, bw, 18.0, if *is_err { RED } else { GREEN }, 1.0);
                    ry += 18.0;
                }

                // Lista de librerías actuales de este framework
                if let Some(t) = self.cfg.templates.get(tpl_idx) {
                    let max_vis = 5usize;
                    let max_sc = t.libraries.len().saturating_sub(max_vis);
                    let sc = self.ed_scroll.min(max_sc);
                    for (i, l) in t.libraries.iter().enumerate().skip(sc).take(max_vis) {
                        self.row_bg(lx, ry, tw, 44.0, Hit::EdToggleLibDefault(i), false);
                        if l.default {
                            g.rrect(lx + 22.0, ry + 12.0, 20.0, 20.0, 6.0, GREEN, 1.0);
                            g.text(G_CHECK, &g.f.icon_sm, lx + 22.0, ry + 12.0, 20.0, 20.0, 0x000000, 1.0);
                        } else {
                            g.rrect_stroke(lx + 22.5, ry + 12.5, 19.0, 19.0, 6.0, GRAY, 0.7, 1.2);
                        }
                        g.text(&l.name, &g.f.body, lx + 52.0, ry, 165.0, 44.0, WHITE, 1.0);
                        g.text(&l.command, &g.f.sub_r, lx + 220.0, ry, tw - 275.0, 44.0, DIM, 1.0);
                        self.mini_action_btn(lx + tw - 44.0, ry + 9.0, 26.0, G_TRASH, RED, false, Hit::EdDeleteItem(i));
                        ry += 44.0;
                    }
                }
            }
            1 => {
                // Pestaña 1: Configurar Programas ("Abrir con...") con selector de archivo .exe
                self.draw_input_box(
                    bx,
                    ry,
                    bw,
                    36.0,
                    "Nombre del programa (ej. WebStorm, Sublime Text, Rider)",
                    &self.ed_f1,
                    self.ed_focus == 0,
                    Hit::EdField(0),
                );
                ry += 42.0;

                let cmd_w = bw - 138.0;
                self.draw_input_box(
                    bx,
                    ry,
                    cmd_w,
                    36.0,
                    "Comando o ruta .exe (ej. \"C:\\...\\app.exe\" \"{path}\")",
                    &self.ed_f2,
                    self.ed_focus == 1,
                    Hit::EdField(1),
                );
                self.pill_action_btn(
                    bx + cmd_w + 8.0,
                    ry,
                    130.0,
                    36.0,
                    "📁 Elegir .exe…",
                    WHITE,
                    true,
                    Hit::EdPickExe,
                );
                ry += 44.0;

                self.pill_action_btn(bx, ry, bw, 36.0, "+ Guardar programa en 'Abrir con'", GREEN, true, Hit::EdSave);
                ry += 44.0;

                if let Some((msg, is_err)) = &self.ed_msg {
                    g.text(msg, &g.f.sub, bx + 4.0, ry - 4.0, bw, 18.0, if *is_err { RED } else { GREEN }, 1.0);
                    ry += 18.0;
                }

                let max_vis = 4usize;
                let max_sc = self.cfg.launchers.len().saturating_sub(max_vis);
                let sc = self.ed_scroll.min(max_sc);
                for (i, l) in self.cfg.launchers.iter().enumerate().skip(sc).take(max_vis) {
                    self.row_bg(lx, ry, tw, 44.0, Hit::Launcher(i), false);
                    g.text(&l.name, &g.f.body, lx + 24.0, ry, 190.0, 44.0, WHITE, 1.0);
                    g.text(&l.command, &g.f.sub_r, lx + 215.0, ry, tw - 270.0, 44.0, DIM, 1.0);
                    if self.cfg.launchers.len() > 1 {
                        self.mini_action_btn(lx + tw - 44.0, ry + 9.0, 26.0, G_TRASH, RED, false, Hit::EdDeleteItem(i));
                    }
                    ry += 44.0;
                }
            }
            _ => {
                // Pestaña 2: Agregar Nuevo Framework / Plantilla
                let name_w = bw - 190.0;
                self.draw_input_box(
                    bx,
                    ry,
                    name_w,
                    36.0,
                    "Nombre (ej. Nuxt 3, Remix, Laravel)",
                    &self.ed_f1,
                    self.ed_focus == 0,
                    Hit::EdField(0),
                );
                self.pill_action_btn(
                    bx + name_w + 8.0,
                    ry,
                    182.0,
                    36.0,
                    &format!("Icono: {} ▸", self.ed_f3),
                    WHITE,
                    true,
                    Hit::EdCycleStack,
                );
                ry += 42.0;

                self.draw_input_box(
                    bx,
                    ry,
                    bw,
                    36.0,
                    "Comando de creación (usa {name}, ej. npx nuxi@latest init {name})",
                    &self.ed_f2,
                    self.ed_focus == 1,
                    Hit::EdField(1),
                );
                ry += 44.0;

                self.pill_action_btn(bx, ry, bw, 36.0, "+ Guardar nuevo Framework", GREEN, true, Hit::EdSave);
                ry += 44.0;

                if let Some((msg, is_err)) = &self.ed_msg {
                    g.text(msg, &g.f.sub, bx + 4.0, ry - 4.0, bw, 18.0, if *is_err { RED } else { GREEN }, 1.0);
                    ry += 18.0;
                }

                let max_vis = 4usize;
                let max_sc = self.cfg.templates.len().saturating_sub(max_vis);
                let sc = self.ed_scroll.min(max_sc);
                for (i, t) in self.cfg.templates.iter().enumerate().skip(sc).take(max_vis) {
                    self.row_bg(lx, ry, tw, 44.0, Hit::Template(i), false);
                    g.draw_stack_badge(&t.stack, lx + 20.0, ry + 6.0, 32.0);
                    g.text(&t.name, &g.f.body, lx + 60.0, ry, 180.0, 44.0, WHITE, 1.0);
                    let cmd_preview = t.create.first().or(t.setup.first()).map(|s| s.as_str()).unwrap_or("Carpeta limpia");
                    g.text(cmd_preview, &g.f.sub_r, lx + 240.0, ry, tw - 295.0, 44.0, DIM, 1.0);
                    if self.cfg.templates.len() > 1 {
                        self.mini_action_btn(lx + tw - 44.0, ry + 9.0, 26.0, G_TRASH, RED, false, Hit::EdDeleteItem(i));
                    }
                    ry += 44.0;
                }
            }
        }
    }

    fn draw_input_box(&self, x: f32, y: f32, w: f32, h: f32, placeholder: &str, val: &str, focused: bool, id: Hit) {
        let g = &self.gfx;
        g.rrect(x, y, w, h, 8.0, WHITE, if focused { 0.08 } else { 0.04 });
        g.rrect_stroke(
            x + 0.5,
            y + 0.5,
            w - 1.0,
            h - 1.0,
            8.0,
            if focused { WHITE } else { BORDER },
            if focused { 0.65 } else { 0.7 },
            1.0,
        );
        if val.is_empty() {
            g.text(placeholder, &g.f.sub, x + 10.0, y, w - 20.0, h, DIM, 1.0);
        } else {
            g.text(val, &g.f.body, x + 10.0, y, w - 20.0, h, WHITE, 1.0);
        }
        if focused {
            let cx = (x + 10.0 + g.measure(val, &g.f.body) + 1.5).min(x + w - 10.0);
            g.line(cx, y + 9.0, cx, y + h - 9.0, WHITE, 0.95, 1.4);
        }
        self.hit(x, y, w, h, id);
    }

    fn draw_status(&self, lx: f32, y: f32, tw: f32, h: f32) {
        let g = &self.gfx;
        let cy = y + h / 2.0;
        match self.skind {
            SKind::Error => {
                g.circle(lx + 30.0, cy, 18.0, RED, 1.0);
                g.text(G_ERR, &g.f.icon, lx + 12.0, cy - 18.0, 36.0, 36.0, WHITE, 1.0);
            }
            _ => {
                g.draw_logo(lx + 14.0, cy - 18.0, 36.0, 36.0, 1.0);
            }
        }
        g.text(&self.s_title, &g.f.title, lx + 60.0, cy - 19.0, tw - 110.0, 19.0, WHITE, 1.0);
        g.text(
            &self.s_sub,
            &g.f.sub,
            lx + 60.0,
            cy + 1.0,
            tw - 110.0,
            18.0,
            if self.skind == SKind::Creating {
                WHITE
            } else if self.skind == SKind::Error {
                AMBER
            } else {
                GRAY
            },
            1.0,
        );
        let rx = lx + tw - 32.0;
        match self.skind {
            SKind::Opening | SKind::Creating => {
                for i in 0..8 {
                    let a = i as f32 / 8.0 * std::f32::consts::TAU + self.spin * 5.0;
                    let fade = 0.2 + 0.8 * (i as f32 / 8.0);
                    g.circle(rx + a.cos() * 8.0, cy + a.sin() * 8.0, 1.8, WHITE, fade);
                }
            }
            SKind::Done => {
                g.circle(rx, cy, 11.0, WHITE, 1.0);
                g.text(G_CHECK, &g.f.icon_sm, rx - 11.0, cy - 11.0, 22.0, 22.0, 0x000000, 1.0);
            }
            SKind::Error => {
                g.circle(rx, cy, 13.0, WHITE, 0.14);
                g.text(G_TERM, &g.f.icon_sm, rx - 11.0, cy - 11.0, 22.0, 22.0, WHITE, 1.0);
            }
        }
        self.hit(lx, y, tw, h, Hit::Pill);
    }

    // ---------- lógica ----------

    fn status(&mut self, kind: SKind, title: &str, sub: &str, color: u32, init: &str) {
        self.skind = kind;
        self.s_title = title.into();
        self.s_sub = sub.into();
        self.s_color = color;
        self.s_initial = init.into();
        if self.view == View::Status {
            self.fade();
        }
        self.set_view(View::Status);
        self.kick();
    }

    fn error(&mut self, title: &str, sub: &str, log_path: Option<PathBuf>) {
        self.s_log_path = log_path;
        self.status(SKind::Error, title, sub, RED, "!");
        self.s_deadline = Some(Instant::now() + Duration::from_secs(9));
    }

    fn toggle_project_server(&mut self, i: usize) {
        let Some(p) = self.cfg.projects.get(i).cloned() else { return };
        if let Some(srv) = self.servers.remove(&p.path) {
            actions::stop_dev_server(srv.pid);
            let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
            actions::scan_open_ports(extra_ports, self.tx.clone(), self.hwnd.0 as isize);
            self.kick();
            return;
        }
        let Some((cmd_str, default_port)) = actions::detect_dev_command(Path::new(&p.path), &p.stack) else {
            return;
        };
        let mut already_used = self.active_ports.clone();
        for s in self.servers.values() {
            if let Some(pt) = s.port {
                if !already_used.contains(&pt) {
                    already_used.push(pt);
                }
            }
        }
        match actions::start_dev_server(
            p.name.clone(),
            PathBuf::from(&p.path),
            cmd_str,
            default_port,
            already_used,
            self.tx.clone(),
            self.hwnd.0 as isize,
        ) {
            Ok((pid, log_path)) => {
                self.servers.insert(
                    p.path.clone(),
                    RunningServer {
                        pid,
                        port: None,
                        log_path,
                    },
                );
                self.kick();
            }
            Err(e) => self.error(&format!("No se pudo iniciar servidor en {}", p.name), &e, None),
        }
    }

    fn toggle_pin(&mut self, i: usize) {
        if let Some(p) = self.cfg.projects.get_mut(i) {
            p.pinned = !p.pinned;
        }
        self.cfg.sort_projects();
        self.save();
        self.kick();
    }

    fn open_project(&mut self, i: usize, launcher: Option<String>) {
        let Some(p) = self.cfg.projects.get(i).cloned() else { return };
        let lid = launcher
            .clone()
            .or(p.launcher.clone())
            .unwrap_or(self.cfg.default_launcher.clone());
        let Some(l) = self.cfg.launcher(&lid).cloned() else { return };
        match actions::launch(&l, &p.path) {
            Ok(()) => {
                self.s_log_path = None;
                self.status(SKind::Opening, "Abriendo…", &p.name, hex(&p.color), &initial(&p.name));
                self.s_next = format!("Se abrió en {}", l.name);
                self.s_deadline = Some(Instant::now() + Duration::from_millis(900));
                if let Some(pp) = self.cfg.projects.get_mut(i) {
                    pp.last_opened = config::now();
                    if launcher.is_some() {
                        pp.launcher = launcher;
                    }
                }
                self.cfg.sort_projects();
                self.scroll = 0;
                self.save();
            }
            Err(e) => self.error(&format!("No se pudo abrir {}", p.name), &e, None),
        }
    }

    fn start_wizard(&mut self) {
        self.wiz_name.clear();
        self.wiz_err = None;
        self.wiz_tpl = 0;
        self.wiz_tpl_scroll = 0;
        self.wiz_lib_scroll = 0;
        self.reset_libs();
        self.wiz_launcher = self.cfg.launcher_index(&self.cfg.default_launcher);
        self.step = Step::Name;
        self.set_view(View::Wizard);
    }

    fn reset_libs(&mut self) {
        self.wiz_libs = self
            .cfg
            .templates
            .get(self.wiz_tpl)
            .map(|t| t.libraries.iter().map(|l| l.default).collect())
            .unwrap_or_default();
        self.wiz_lib_scroll = 0;
    }

    fn goto_step(&mut self, s: Step) {
        self.step = s;
        self.hover = None;
        self.fade();
        self.kick();
    }

    fn wizard_next(&mut self) {
        match self.step {
            Step::Name => {
                let name = self.wiz_name.trim().to_string();
                if name.is_empty() {
                    self.wiz_err = Some("Escribe un nombre para el proyecto".into());
                    self.kick();
                    return;
                }
                if PathBuf::from(&self.cfg.base_dir).join(&name).exists() {
                    self.wiz_err = Some("Ya existe una carpeta con ese nombre".into());
                    self.kick();
                    return;
                }
                self.wiz_err = None;
                self.goto_step(if self.cur_libs_len() > 0 { Step::Libraries } else { Step::Launcher });
            }
            Step::Libraries => self.goto_step(Step::Launcher),
            Step::Launcher => self.create(),
        }
    }

    fn back(&mut self) {
        match self.view {
            View::Wizard => match self.step {
                Step::Name => self.set_view(View::Expanded),
                Step::Libraries => self.goto_step(Step::Name),
                Step::Launcher => {
                    self.goto_step(if self.cur_libs_len() > 0 { Step::Libraries } else { Step::Name })
                }
            },
            View::Settings | View::GitModal => self.set_view(View::Expanded),
            View::CustomEditor => self.set_view(View::Settings),
            View::Expanded if !self.search.is_empty() => {
                self.search.clear();
                self.sel_idx = 0;
                self.scroll = 0;
                self.kick();
            }
            _ => {
                self.resume_view = None;
                self.set_view(View::Compact);
            }
        }
    }

    fn create(&mut self) {
        let Some(tpl) = self.cfg.templates.get(self.wiz_tpl).cloned() else { return };
        let libs: Vec<_> = tpl
            .libraries
            .iter()
            .zip(&self.wiz_libs)
            .filter(|(_, on)| **on)
            .map(|(l, _)| l.clone())
            .collect();
        let name = self.wiz_name.trim().to_string();
        let launcher = self
            .cfg
            .launchers
            .get(self.wiz_launcher)
            .map(|l| l.id.clone())
            .unwrap_or_default();
        self.pending = Some(Pending {
            name: name.clone(),
            stack: tpl.stack.clone(),
            color: tpl.color.clone(),
            launcher,
        });
        let base = PathBuf::from(&self.cfg.base_dir);
        let log = config::logs_dir().join(format!("create-{}.log", actions::safe_slug(&name)));
        let tx = self.tx.clone();
        let hw = self.hwnd.0 as isize;
        let color = hex(&tpl.color);
        let auto_gh = self.wiz_github;
        self.s_log_path = Some(log.clone());
        self.s_proj_path = Some(base.join(&name));
        self.resume_view = None;
        std::thread::spawn(move || actions::scaffold(tpl, libs, name.clone(), base, auto_gh, log, tx, hw));
        let n = self.wiz_name.trim().to_string();
        self.status(SKind::Creating, &format!("Creando {n}"), "Preparando…", color, &initial(&n));
        self.s_deadline = None;
    }

    fn on_worker(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                WorkMsg::Progress(s) => {
                    self.s_sub = s;
                    self.kick();
                }
                WorkMsg::Done { res: Ok(path), warn } => {
                    let Some(p) = self.pending.take() else { continue };
                    self.cfg.projects.push(Project {
                        name: p.name.clone(),
                        path: path.to_string_lossy().to_string(),
                        stack: p.stack,
                        color: p.color,
                        launcher: Some(p.launcher.clone()),
                        last_opened: 0,
                        pinned: false,
                    });
                    let i = self.cfg.projects.len() - 1;
                    self.open_project(i, Some(p.launcher));
                    if let Some((w_msg, log_p)) = warn {
                        self.s_proj_path = Some(path);
                        self.error(
                            &format!("{} creado con advertencia en librería/Git", p.name),
                            &format!("{w_msg} · clic para ver terminal"),
                            Some(log_p),
                        );
                    }
                    self.refresh_live_state();
                }
                WorkMsg::Done { res: Err(e), warn: _ } => {
                    self.pending = None;
                    let log_p = self.s_log_path.clone();
                    self.error(
                        "Error al crear el proyecto",
                        &format!("{e} · clic aquí para ver el error en terminal"),
                        log_p,
                    );
                }
                WorkMsg::GitStates(map) => {
                    self.git_states = map;
                    self.kick();
                }
                WorkMsg::ActivePorts(mut ports) => {
                    for srv in self.servers.values() {
                        if let Some(sp) = srv.port {
                            if !ports.contains(&sp) && actions::is_port_open(sp) {
                                ports.push(sp);
                            }
                        }
                    }
                    ports.sort_unstable();
                    ports.dedup();
                    if self.active_ports != ports {
                        self.active_ports = ports;
                        self.kick();
                    }
                }
                WorkMsg::ServerPort { path, pid, port } => {
                    if let Some(srv) = self.servers.get_mut(&path) {
                        if srv.pid == pid {
                            srv.port = Some(port);
                        }
                    }
                    if !self.active_ports.contains(&port) {
                        self.active_ports.push(port);
                        self.active_ports.sort_unstable();
                    }
                    self.kick();
                }
                WorkMsg::ServerExited {
                    path,
                    pid,
                    err_reason,
                    log_path,
                } => {
                    if self.servers.get(&path).map(|s| s.pid) == Some(pid) {
                        self.servers.remove(&path);
                        let proj_name = Path::new(&path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Proyecto".into());
                        if let Some(reason) = err_reason {
                            self.s_proj_path = Some(PathBuf::from(&path));
                            self.error(
                                &format!("Error al iniciar servidor en {proj_name}"),
                                &format!("{reason} · clic aquí para ver la terminal"),
                                log_path,
                            );
                        }
                        let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
                        actions::scan_open_ports(extra_ports, self.tx.clone(), self.hwnd.0 as isize);
                        self.kick();
                    }
                }
                WorkMsg::Toast {
                    title,
                    sub,
                    is_err,
                    log_path,
                } => {
                    if is_err {
                        self.error(&title, &sub, log_path);
                    } else {
                        self.s_log_path = log_path;
                        self.status(SKind::Done, &title, &sub, WHITE, "✓");
                        self.s_deadline = Some(Instant::now() + Duration::from_millis(2200));
                    }
                }
            }
        }
    }

    fn add_existing(&mut self) {
        if let Some(p) = actions::pick_folder(self.hwnd, "Agregar proyecto existente") {
            self.register_and_open_folder(p, false);
        }
        self.focus();
        self.kick();
    }

    fn register_and_open_folder(&mut self, p: PathBuf, open_immediately: bool) {
        if !p.is_dir() {
            return;
        }
        let path = p.to_string_lossy().to_string();
        if !self.cfg.projects.iter().any(|x| x.path.eq_ignore_ascii_case(&path)) {
            let (stack, color) = actions::detect(&p);
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(path.clone());
            self.cfg.projects.push(Project {
                name,
                path: path.clone(),
                stack,
                color,
                launcher: None,
                last_opened: config::now(),
                pinned: false,
            });
            self.cfg.sort_projects();
            self.save();
            self.refresh_live_state();
        }
        if open_immediately {
            if let Some(idx) = self.cfg.projects.iter().position(|x| x.path.eq_ignore_ascii_case(&path)) {
                self.open_project(idx, None);
            }
        } else {
            self.set_view(View::Expanded);
        }
    }

    fn save_custom_editor_item(&mut self) {
        let f1 = self.ed_f1.trim().to_string();
        let f2 = self.ed_f2.trim().to_string();
        if f1.is_empty() || f2.is_empty() {
            self.ed_msg = Some(("Completa el nombre y el comando antes de guardar".into(), true));
            self.kick();
            return;
        }
        match self.ed_tab {
            0 => {
                let tpl_idx = self.ed_tpl_idx.min(self.cfg.templates.len().saturating_sub(1));
                if let Some(t) = self.cfg.templates.get_mut(tpl_idx) {
                    t.libraries.push(Library {
                        name: f1.clone(),
                        command: f2,
                        default: false,
                    });
                    self.save();
                    if self.wiz_tpl == tpl_idx {
                        self.wiz_libs.push(false);
                    }
                    self.ed_f1.clear();
                    self.ed_f2.clear();
                    self.ed_msg = Some((format!("Librería '{f1}' agregada correctamente"), false));
                }
            }
            1 => {
                let id = actions::safe_slug(&f1.to_lowercase());
                let cmd_str = if f2.contains("{path}") {
                    f2
                } else {
                    format!("\"{}\" \"{{path}}\"", f2.trim_matches('"'))
                };
                self.cfg.launchers.push(Launcher {
                    id,
                    name: f1.clone(),
                    command: cmd_str,
                });
                self.save();
                self.ed_f1.clear();
                self.ed_f2.clear();
                self.ed_msg = Some((format!("Programa '{f1}' agregado a 'Abrir con'"), false));
            }
            _ => {
                let id = actions::safe_slug(&f1.to_lowercase());
                let stack = self.ed_f3.clone();
                let color = STACK_PRESETS
                    .iter()
                    .find(|(s, _)| *s == stack)
                    .map(|(_, c)| *c)
                    .unwrap_or("#3B82F6")
                    .to_string();
                self.cfg.templates.push(Template {
                    id,
                    name: f1.clone(),
                    stack,
                    color,
                    create: vec![f2],
                    setup: Vec::new(),
                    libraries: Vec::new(),
                });
                self.save();
                self.ed_f1.clear();
                self.ed_f2.clear();
                self.ed_msg = Some((format!("Framework '{f1}' guardado en plantillas"), false));
            }
        }
        self.kick();
    }

    fn click(&mut self, h: Hit) {
        match h {
            Hit::Pill => {
                if self.view == View::Status && self.skind == SKind::Error {
                    if let Some(lp) = self.s_log_path.clone() {
                        if lp.exists() {
                            actions::open_log_in_terminal(
                                &self.s_title,
                                self.s_proj_path.as_deref(),
                                &lp,
                            );
                        }
                    }
                    self.set_view(View::Compact);
                } else {
                    self.reload_if_changed();
                    if self.view != View::Status || self.skind != SKind::Creating {
                        self.reopen_preserved_view();
                    }
                }
            }
            Hit::Close => {
                self.resume_view = None;
                self.set_view(View::Compact);
            }
            Hit::Settings => {
                self.startup = actions::startup_enabled();
                self.set_view(View::Settings);
            }
            Hit::Back => self.back(),
            Hit::SearchBox => {}
            Hit::ClearSearch => {
                self.search.clear();
                self.sel_idx = 0;
                self.scroll = 0;
                self.kick();
            }
            Hit::PortBadge(port) | Hit::ProjPort(_, port) => {
                actions::open_browser(&format!("http://localhost:{port}"));
                self.minimize_preserving_state();
            }
            Hit::PortTerm(port) => {
                let matched = self
                    .cfg
                    .projects
                    .iter()
                    .find(|p| self.servers.get(&p.path).and_then(|s| s.port) == Some(port));
                if let Some(p) = matched {
                    let lp = self.servers.get(&p.path).map(|s| s.log_path.as_path());
                    actions::show_server_terminal(Some(&p.name), Some(Path::new(&p.path)), Some(port), lp);
                } else {
                    actions::show_server_terminal(None, None, Some(port), None);
                }
                self.minimize_preserving_state();
            }
            Hit::KillPort(port) => {
                actions::kill_port(port, self.tx.clone(), self.hwnd.0 as isize);
            }
            Hit::Project(i) => self.open_project(i, None),
            Hit::ProjServer(i) => self.toggle_project_server(i),
            Hit::ProjServerTerm(i) => {
                if let Some(p) = self.cfg.projects.get(i) {
                    let srv_port = self.servers.get(&p.path).and_then(|s| s.port);
                    let lp = self.servers.get(&p.path).map(|s| s.log_path.as_path());
                    actions::show_server_terminal(Some(&p.name), Some(Path::new(&p.path)), srv_port, lp);
                    self.minimize_preserving_state();
                }
            }
            Hit::ProjGit(i) => {
                self.git_proj_idx = i;
                if self.git_commit_msg.trim().is_empty() {
                    self.git_commit_msg = "feat: actualización".into();
                }
                self.set_view(View::GitModal);
            }
            Hit::ProjTerm(i) => {
                if let Some(p) = self.cfg.projects.get(i) {
                    actions::open_terminal_at(Path::new(&p.path));
                    self.minimize_preserving_state();
                }
            }
            Hit::ProjExplorer(i) => {
                if let Some(p) = self.cfg.projects.get(i) {
                    actions::open_explorer_at(Path::new(&p.path));
                    self.minimize_preserving_state();
                }
            }
            Hit::ProjPin(i) => self.toggle_pin(i),
            Hit::NewProject => self.start_wizard(),
            Hit::Template(i) => {
                if self.view == View::Wizard {
                    self.wiz_tpl = i;
                    self.reset_libs();
                    self.kick();
                }
            }
            Hit::Library(i) => {
                if let Some(b) = self.wiz_libs.get_mut(i) {
                    *b = !*b;
                }
                self.kick();
            }
            Hit::Launcher(i) => {
                if self.view == View::Wizard {
                    self.wiz_launcher = i;
                    self.kick();
                }
            }
            Hit::WizToggleGithub => {
                self.wiz_github = !self.wiz_github;
                self.kick();
            }
            Hit::WizOpenLibEditor => {
                self.ed_tab = 0;
                self.ed_tpl_idx = self.wiz_tpl;
                self.ed_focus = 0;
                self.ed_scroll = 0;
                self.ed_msg = None;
                self.set_view(View::CustomEditor);
            }
            Hit::WizOpenLauncherEditor => {
                self.ed_tab = 1;
                self.ed_focus = 0;
                self.ed_scroll = 0;
                self.ed_msg = None;
                self.set_view(View::CustomEditor);
            }
            Hit::Next => self.wizard_next(),
            Hit::Input => {}
            Hit::ToggleVisibility => {
                self.set_visible(!self.visible);
            }
            Hit::ToggleFullscreenHide => {
                self.cfg.hide_on_fullscreen = !self.cfg.hide_on_fullscreen;
                self.save();
                self.kick();
            }
            Hit::CycleMonitor => {
                let mons = actions::list_monitors();
                let next = match self.cfg.monitor_mode.as_str() {
                    "primary" => "cursor".to_string(),
                    "cursor" => {
                        if mons.len() > 1 {
                            "monitor:0".to_string()
                        } else {
                            "primary".to_string()
                        }
                    }
                    s if s.starts_with("monitor:") => {
                        let cur = s.trim_start_matches("monitor:").parse::<usize>().unwrap_or(0);
                        if cur + 1 < mons.len() {
                            format!("monitor:{}", cur + 1)
                        } else {
                            "primary".to_string()
                        }
                    }
                    _ => "primary".to_string(),
                };
                self.cfg.monitor_mode = next;
                self.save();
                self.reposition();
                self.kick();
            }
            Hit::Startup => {
                self.startup = !self.startup;
                actions::set_startup(self.startup);
                self.startup = actions::startup_enabled();
                self.kick();
            }
            Hit::DefaultLauncher => {
                let i = (self.cfg.launcher_index(&self.cfg.default_launcher) + 1)
                    % self.cfg.launchers.len().max(1);
                if let Some(l) = self.cfg.launchers.get(i) {
                    self.cfg.default_launcher = l.id.clone();
                }
                self.save();
                self.kick();
            }
            Hit::BaseDir => {
                if let Some(p) = actions::pick_folder(self.hwnd, "Carpeta donde se crean tus proyectos") {
                    self.cfg.base_dir = p.to_string_lossy().to_string();
                    self.scan_and_sync();
                    self.save();
                }
                self.focus();
                self.kick();
            }
            Hit::OpenCustomEditor => {
                self.ed_tab = 0;
                self.ed_focus = 0;
                self.ed_scroll = 0;
                self.ed_msg = None;
                self.set_view(View::CustomEditor);
            }
            Hit::AddExisting => self.add_existing(),
            Hit::EditConfig => {
                actions::open_file(&config::config_path());
                self.minimize_preserving_state();
            }
            Hit::Quit => self.quit(),
            // Git Modal
            Hit::GitCommitInput => {}
            Hit::GitTogglePublic => {
                self.git_public = !self.git_public;
                self.kick();
            }
            Hit::GitDoCommitPush => {
                if let Some(p) = self.cfg.projects.get(self.git_proj_idx).cloned() {
                    let all_paths: Vec<String> = self.cfg.projects.iter().map(|x| x.path.clone()).collect();
                    let msg = self.git_commit_msg.clone();
                    let is_pub = self.git_public;
                    self.s_proj_path = Some(PathBuf::from(&p.path));
                    self.status(
                        SKind::Creating,
                        &format!("Subiendo {} a GitHub…", p.name),
                        "Ejecutando git add + commit + push…",
                        WHITE,
                        "⎇",
                    );
                    actions::git_commit_and_push(
                        PathBuf::from(&p.path),
                        p.name,
                        msg,
                        true,
                        is_pub,
                        all_paths,
                        self.tx.clone(),
                        self.hwnd.0 as isize,
                    );
                }
            }
            Hit::GitDoPull => {
                if let Some(p) = self.cfg.projects.get(self.git_proj_idx).cloned() {
                    self.s_proj_path = Some(PathBuf::from(&p.path));
                    actions::git_pull(
                        PathBuf::from(&p.path),
                        p.name,
                        self.tx.clone(),
                        self.hwnd.0 as isize,
                    );
                }
            }
            Hit::GitOpenRepoWeb => {
                if let Some(p) = self.cfg.projects.get(self.git_proj_idx) {
                    if let Some(url) = self.git_states.get(&p.path).and_then(|g| g.remote_url.clone()) {
                        actions::open_browser(&url);
                        self.minimize_preserving_state();
                    }
                }
            }
            Hit::GitOpenLogTerm => {
                if let Some(p) = self.cfg.projects.get(self.git_proj_idx) {
                    let lp = config::logs_dir().join(format!("git-{}.log", actions::safe_slug(&p.name)));
                    if lp.exists() {
                        actions::open_log_in_terminal(&p.name, Some(Path::new(&p.path)), &lp);
                    } else {
                        actions::open_terminal_at(Path::new(&p.path));
                    }
                    self.minimize_preserving_state();
                }
            }
            // Custom Editor
            Hit::EdTab(t) => {
                self.ed_tab = t;
                self.ed_focus = 0;
                self.ed_scroll = 0;
                self.ed_f1.clear();
                self.ed_f2.clear();
                self.ed_msg = None;
                self.kick();
            }
            Hit::EdField(f) => {
                self.ed_focus = f;
                self.kick();
            }
            Hit::EdPickExe => {
                if let Some(exe) = actions::pick_exe_file(self.hwnd, "Selecciona el archivo ejecutable (.exe) del programa") {
                    if self.ed_f1.trim().is_empty() {
                        if let Some(stem) = exe.file_stem() {
                            self.ed_f1 = stem.to_string_lossy().to_string();
                        }
                    }
                    self.ed_f2 = format!("\"{}\" \"{{path}}\"", exe.display());
                    self.ed_msg = None;
                }
                self.focus();
                self.kick();
            }
            Hit::EdPrevTpl => {
                let len = self.cfg.templates.len().max(1);
                self.ed_tpl_idx = (self.ed_tpl_idx + len - 1) % len;
                self.ed_scroll = 0;
                self.kick();
            }
            Hit::EdNextTpl => {
                let len = self.cfg.templates.len().max(1);
                self.ed_tpl_idx = (self.ed_tpl_idx + 1) % len;
                self.ed_scroll = 0;
                self.kick();
            }
            Hit::EdCycleStack => {
                let cur = STACK_PRESETS
                    .iter()
                    .position(|(s, _)| *s == self.ed_f3)
                    .unwrap_or(0);
                self.ed_f3 = STACK_PRESETS[(cur + 1) % STACK_PRESETS.len()].0.to_string();
                self.kick();
            }
            Hit::EdSave => self.save_custom_editor_item(),
            Hit::EdDeleteItem(idx) => {
                match self.ed_tab {
                    0 => {
                        let tpl_idx = self.ed_tpl_idx.min(self.cfg.templates.len().saturating_sub(1));
                        if let Some(t) = self.cfg.templates.get_mut(tpl_idx) {
                            if idx < t.libraries.len() {
                                t.libraries.remove(idx);
                                self.save();
                                self.reset_libs();
                            }
                        }
                    }
                    1 => {
                        if self.cfg.launchers.len() > 1 && idx < self.cfg.launchers.len() {
                            self.cfg.launchers.remove(idx);
                            self.save();
                        }
                    }
                    _ => {
                        if self.cfg.templates.len() > 1 && idx < self.cfg.templates.len() {
                            self.cfg.templates.remove(idx);
                            self.save();
                        }
                    }
                }
                self.kick();
            }
            Hit::EdToggleLibDefault(idx) => {
                let tpl_idx = self.ed_tpl_idx.min(self.cfg.templates.len().saturating_sub(1));
                if let Some(t) = self.cfg.templates.get_mut(tpl_idx) {
                    if let Some(lib) = t.libraries.get_mut(idx) {
                        lib.default = !lib.default;
                        self.save();
                        self.reset_libs();
                    }
                }
                self.kick();
            }
        }
    }

    fn project_menu(&mut self, i: usize) {
        let Some(p) = self.cfg.projects.get(i).cloned() else { return };
        let current = p.launcher.clone().unwrap_or(self.cfg.default_launcher.clone());
        let scripts = actions::detect_package_scripts(Path::new(&p.path));
        let is_srv_running = self.servers.contains_key(&p.path);
        let srv_port = self.servers.get(&p.path).and_then(|s| s.port);
        let srv_log = self.servers.get(&p.path).map(|s| s.log_path.clone());

        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            for (k, l) in self.cfg.launchers.iter().enumerate() {
                let flags = if l.id == current {
                    MF_STRING | MF_CHECKED
                } else {
                    MF_STRING
                };
                let _ = AppendMenuW(menu, flags, 100 + k, &HSTRING::from(format!("Abrir con {}", l.name)));
            }

            if is_srv_running {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(menu, MF_STRING, 302, w!(">_ Ver terminal del servidor en vivo"));
                let _ = AppendMenuW(menu, MF_STRING, 300, w!("■ Detener servidor local"));
            }

            if !scripts.is_empty() {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                for (idx, (s_name, _)) in scripts.iter().take(8).enumerate() {
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING,
                        400 + idx,
                        &HSTRING::from(format!("⚡ npm run {s_name}")),
                    );
                }
            }

            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(menu, MF_STRING, 201, w!("🗑 Quitar proyecto de la lista"));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            self.focus();
            let cmd = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
                pt.x,
                pt.y,
                0,
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(menu);
            let cmd = cmd.0 as usize;

            if (100..200).contains(&cmd) {
                if let Some(l) = self.cfg.launchers.get(cmd - 100) {
                    let id = l.id.clone();
                    self.open_project(i, Some(id));
                }
            } else if cmd == 201 {
                self.cfg.projects.remove(i);
                self.scroll = 0;
                self.save();
                self.kick();
            } else if cmd == 300 {
                self.toggle_project_server(i);
            } else if cmd == 302 {
                actions::show_server_terminal(
                    Some(&p.name),
                    Some(Path::new(&p.path)),
                    srv_port,
                    srv_log.as_deref(),
                );
                self.minimize_preserving_state();
            } else if (400..415).contains(&cmd) {
                if let Some((s_name, _)) = scripts.get(cmd - 400) {
                    if s_name == "dev" && !is_srv_running {
                        self.toggle_project_server(i);
                    } else {
                        actions::run_npm_script_in_terminal(Path::new(&p.path), s_name);
                    }
                }
            }
        }
    }

    fn tray_menu(&mut self) {
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            let _ = AppendMenuW(menu, MF_STRING, 1, w!("Abrir proyectos"));
            let _ = AppendMenuW(menu, MF_STRING, 2, w!("Nuevo proyecto"));
            let _ = AppendMenuW(menu, MF_STRING, 6, w!("Personalizar (Programas y Librerías)"));
            let _ = AppendMenuW(menu, MF_STRING, 3, w!("Ajustes"));
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                4,
                if self.visible {
                    w!("Ocultar isla")
                } else {
                    w!("Mostrar isla")
                },
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(menu, MF_STRING, 5, w!("Salir"));
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
                pt.x,
                pt.y,
                0,
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(menu);
            match cmd.0 {
                1 => {
                    self.set_visible(true);
                    self.set_view(View::Expanded)
                }
                2 => {
                    self.set_visible(true);
                    self.start_wizard()
                }
                6 => {
                    self.set_visible(true);
                    self.ed_tab = 0;
                    self.set_view(View::CustomEditor)
                }
                3 => {
                    self.set_visible(true);
                    self.startup = actions::startup_enabled();
                    self.set_view(View::Settings)
                }
                4 => self.set_visible(!self.visible),
                5 => self.quit(),
                _ => {}
            }
        }
    }

    fn toggle(&mut self) {
        if !self.visible {
            self.set_visible(true);
        }
        if self.view == View::Compact || self.view == View::Status {
            self.reload_if_changed();
            self.reopen_preserved_view();
            self.focus();
        } else {
            self.minimize_preserving_state();
        }
    }

    fn key(&mut self, vk: u32) -> Option<isize> {
        let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
        match vk {
            x if x == VK_ESCAPE.0 as u32 => self.back(),
            x if x == VK_TAB.0 as u32 && self.view == View::CustomEditor => {
                self.ed_focus = (self.ed_focus + 1) % 2;
                self.kick();
            }
            x if x == VK_RETURN.0 as u32 => match self.view {
                View::Wizard => self.wizard_next(),
                View::GitModal => self.click(Hit::GitDoCommitPush),
                View::CustomEditor => self.save_custom_editor_item(),
                View::Expanded => {
                    let filtered = self.filtered_projects();
                    if let Some(&proj_idx) = filtered.get(self.sel_idx.min(filtered.len().saturating_sub(1))) {
                        self.open_project(proj_idx, None);
                    }
                }
                _ => {}
            },
            x if x == VK_DOWN.0 as u32 && self.view == View::Expanded => {
                let len = self.filtered_projects().len();
                if len > 0 {
                    self.sel_idx = (self.sel_idx + 1).min(len - 1);
                    if self.sel_idx >= self.scroll + MAX_ROWS {
                        self.scroll = self.sel_idx + 1 - MAX_ROWS;
                    }
                    self.kick();
                }
            }
            x if x == VK_UP.0 as u32 && self.view == View::Expanded => {
                self.sel_idx = self.sel_idx.saturating_sub(1);
                if self.sel_idx < self.scroll {
                    self.scroll = self.sel_idx;
                }
                self.kick();
            }
            0x31..=0x39 if ctrl && self.view == View::Expanded => {
                let filtered = self.filtered_projects();
                if let Some(&proj_idx) = filtered.get((vk - 0x31) as usize) {
                    self.open_project(proj_idx, None);
                }
            }
            0x4E if ctrl && self.view == View::Expanded => self.start_wizard(),
            _ => return None,
        }
        Some(0)
    }

    fn chr(&mut self, c: u32) -> Option<isize> {
        let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
        if ctrl {
            return None;
        }
        match self.view {
            View::Expanded => match c {
                8 => {
                    self.search.pop();
                    self.sel_idx = 0;
                    self.scroll = 0;
                    self.kick();
                    return Some(0);
                }
                13 | 27 | 9 => return Some(0),
                _ => {
                    let Some(ch) = char::from_u32(c) else { return Some(0) };
                    if !ch.is_control() && self.search.chars().count() < 46 {
                        self.search.push(ch);
                        self.sel_idx = 0;
                        self.scroll = 0;
                        self.kick();
                    }
                    return Some(0);
                }
            },
            View::GitModal => match c {
                8 => {
                    self.git_commit_msg.pop();
                    self.kick();
                    return Some(0);
                }
                13 | 27 | 9 => return Some(0),
                _ => {
                    let Some(ch) = char::from_u32(c) else { return Some(0) };
                    if !ch.is_control() && self.git_commit_msg.chars().count() < 80 {
                        self.git_commit_msg.push(ch);
                        self.kick();
                    }
                    return Some(0);
                }
            },
            View::CustomEditor => {
                let target = if self.ed_focus == 0 {
                    &mut self.ed_f1
                } else {
                    &mut self.ed_f2
                };
                match c {
                    8 => {
                        target.pop();
                        self.ed_msg = None;
                        self.kick();
                        return Some(0);
                    }
                    13 | 27 | 9 => return Some(0),
                    _ => {
                        let Some(ch) = char::from_u32(c) else { return Some(0) };
                        if !ch.is_control() && target.chars().count() < 95 {
                            target.push(ch);
                            self.ed_msg = None;
                            self.kick();
                        }
                        return Some(0);
                    }
                }
            }
            View::Wizard if self.step == Step::Name => {
                match c {
                    8 => {
                        self.wiz_name.pop();
                    }
                    13 | 27 | 9 => return Some(0),
                    _ => {
                        let Some(ch) = char::from_u32(c) else { return Some(0) };
                        if ch.is_control()
                            || "\\/:*?\"<>|".contains(ch)
                            || self.wiz_name.chars().count() >= 60
                        {
                            return Some(0);
                        }
                        self.wiz_name.push(if ch == ' ' { '-' } else { ch });
                    }
                }
                self.wiz_err = None;
                self.kick();
                Some(0)
            }
            _ => None,
        }
    }

    fn mpos(&self, lp: LPARAM) -> (f32, f32) {
        let x = (lp.0 & 0xFFFF) as u16 as i16 as f32;
        let y = ((lp.0 >> 16) & 0xFFFF) as u16 as i16 as f32;
        (x / self.gfx.scale, y / self.gfx.scale)
    }

    pub fn handle(&mut self, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<isize> {
        unsafe {
            match msg {
                WM_TIMER => {
                    if wp.0 == TIMER_ANIM {
                        self.tick();
                    } else if wp.0 == TIMER_POLL {
                        self.poll_periodic();
                    }
                    Some(0)
                }
                WM_DROPFILES => {
                    let hdrop = HDROP(wp.0 as _);
                    let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
                    for i in 0..count {
                        let len = DragQueryFileW(hdrop, i, None);
                        if len > 0 {
                            let mut buf = vec![0u16; (len + 1) as usize];
                            DragQueryFileW(hdrop, i, Some(&mut buf));
                            if let Some(pos) = buf.iter().position(|&c| c == 0) {
                                buf.truncate(pos);
                            }
                            let dropped = PathBuf::from(String::from_utf16_lossy(&buf));
                            let folder = if dropped.is_dir() {
                                Some(dropped)
                            } else {
                                dropped.parent().map(|p| p.to_path_buf())
                            };
                            if let Some(dir) = folder {
                                self.register_and_open_folder(dir, true);
                            }
                        }
                    }
                    DragFinish(hdrop);
                    Some(0)
                }
                WM_MOUSEMOVE => {
                    if !self.tracking {
                        let mut t = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: self.hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut t);
                        self.tracking = true;
                    }
                    let (x, y) = self.mpos(lp);
                    let h = self.hit_at(x, y);
                    if h != self.hover {
                        self.hover = h;
                        self.kick();
                    }
                    Some(0)
                }
                WM_MOUSELEAVE => {
                    self.tracking = false;
                    if self.hover.is_some() {
                        self.hover = None;
                        self.kick();
                    }
                    Some(0)
                }
                WM_LBUTTONUP => {
                    let (x, y) = self.mpos(lp);
                    if let Some(h) = self.hit_at(x, y) {
                        self.click(h);
                    }
                    Some(0)
                }
                WM_RBUTTONUP => {
                    let (x, y) = self.mpos(lp);
                    match self.hit_at(x, y) {
                        Some(Hit::Project(i))
                        | Some(Hit::ProjServer(i))
                        | Some(Hit::ProjServerTerm(i))
                        | Some(Hit::ProjPort(i, _))
                        | Some(Hit::ProjGit(i))
                        | Some(Hit::ProjTerm(i))
                        | Some(Hit::ProjExplorer(i))
                        | Some(Hit::ProjPin(i)) => self.project_menu(i),
                        Some(Hit::PortBadge(port)) => {
                            actions::kill_port(port, self.tx.clone(), self.hwnd.0 as isize);
                        }
                        Some(Hit::Pill) => self.tray_menu(),
                        _ => {}
                    }
                    Some(0)
                }
                WM_MOUSEWHEEL => {
                    let d = ((wp.0 >> 16) & 0xFFFF) as u16 as i16;
                    match self.view {
                        View::Expanded => {
                            let max = self.filtered_projects().len().saturating_sub(MAX_ROWS);
                            self.scroll = if d < 0 {
                                (self.scroll + 1).min(max)
                            } else {
                                self.scroll.saturating_sub(1)
                            };
                            self.kick();
                        }
                        View::Wizard if self.step == Step::Name => {
                            let max = self.cfg.templates.len().saturating_sub(5);
                            self.wiz_tpl_scroll = if d < 0 {
                                (self.wiz_tpl_scroll + 1).min(max)
                            } else {
                                self.wiz_tpl_scroll.saturating_sub(1)
                            };
                            self.kick();
                        }
                        View::Wizard if self.step == Step::Libraries => {
                            let max = self.cur_libs_len().saturating_sub(6);
                            self.wiz_lib_scroll = if d < 0 {
                                (self.wiz_lib_scroll + 1).min(max)
                            } else {
                                self.wiz_lib_scroll.saturating_sub(1)
                            };
                            self.kick();
                        }
                        View::CustomEditor => {
                            self.ed_scroll = if d < 0 {
                                self.ed_scroll + 1
                            } else {
                                self.ed_scroll.saturating_sub(1)
                            };
                            self.kick();
                        }
                        _ => {}
                    }
                    Some(0)
                }
                WM_SETCURSOR => {
                    if self.hover.is_some() {
                        let _ = SetCursor(LoadCursorW(None, IDC_HAND).unwrap_or_default());
                        return Some(1);
                    }
                    None
                }
                WM_KEYDOWN => self.key(wp.0 as u32),
                WM_CHAR => self.chr(wp.0 as u32),
                WM_ACTIVATE => {
                    if (wp.0 & 0xFFFF) as u32 == WA_INACTIVE
                        && matches!(
                            self.view,
                            View::Expanded
                                | View::Settings
                                | View::Wizard
                                | View::GitModal
                                | View::CustomEditor
                        )
                    {
                        self.minimize_preserving_state();
                    }
                    let _ = SetWindowPos(
                        self.hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    None
                }
                WM_WINDOWPOSCHANGING => {
                    let ppos = lp.0 as *mut WINDOWPOS;
                    if !ppos.is_null() {
                        (*ppos).hwndInsertAfter = HWND_TOPMOST;
                    }
                    None
                }
                WM_HOTKEY => {
                    self.toggle();
                    Some(0)
                }
                WM_TRAY => {
                    match (lp.0 as u32) & 0xFFFF {
                        WM_LBUTTONUP => self.toggle(),
                        WM_RBUTTONUP => self.tray_menu(),
                        _ => {}
                    }
                    Some(0)
                }
                WM_WORKER => {
                    self.on_worker();
                    Some(0)
                }
                WM_DPICHANGED => {
                    let dpi = (wp.0 & 0xFFFF) as f32;
                    let _ = self.gfx.resize(dpi / 96.0, WIN_W, WIN_H);
                    self.reposition();
                    self.kick();
                    Some(0)
                }
                WM_DISPLAYCHANGE => {
                    self.reposition();
                    self.kick();
                    None
                }
                _ => None,
            }
        }
    }
}
