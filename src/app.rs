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

use crate::actions::{self, FullStackInfo, GitInfo, OpenProjectWindow, WorkMsg, WM_WORKER};
use crate::config::{self, hex, Config, Launcher, Library, Project, Template};
use crate::gfx::Gfx;

pub const WIN_W: f32 = 820.0;
pub const WIN_H: f32 = 760.0;
pub(crate) const TOP: f32 = 0.0;
pub const WM_TRAY: u32 = WM_APP + 2;
const WM_MOUSELEAVE: u32 = 0x02A3;
const TIMER_ANIM: usize = 1;
const TIMER_POLL: usize = 2;
const HOTKEY_ID: i32 = 1;
pub(crate) const MAX_ROWS: usize = 6;

// Paleta Black & White minimalista (Apple / Vercel / Linear style) + indicadores semánticos
pub(crate) const WHITE: u32 = 0xFFFFFF;
pub(crate) const GRAY: u32 = 0xA1A1AA;
pub(crate) const DIM: u32 = 0x52525B;
pub(crate) const ACCENT: u32 = 0xFFFFFF;
pub(crate) const BODY: u32 = 0x000000;
pub(crate) const CARD: u32 = 0x121214;
pub(crate) const CARD_HOV: u32 = 0x1C1C20;
pub(crate) const BORDER: u32 = 0x27272A;
pub(crate) const RED: u32 = 0xEF4444;
pub(crate) const GREEN: u32 = 0x22C55E;
pub(crate) const AMBER: u32 = 0xF59E0B;
pub(crate) const CYAN: u32 = 0x38BDF8;

// Glifos Segoe Fluent Icons / MDL2
pub(crate) const G_CHEV: &str = "\u{E76C}";
pub(crate) const G_BACK: &str = "\u{E72B}";
pub(crate) const G_ADD: &str = "\u{E710}";
pub(crate) const G_GEAR: &str = "\u{E713}";
pub(crate) const G_CLOSE: &str = "\u{E711}";
pub(crate) const G_FOLDER: &str = "\u{E8B7}";
pub(crate) const G_CHECK: &str = "\u{E73E}";
pub(crate) const G_POWER: &str = "\u{E7E8}";
pub(crate) const G_EDIT: &str = "\u{E70F}";
pub(crate) const G_TERM: &str = "\u{E756}";
pub(crate) const G_ERR: &str = "\u{E783}";
pub(crate) const G_EXIT: &str = "\u{E8BB}";
pub(crate) const G_SEARCH: &str = "\u{E721}";
pub(crate) const G_PLAY: &str = "\u{E768}";
pub(crate) const G_STOP: &str = "\u{E71A}";
pub(crate) const G_PIN: &str = "\u{E718}";
pub(crate) const G_TRASH: &str = "\u{E74D}";
pub(crate) const G_UPLOAD: &str = "\u{E898}";
pub(crate) const G_DOWNLOAD: &str = "\u{E896}";
pub(crate) const G_GLOBE: &str = "\u{E774}";
pub(crate) const G_SYNC: &str = "\u{E895}";
pub(crate) const G_LINK: &str = "\u{E71B}";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterMode {
    All,
    Pinned,
    GitDirty,
    RunningServer,
    FullStack,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ServerRole {
    Main,
    Front,
    Back,
    Both,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    Pill,
    Close,
    Settings,
    Back,
    SearchBox,
    ClearSearch,
    FilterChip(FilterMode),
    CloneClipboardRepo,
    QuickOpenBaseDir,
    QuickSyncProjects,
    PortBadge(u16),
    PortTerm(u16),
    KillPort(u16),
    Project(usize),
    ProjServer(usize),
    ProjServerRole(usize, ServerRole),
    ProjServerTerm(usize),
    ProjPort(usize, u16),
    ProjGit(usize),
    ProjNote(usize),
    ProjTerm(usize),
    ProjExplorer(usize),
    ProjPin(usize),
    OpenWinFocus(usize),
    OpenWinClose(usize),
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
    // Note Modal (Mini-TODO por proyecto)
    NoteInput,
    NoteSave,
    NoteClear,
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum View {
    Compact,
    Expanded,
    Settings,
    Wizard,
    GitModal,
    NoteModal,
    CustomEditor,
    Status,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Step {
    Name,
    Libraries,
    Launcher,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum SKind {
    Opening,
    Done,
    Creating,
    Error,
}

#[derive(Clone, Copy)]
pub(crate) struct Spring {
    pub(crate) x: f32,
    pub(crate) v: f32,
    pub(crate) t: f32,
}

impl Spring {
    pub(crate) fn new(x: f32) -> Self {
        Self { x, v: 0.0, t: x }
    }
    pub(crate) fn step(&mut self, dt: f32, k: f32, d: f32, eps: f32) -> bool {
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

pub(crate) struct Pending {
    pub(crate) name: String,
    pub(crate) stack: String,
    pub(crate) color: String,
    pub(crate) launcher: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RunningServer {
    pub(crate) pid: u32,
    pub(crate) port: Option<u16>,
    pub(crate) log_path: PathBuf,
    pub(crate) label: String,
    pub(crate) project_name: String,
}

pub struct App {
    pub(crate) hwnd: HWND,
    pub(crate) gfx: Gfx,
    pub(crate) cfg: Config,
    pub(crate) cfg_mtime: Option<SystemTime>,
    pub(crate) view: View,
    pub(crate) resume_view: Option<View>,
    pub(crate) hover: Option<Hit>,
    pub(crate) hits: RefCell<Vec<(f32, f32, f32, f32, Hit)>>,
    pub(crate) sw: Spring,
    pub(crate) sh: Spring,
    pub(crate) sr: Spring,
    pub(crate) sa: Spring,
    pub(crate) last: Instant,
    pub(crate) timer: bool,
    pub(crate) tracking: bool,
    pub(crate) spin: f32,
    pub(crate) glow_anim: f32,
    pub(crate) scroll: usize,
    pub(crate) pos: (i32, i32),
    pub(crate) visible: bool,
    pub(crate) auto_hidden_fs: bool,
    pub(crate) startup: bool,
    // buscador instantáneo ("Spotlight") y filtros rápidos por chips
    pub(crate) search: String,
    pub(crate) filter_mode: FilterMode,
    pub(crate) sel_idx: usize,
    // portapapeles GitHub ("1-Click Clone") y progreso en el borde de la gota
    pub(crate) clipboard_repo: Option<(String, String)>,
    pub(crate) bg_progress: Option<f32>,
    // estado de Git, servidores, combos Full-Stack y ventanas abiertas en tiempo real
    pub(crate) git_states: HashMap<String, GitInfo>,
    pub(crate) fullstack_map: HashMap<String, FullStackInfo>,
    pub(crate) servers: HashMap<String, RunningServer>,
    pub(crate) active_ports: Vec<u16>,
    pub(crate) open_windows: Vec<OpenProjectWindow>,
    pub(crate) open_scroll: usize,
    // asistente de creación
    pub(crate) step: Step,
    pub(crate) wiz_name: String,
    pub(crate) wiz_tpl: usize,
    pub(crate) wiz_tpl_scroll: usize,
    pub(crate) wiz_libs: Vec<bool>,
    pub(crate) wiz_lib_scroll: usize,
    pub(crate) wiz_launcher: usize,
    pub(crate) wiz_github: bool,
    pub(crate) wiz_err: Option<String>,
    // modal visual de Git & GitHub
    pub(crate) git_proj_idx: usize,
    pub(crate) git_commit_msg: String,
    pub(crate) git_public: bool,
    // modal de nota / recordatorio rápido (Mini-TODO)
    pub(crate) note_proj_idx: usize,
    pub(crate) note_text: String,
    // editor visual de Librerías, Programas y Frameworks
    pub(crate) ed_tab: usize, // 0 = Librerías, 1 = Programas, 2 = Frameworks
    pub(crate) ed_focus: usize,
    pub(crate) ed_tpl_idx: usize,
    pub(crate) ed_f1: String,
    pub(crate) ed_f2: String,
    pub(crate) ed_f3: String,
    pub(crate) ed_scroll: usize,
    pub(crate) ed_msg: Option<(String, bool)>,
    // estado / notificaciones
    pub(crate) skind: SKind,
    pub(crate) s_title: String,
    pub(crate) s_sub: String,
    pub(crate) s_color: u32,
    pub(crate) s_initial: String,
    pub(crate) s_next: String,
    pub(crate) s_deadline: Option<Instant>,
    pub(crate) s_log_path: Option<PathBuf>,
    pub(crate) s_proj_path: Option<PathBuf>,
    pub(crate) pending: Option<Pending>,
    pub(crate) tx: Sender<WorkMsg>,
    pub(crate) rx: Receiver<WorkMsg>,
    pub(crate) tray_icon: HICON,
}

pub(crate) fn initial(s: &str) -> String {
    s.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or("·".into())
}

pub(crate) const STACK_PRESETS: &[(&str, &str)] = &[
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
            filter_mode: FilterMode::All,
            sel_idx: 0,
            clipboard_repo: None,
            bg_progress: None,
            git_states: HashMap::new(),
            fullstack_map: HashMap::new(),
            servers: HashMap::new(),
            active_ports: Vec::new(),
            open_windows: Vec::new(),
            open_scroll: 0,
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
            note_proj_idx: 0,
            note_text: String::new(),
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

    pub(crate) fn scan_and_sync(&mut self) {
        let scanned = actions::scan_base_dir(Path::new(&self.cfg.base_dir));
        if self.cfg.sync_scanned_projects(scanned) {
            self.save();
        }
        // Limpiar estados de proyectos que ya no existan
        let valid_paths: std::collections::HashSet<String> =
            self.cfg.projects.iter().map(|p| p.path.to_lowercase()).collect();
        self.git_states
            .retain(|k, _| valid_paths.contains(&k.to_lowercase()));
        self.fullstack_map.clear();

        for p in &self.cfg.projects {
            let fs_info = actions::detect_project_targets(
                Path::new(&p.path),
                &p.stack,
                p.linked_path.as_deref(),
            );
            self.fullstack_map.insert(p.path.clone(), fs_info);

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

    pub(crate) fn refresh_live_state(&mut self) {
        self.scan_and_sync();
        self.clipboard_repo = actions::read_clipboard_github_repo(self.hwnd);
        let paths: Vec<String> = self.cfg.projects.iter().map(|p| p.path.clone()).collect();
        let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
        let hw = self.hwnd.0 as isize;
        self.open_windows = actions::scan_open_project_windows_sync(&self.cfg.projects, self.hwnd);
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

        // Mantener la isla siempre visible por encima de la barra de tareas y bandeja del sistema
        if self.visible && !self.auto_hidden_fs {
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }

        // 2. Si el panel está abierto o en hover, sincronizar carpetas borradas y ventanas abiertas
        if self.view == View::Expanded || (self.view == View::Compact && self.hov(Hit::Pill)) {
            let scanned = actions::scan_base_dir(Path::new(&self.cfg.base_dir));
            if self.cfg.sync_scanned_projects(scanned) {
                self.save();
                self.kick();
            }
            let new_open = actions::scan_open_project_windows_sync(&self.cfg.projects, self.hwnd);
            if new_open != self.open_windows {
                self.open_windows = new_open;
                self.kick();
            }
        }

        // 3. Escanear puertos activos
        let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
        actions::scan_open_ports(extra_ports, self.tx.clone(), self.hwnd.0 as isize);
    }

    pub(crate) fn active_server_count(&self) -> usize {
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

    pub(crate) fn has_live_server(&self) -> bool {
        self.active_server_count() > 0
    }

    pub(crate) fn primary_live_port(&self) -> Option<u16> {
        self.servers
            .values()
            .find_map(|s| s.port)
            .or_else(|| self.active_ports.first().copied())
    }

    pub(crate) fn project_has_running_server(&self, proj_idx: usize) -> bool {
        let Some(p) = self.cfg.projects.get(proj_idx) else {
            return false;
        };
        if self.servers.contains_key(&p.path) {
            return true;
        }
        if let Some(fs) = self.fullstack_map.get(&p.path) {
            if let Some(f) = &fs.front {
                if self.servers.contains_key(&f.cwd.to_string_lossy().to_string()) {
                    return true;
                }
            }
            if let Some(b) = &fs.back {
                if self.servers.contains_key(&b.cwd.to_string_lossy().to_string()) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn waterdrop_dot_point(cx: f32, w: f32, h: f32, u: f32, dot_r: f32) -> (f32, f32) {
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

    pub(crate) fn waterdrop_dot_positions(cx: f32, w: f32, h: f32, count: usize, dot_r: f32) -> Vec<(f32, f32)> {
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

    pub(crate) fn filtered_projects(&self) -> Vec<usize> {
        let q = self.search.trim().to_lowercase();
        self.cfg
            .projects
            .iter()
            .enumerate()
            .filter(|(idx, p)| {
                let mode_ok = match self.filter_mode {
                    FilterMode::All => true,
                    FilterMode::Pinned => p.pinned,
                    FilterMode::GitDirty => self
                        .git_states
                        .get(&p.path)
                        .map(|g| g.dirty)
                        .unwrap_or(false),
                    FilterMode::RunningServer => self.project_has_running_server(*idx),
                    FilterMode::FullStack => self
                        .fullstack_map
                        .get(&p.path)
                        .map(|fs| fs.is_fullstack())
                        .unwrap_or(false),
                };
                if !mode_ok {
                    return false;
                }
                if q.is_empty() {
                    return true;
                }
                let branch = self
                    .git_states
                    .get(&p.path)
                    .map(|g| g.branch.to_lowercase())
                    .unwrap_or_default();
                let note = p
                    .note
                    .as_deref()
                    .map(|n| n.to_lowercase())
                    .unwrap_or_default();
                p.name.to_lowercase().contains(&q)
                    || p.stack.to_lowercase().contains(&q)
                    || branch.contains(&q)
                    || note.contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    // ---------- ventana / sistema ----------

    pub(crate) fn reposition(&mut self) {
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

    pub(crate) fn quit(&mut self) {
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

    pub(crate) fn focus(&self) {
        unsafe {
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(self.hwnd);
        }
    }

    pub(crate) fn set_visible(&mut self, v: bool) {
        self.visible = v;
        self.auto_hidden_fs = false;
        unsafe {
            let _ = ShowWindow(self.hwnd, if v { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
        if v {
            self.kick();
        }
    }

    pub(crate) fn reload_if_changed(&mut self) {
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

    pub(crate) fn save(&mut self) {
        config::save(&self.cfg);
        self.cfg_mtime = config::mtime();
    }

    // ---------- animación ----------

    pub(crate) fn kick(&mut self) {
        if !self.timer {
            self.last = Instant::now();
            unsafe {
                SetTimer(self.hwnd, TIMER_ANIM, 8, None);
            }
            self.timer = true;
        }
        self.tick();
    }

    pub(crate) fn target(&self) -> (f32, f32, f32) {
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
                let n = filtered_len.clamp(3, MAX_ROWS) as f32;
                let ports_bar = if self.active_ports.is_empty() { 0.0 } else { 32.0 };
                let search_bar = 42.0;
                let chips_bar = 30.0;
                (796.0, 64.0 + ports_bar + search_bar + chips_bar + n * 54.0 + 54.0 + 18.0, 26.0)
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
            View::NoteModal => (520.0, 252.0, 26.0),
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

    pub(crate) fn fade(&mut self) {
        self.sa.x = 0.0;
        self.sa.v = 0.0;
    }

    /// Cambia la vista activa sin destruir el progreso de creación, búsqueda o formularios.
    pub(crate) fn set_view(&mut self, v: View) {
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
            View::Expanded
                | View::Settings
                | View::Wizard
                | View::GitModal
                | View::NoteModal
                | View::CustomEditor
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
    pub(crate) fn minimize_preserving_state(&mut self) {
        if matches!(
            self.view,
            View::Expanded
                | View::Settings
                | View::Wizard
                | View::GitModal
                | View::NoteModal
                | View::CustomEditor
        ) {
            self.resume_view = Some(self.view);
        }
        self.set_view(View::Compact);
    }

    /// Restaura la última vista en la que estaba trabajando el usuario (o `View::Expanded` por defecto).
    pub(crate) fn reopen_preserved_view(&mut self) {
        let target = self.resume_view.take().unwrap_or(View::Expanded);
        self.set_view(target);
    }

    // ---------- hit-testing y render ----------

    pub(crate) fn hit(&self, x: f32, y: f32, w: f32, h: f32, id: Hit) {
        self.hits.borrow_mut().push((x, y, w, h, id));
    }

    pub(crate) fn hov(&self, id: Hit) -> bool {
        self.hover == Some(id)
    }

    pub(crate) fn is_row_hov(&self, i: usize) -> bool {
        matches!(
            self.hover,
            Some(Hit::Project(k))
                | Some(Hit::ProjServer(k))
                | Some(Hit::ProjServerRole(k, _))
                | Some(Hit::ProjServerTerm(k))
                | Some(Hit::ProjPort(k, _))
                | Some(Hit::ProjGit(k))
                | Some(Hit::ProjNote(k))
                | Some(Hit::ProjTerm(k))
                | Some(Hit::ProjExplorer(k))
                | Some(Hit::ProjPin(k))
                if k == i
        )
    }

    pub(crate) fn hit_at(&self, x: f32, y: f32) -> Option<Hit> {
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

    pub(crate) fn cur_libs_len(&self) -> usize {
        self.cfg
            .templates
            .get(self.wiz_tpl)
            .map(|t| t.libraries.len())
            .unwrap_or(0)
    }

    // ---------- lógica ----------

    pub(crate) fn status(&mut self, kind: SKind, title: &str, sub: &str, color: u32, init: &str) {
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

    pub(crate) fn error(&mut self, title: &str, sub: &str, log_path: Option<PathBuf>) {
        self.s_log_path = log_path;
        self.status(SKind::Error, title, sub, RED, "!");
        self.s_deadline = Some(Instant::now() + Duration::from_secs(9));
    }

    fn start_single_target(&mut self, proj_name: &str, target: &actions::DevTarget) {
        let key = target.cwd.to_string_lossy().to_string();
        if self.servers.contains_key(&key) {
            return;
        }
        let mut already_used = self.active_ports.clone();
        for s in self.servers.values() {
            if let Some(pt) = s.port {
                if !already_used.contains(&pt) {
                    already_used.push(pt);
                }
            }
        }
        let display_name = if target.label == "Main" {
            proj_name.to_string()
        } else {
            format!("{proj_name} ({})", target.label)
        };
        match actions::start_dev_server(
            display_name.clone(),
            target.cwd.clone(),
            target.cmd.clone(),
            target.port,
            already_used,
            self.tx.clone(),
            self.hwnd.0 as isize,
        ) {
            Ok((pid, log_path)) => {
                self.servers.insert(
                    key,
                    RunningServer {
                        pid,
                        port: None,
                        log_path,
                        label: target.label.clone(),
                        project_name: proj_name.to_string(),
                    },
                );
                self.kick();
            }
            Err(e) => self.error(&format!("No se pudo iniciar servidor en {display_name}"), &e, None),
        }
    }

    fn stop_single_target(&mut self, cwd: &Path) -> bool {
        let key = cwd.to_string_lossy().to_string();
        if let Some(srv) = self.servers.remove(&key) {
            actions::stop_dev_server(srv.pid);
            let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
            actions::scan_open_ports(extra_ports, self.tx.clone(), self.hwnd.0 as isize);
            self.kick();
            true
        } else {
            false
        }
    }

    pub(crate) fn toggle_project_server(&mut self, i: usize) {
        self.toggle_project_server_role(i, ServerRole::Main);
    }

    pub(crate) fn toggle_project_server_role(&mut self, i: usize, role: ServerRole) {
        let Some(p) = self.cfg.projects.get(i).cloned() else { return };
        let fs_info = self
            .fullstack_map
            .get(&p.path)
            .cloned()
            .unwrap_or_else(|| actions::detect_project_targets(Path::new(&p.path), &p.stack, p.linked_path.as_deref()));

        match role {
            ServerRole::Main => {
                if let Some(t) = &fs_info.main {
                    if !self.stop_single_target(&t.cwd) {
                        self.start_single_target(&p.name, t);
                    }
                }
            }
            ServerRole::Front => {
                if let Some(t) = &fs_info.front {
                    if !self.stop_single_target(&t.cwd) {
                        self.start_single_target(&p.name, t);
                    }
                }
            }
            ServerRole::Back => {
                if let Some(t) = &fs_info.back {
                    if !self.stop_single_target(&t.cwd) {
                        self.start_single_target(&p.name, t);
                    }
                }
            }
            ServerRole::Both => {
                let f_running = fs_info
                    .front
                    .as_ref()
                    .map(|t| self.servers.contains_key(&t.cwd.to_string_lossy().to_string()))
                    .unwrap_or(false);
                let b_running = fs_info
                    .back
                    .as_ref()
                    .map(|t| self.servers.contains_key(&t.cwd.to_string_lossy().to_string()))
                    .unwrap_or(false);
                if f_running && b_running {
                    if let Some(f) = &fs_info.front {
                        self.stop_single_target(&f.cwd);
                    }
                    if let Some(b) = &fs_info.back {
                        self.stop_single_target(&b.cwd);
                    }
                } else {
                    if let Some(f) = &fs_info.front {
                        self.start_single_target(&p.name, f);
                    }
                    if let Some(b) = &fs_info.back {
                        self.start_single_target(&p.name, b);
                    }
                }
            }
        }
    }

    pub(crate) fn toggle_pin(&mut self, i: usize) {
        if let Some(p) = self.cfg.projects.get_mut(i) {
            p.pinned = !p.pinned;
        }
        self.cfg.sort_projects();
        self.save();
        self.kick();
    }

    pub(crate) fn open_project(&mut self, i: usize, launcher: Option<String>) {
        let Some(p) = self.cfg.projects.get(i).cloned() else { return };
        let lid = launcher
            .clone()
            .or(p.launcher.clone())
            .unwrap_or(self.cfg.default_launcher.clone());

        // Si el proyecto ya está abierto en un editor/programa compatible, saltar a esa ventana en vez de abrir otra duplicada
        if let Some(prog_name) = actions::focus_existing_project_window(&self.cfg.projects, i, &lid, self.hwnd) {
            self.s_log_path = None;
            self.status(
                SKind::Done,
                &p.name,
                &format!("Ya abierto · Saltando a {prog_name}"),
                WHITE,
                "✓",
            );
            self.s_deadline = Some(Instant::now() + Duration::from_millis(1100));
            if let Some(pp) = self.cfg.projects.get_mut(i) {
                pp.last_opened = config::now();
                if launcher.is_some() {
                    pp.launcher = launcher;
                }
            }
            self.cfg.sort_projects();
            self.scroll = 0;
            self.save();
            return;
        }

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

    pub(crate) fn clone_from_clipboard(&mut self) {
        let Some((repo_name, url)) = self
            .clipboard_repo
            .clone()
            .or_else(|| actions::read_clipboard_github_repo(self.hwnd))
        else {
            return;
        };
        let base = PathBuf::from(&self.cfg.base_dir);
        let dest = base.join(&repo_name);
        if dest.exists() {
            if let Some(idx) = self
                .cfg
                .projects
                .iter()
                .position(|p| p.path.eq_ignore_ascii_case(&dest.to_string_lossy()))
            {
                self.open_project(idx, None);
            } else {
                self.register_and_open_folder(dest, true);
            }
            return;
        }
        let launcher = self.cfg.default_launcher.clone();
        self.pending = Some(Pending {
            name: repo_name.clone(),
            stack: "Node.js".into(),
            color: "#3B82F6".into(),
            launcher,
        });
        let log = config::logs_dir().join(format!("clone-{}.log", actions::safe_slug(&repo_name)));
        let tx = self.tx.clone();
        let hw = self.hwnd.0 as isize;
        self.s_log_path = Some(log.clone());
        self.s_proj_path = Some(dest);
        self.bg_progress = Some(0.15);
        self.resume_view = None;
        let rn = repo_name.clone();
        std::thread::spawn(move || actions::git_clone_repo(url, rn, base, log, tx, hw));
        self.status(
            SKind::Creating,
            &format!("Clonando {repo_name}…"),
            "Descargando repositorio de GitHub…",
            GREEN,
            "📥",
        );
        self.s_deadline = None;
    }

    pub(crate) fn start_wizard(&mut self) {
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

    pub(crate) fn reset_libs(&mut self) {
        self.wiz_libs = self
            .cfg
            .templates
            .get(self.wiz_tpl)
            .map(|t| t.libraries.iter().map(|l| l.default).collect())
            .unwrap_or_default();
        self.wiz_lib_scroll = 0;
    }

    pub(crate) fn goto_step(&mut self, s: Step) {
        self.step = s;
        self.hover = None;
        self.fade();
        self.kick();
    }

    pub(crate) fn wizard_next(&mut self) {
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

    pub(crate) fn back(&mut self) {
        match self.view {
            View::Wizard => match self.step {
                Step::Name => self.set_view(View::Expanded),
                Step::Libraries => self.goto_step(Step::Name),
                Step::Launcher => {
                    self.goto_step(if self.cur_libs_len() > 0 { Step::Libraries } else { Step::Name })
                }
            },
            View::Settings | View::GitModal | View::NoteModal => self.set_view(View::Expanded),
            View::CustomEditor => self.set_view(View::Settings),
            View::Expanded if !self.search.is_empty() || self.filter_mode != FilterMode::All => {
                self.search.clear();
                self.filter_mode = FilterMode::All;
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
        self.bg_progress = Some(0.08);
        self.resume_view = None;
        std::thread::spawn(move || actions::scaffold(tpl, libs, name.clone(), base, auto_gh, log, tx, hw));
        let n = self.wiz_name.trim().to_string();
        self.status(SKind::Creating, &format!("Creando {n}"), "Preparando…", color, &initial(&n));
        self.s_deadline = None;
    }

    fn on_worker(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                WorkMsg::Progress(frac, s) => {
                    if frac <= 0.001 && s.is_empty() {
                        self.bg_progress = None;
                    } else {
                        self.bg_progress = Some(frac.clamp(0.05, 0.98));
                        self.s_sub = s;
                    }
                    self.kick();
                }
                WorkMsg::Done { res: Ok(path), warn } => {
                    self.bg_progress = None;
                    let Some(p) = self.pending.take() else { continue };
                    let (det_stack, det_color) = actions::detect(&path);
                    let final_stack = if p.stack == "Node.js" && det_stack != "Solo carpeta" {
                        det_stack
                    } else {
                        p.stack
                    };
                    let final_color = if p.color == "#3B82F6" && det_color != "#6366F1" {
                        det_color
                    } else {
                        p.color
                    };
                    self.cfg.projects.push(Project {
                        name: p.name.clone(),
                        path: path.to_string_lossy().to_string(),
                        stack: final_stack,
                        color: final_color,
                        launcher: Some(p.launcher.clone()),
                        last_opened: 0,
                        pinned: false,
                        note: None,
                        linked_path: None,
                    });
                    let i = self.cfg.projects.len() - 1;
                    self.open_project(i, Some(p.launcher));
                    if let Some((w_msg, log_p)) = warn {
                        self.s_proj_path = Some(path);
                        self.error(
                            &format!("{} listo con advertencia en librería/Git", p.name),
                            &format!("{w_msg} · clic para ver terminal"),
                            Some(log_p),
                        );
                    }
                    self.refresh_live_state();
                }
                WorkMsg::Done { res: Err(e), warn: _ } => {
                    self.bg_progress = None;
                    self.pending = None;
                    let log_p = self.s_log_path.clone();
                    self.error(
                        "Error en la operación",
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
                        let removed = self.servers.remove(&path);
                        let proj_name = removed
                            .map(|r| r.project_name)
                            .unwrap_or_else(|| {
                                Path::new(&path)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "Proyecto".into())
                            });
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
                    self.bg_progress = None;
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

    pub(crate) fn add_existing(&mut self) {
        if let Some(p) = actions::pick_folder(self.hwnd, "Agregar proyecto existente") {
            self.register_and_open_folder(p, false);
        }
        self.focus();
        self.kick();
    }

    pub(crate) fn register_and_open_folder(&mut self, p: PathBuf, open_immediately: bool) {
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
                note: None,
                linked_path: None,
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

    pub(crate) fn save_project_note(&mut self) {
        if let Some(p) = self.cfg.projects.get_mut(self.note_proj_idx) {
            let clean = self.note_text.trim().to_string();
            p.note = if clean.is_empty() { None } else { Some(clean) };
            self.save();
        }
        self.set_view(View::Expanded);
    }

    pub(crate) fn save_custom_editor_item(&mut self) {
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

    pub(crate) fn click(&mut self, h: Hit) {
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
            Hit::FilterChip(mode) => {
                self.filter_mode = if self.filter_mode == mode {
                    FilterMode::All
                } else {
                    mode
                };
                self.sel_idx = 0;
                self.scroll = 0;
                self.kick();
            }
            Hit::CloneClipboardRepo => {
                self.clone_from_clipboard();
            }
            Hit::QuickOpenBaseDir => {
                actions::open_explorer_at(Path::new(&self.cfg.base_dir));
                self.minimize_preserving_state();
            }
            Hit::QuickSyncProjects => {
                self.refresh_live_state();
                self.kick();
            }
            Hit::PortBadge(port) | Hit::ProjPort(_, port) => {
                actions::open_browser(&format!("http://localhost:{port}"));
                self.minimize_preserving_state();
            }
            Hit::PortTerm(port) => {
                let matched = self
                    .servers
                    .iter()
                    .find(|(_, s)| s.port == Some(port));
                if let Some((cwd_str, srv)) = matched {
                    actions::show_server_terminal(
                        Some(&srv.project_name),
                        Some(Path::new(cwd_str)),
                        Some(port),
                        Some(&srv.log_path),
                    );
                } else {
                    actions::show_server_terminal(None, None, Some(port), None);
                }
                self.minimize_preserving_state();
            }
            Hit::KillPort(port) => {
                let matched_pid = self
                    .servers
                    .iter()
                    .find(|(_, s)| s.port == Some(port))
                    .map(|(k, s)| (k.clone(), s.pid));
                if let Some((k, pid)) = matched_pid {
                    self.servers.remove(&k);
                    actions::stop_dev_server(pid);
                }
                actions::kill_port(port, self.tx.clone(), self.hwnd.0 as isize);
            }
            Hit::Project(i) => self.open_project(i, None),
            Hit::ProjServer(i) => self.toggle_project_server(i),
            Hit::ProjServerRole(i, role) => self.toggle_project_server_role(i, role),
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
            Hit::ProjNote(i) => {
                if let Some(p) = self.cfg.projects.get(i) {
                    self.note_proj_idx = i;
                    self.note_text = p.note.clone().unwrap_or_default();
                    self.set_view(View::NoteModal);
                }
            }
            Hit::ProjTerm(i) => {
                if let Some(p) = self.cfg.projects.get(i) {
                    if actions::focus_existing_project_window(&self.cfg.projects, i, "terminal", self.hwnd).is_none() {
                        actions::open_terminal_at(Path::new(&p.path));
                    }
                    self.minimize_preserving_state();
                }
            }
            Hit::ProjExplorer(i) => {
                if let Some(p) = self.cfg.projects.get(i) {
                    if actions::focus_existing_project_window(&self.cfg.projects, i, "explorer", self.hwnd).is_none() {
                        actions::open_explorer_at(Path::new(&p.path));
                    }
                    self.minimize_preserving_state();
                }
            }
            Hit::ProjPin(i) => self.toggle_pin(i),
            Hit::OpenWinFocus(idx) => {
                if let Some(ow) = self.open_windows.get(idx).cloned() {
                    actions::bring_hwnd_to_front(HWND(ow.hwnd as _));
                    self.minimize_preserving_state();
                }
            }
            Hit::OpenWinClose(idx) => {
                if let Some(ow) = self.open_windows.get(idx).cloned() {
                    actions::close_window_handle(ow.hwnd);
                    self.open_windows.remove(idx);
                    self.kick();
                }
            }
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
                    self.bg_progress = Some(0.15);
                    self.status(
                        SKind::Creating,
                        &format!("Subiendo {} a GitHub…", p.name),
                        "Ejecutando git add + commit + push…",
                        WHITE,
                        "✓",
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
            // Note Modal
            Hit::NoteInput => {}
            Hit::NoteSave => self.save_project_note(),
            Hit::NoteClear => {
                self.note_text.clear();
                self.save_project_note();
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
        let pm = actions::detect_package_manager(Path::new(&p.path));
        let is_srv_running = self.project_has_running_server(i);
        let srv_port = self.servers.get(&p.path).and_then(|s| s.port);
        let srv_log = self.servers.get(&p.path).map(|s| s.log_path.clone());
        let fs_info = self.fullstack_map.get(&p.path).cloned();

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

            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                210,
                w!("📝 Editar nota / recordatorio rápido (Mini-TODO)"),
            );
            if p.linked_path.is_some() {
                let _ = AppendMenuW(
                    menu,
                    MF_STRING,
                    212,
                    w!("⛓️‍💥 Desvincular par Full-Stack (Front/Back)"),
                );
            } else {
                let _ = AppendMenuW(
                    menu,
                    MF_STRING,
                    211,
                    w!("🔗 Vincular con otro proyecto (Par Front + Back)"),
                );
            }

            if let Some(fs) = &fs_info {
                if fs.is_fullstack() {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                    let _ = AppendMenuW(menu, MF_STRING, 310, w!("▶ Iniciar/Detener solo Frontend"));
                    let _ = AppendMenuW(menu, MF_STRING, 311, w!("▶ Iniciar/Detener solo Backend"));
                    let _ = AppendMenuW(menu, MF_STRING, 312, w!("⚡ Iniciar/Detener Ambos (Combo Full-Stack)"));
                }
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
                        &HSTRING::from(format!("⚡ {pm} run {s_name}")),
                    );
                }
            }

            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(menu, MF_STRING, 201, w!("📋 Quitar proyecto solo de la lista"));
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                204,
                w!("🗑 Eliminar proyecto por completo (borrar carpeta del disco)"),
            );

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
            } else if cmd == 210 {
                self.note_proj_idx = i;
                self.note_text = p.note.clone().unwrap_or_default();
                self.set_view(View::NoteModal);
            } else if cmd == 211 {
                if let Some(other) = actions::pick_folder(
                    self.hwnd,
                    "Selecciona la carpeta del proyecto hermano (Frontend o Backend) para vincular",
                ) {
                    if let Some(proj) = self.cfg.projects.get_mut(i) {
                        proj.linked_path = Some(other.to_string_lossy().to_string());
                    }
                    self.save();
                    self.scan_and_sync();
                    self.focus();
                    self.kick();
                }
            } else if cmd == 212 {
                if let Some(proj) = self.cfg.projects.get_mut(i) {
                    proj.linked_path = None;
                }
                self.save();
                self.scan_and_sync();
                self.kick();
            } else if cmd == 310 {
                self.toggle_project_server_role(i, ServerRole::Front);
            } else if cmd == 311 {
                self.toggle_project_server_role(i, ServerRole::Back);
            } else if cmd == 312 {
                self.toggle_project_server_role(i, ServerRole::Both);
            } else if cmd == 201 {
                self.cfg.projects.remove(i);
                self.scroll = 0;
                self.save();
                self.kick();
            } else if cmd == 204 {
                if actions::confirm_delete_project(self.hwnd, &p.name, &p.path) {
                    if let Some(srv) = self.servers.remove(&p.path) {
                        actions::stop_dev_server(srv.pid);
                    }
                    for ow in &self.open_windows {
                        if ow.project_path.eq_ignore_ascii_case(&p.path) {
                            actions::close_window_handle(ow.hwnd);
                        }
                    }
                    match actions::delete_project_from_disk(Path::new(&p.path)) {
                        Ok(()) => {
                            if let Some(pos) = self
                                .cfg
                                .projects
                                .iter()
                                .position(|x| x.path.eq_ignore_ascii_case(&p.path))
                            {
                                self.cfg.projects.remove(pos);
                            }
                            self.git_states.remove(&p.path);
                            self.fullstack_map.remove(&p.path);
                            self.scroll = 0;
                            self.save();
                            self.open_windows = actions::scan_open_project_windows_sync(&self.cfg.projects, self.hwnd);
                            self.status(
                                SKind::Done,
                                &format!("{} eliminado", p.name),
                                "Se borró la carpeta del proyecto del disco",
                                WHITE,
                                "✓",
                            );
                            self.s_deadline = Some(Instant::now() + Duration::from_millis(1800));
                        }
                        Err(e) => {
                            self.error(
                                &format!("No se pudo borrar {}", p.name),
                                &format!("{e} (cierra programas que usen esa carpeta)"),
                                None,
                            );
                        }
                    }
                } else {
                    self.focus();
                    self.kick();
                }
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
                View::NoteModal => self.save_project_note(),
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
            View::NoteModal => match c {
                8 => {
                    self.note_text.pop();
                    self.kick();
                    return Some(0);
                }
                13 | 27 | 9 => return Some(0),
                _ => {
                    let Some(ch) = char::from_u32(c) else { return Some(0) };
                    if !ch.is_control() && self.note_text.chars().count() < 68 {
                        self.note_text.push(ch);
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
                        | Some(Hit::ProjServerRole(i, _))
                        | Some(Hit::ProjServerTerm(i))
                        | Some(Hit::ProjPort(i, _))
                        | Some(Hit::ProjGit(i))
                        | Some(Hit::ProjNote(i))
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
                                | View::NoteModal
                                | View::CustomEditor
                        )
                    {
                        self.minimize_preserving_state();
                    }
                    let mut flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
                    if self.visible && !self.auto_hidden_fs {
                        flags |= SWP_SHOWWINDOW;
                    }
                    let _ = SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags);
                    None
                }
                WM_WINDOWPOSCHANGING => {
                    let ppos = lp.0 as *mut WINDOWPOS;
                    if !ppos.is_null() {
                        (*ppos).hwndInsertAfter = HWND_TOPMOST;
                        if self.visible && !self.auto_hidden_fs {
                            (*ppos).flags &= !SWP_HIDEWINDOW;
                        }
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
