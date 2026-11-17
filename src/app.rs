use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::actions::{self, FullStackInfo, GitInfo, OpenProjectWindow, ProjectDiskInfo, RunnerItem, WorkMsg, WM_WORKER};
use crate::config::{self, hex, Config, EnvEntry, Launcher, Library, Project, Template};
use crate::gfx::Gfx;

pub const WIN_W: f32 = 820.0;
pub const WIN_H: f32 = 760.0;
pub(crate) const TOP: f32 = 0.0;
pub const WM_TRAY: u32 = WM_APP + 2;
const WM_MOUSELEAVE: u32 = 0x02A3;
const TIMER_ANIM: usize = 1;
const TIMER_POLL: usize = 2;
const TIMER_TOPMOST: usize = 3;
const TIMER_CONSOLE: usize = 4;
const HOTKEY_ID: i32 = 1;
pub(crate) const MAX_ROWS: usize = 6;

// Paleta #E4002B + Azul Glaciar + Blanco Polar sobre fondo nocturno profundo
pub(crate) const WHITE: u32 = 0xF8FAFC;
pub(crate) const GRAY: u32 = 0x94A3B8;
pub(crate) const DIM: u32 = 0x475569;
pub(crate) const ACCENT: u32 = 0x38BDF8;
pub(crate) const GLACIER: u32 = 0x7DD3FC;
pub(crate) const BRAND_RED: u32 = 0xE4002B;
pub(crate) const BODY: u32 = 0x05070B;
pub(crate) const CARD: u32 = 0x0D121B;
pub(crate) const CARD_HOV: u32 = 0x151E2E;
pub(crate) const BORDER: u32 = 0x1E293B;
pub(crate) const RED: u32 = 0xE4002B;
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
pub(crate) const G_DOWNLOAD: &str = "\u{E896}";
pub(crate) const G_GLOBE: &str = "\u{E774}";
pub(crate) const G_SYNC: &str = "\u{E895}";
pub(crate) const G_LINK: &str = "\u{E71B}";
pub(crate) const G_BOLT: &str = "\u{E945}";
pub(crate) const G_WORK: &str = "\u{E821}";
pub(crate) const G_KEY: &str = "\u{E8D7}";
pub(crate) const G_DISK: &str = "\u{EDA2}";
pub(crate) const G_COPY: &str = "\u{E8C8}";
pub(crate) const G_SUN: &str = "\u{E706}";
pub(crate) const G_MOON: &str = "\u{E708}";

#[derive(Clone, Copy)]
pub(crate) struct Pal {
    pub(crate) light: bool,
    pub(crate) body: u32,
    pub(crate) card: u32,
    pub(crate) card_hov: u32,
    pub(crate) border: u32,
    pub(crate) white: u32,
    pub(crate) gray: u32,
    pub(crate) dim: u32,
    pub(crate) accent: u32,
    pub(crate) glacier: u32,
    pub(crate) cyan: u32,
    pub(crate) brand_red: u32,
    pub(crate) red: u32,
    pub(crate) green: u32,
    pub(crate) amber: u32,
    pub(crate) box_bg: u32,
    pub(crate) inv_ink: u32,
    pub(crate) shadow: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterMode {
    All,
    Workspace(usize), // 0: Trabajo, 1: Personal, 2: Clientes, 3: Pruebas
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IslandPos {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl IslandPos {
    pub const ALL: [IslandPos; 6] = [
        IslandPos::TopLeft,
        IslandPos::TopCenter,
        IslandPos::TopRight,
        IslandPos::BottomLeft,
        IslandPos::BottomCenter,
        IslandPos::BottomRight,
    ];

    pub fn from_str(s: &str) -> Self {
        match s {
            "top_left" => Self::TopLeft,
            "top_right" => Self::TopRight,
            "bottom_left" => Self::BottomLeft,
            "bottom_center" => Self::BottomCenter,
            "bottom_right" => Self::BottomRight,
            _ => Self::TopCenter,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopCenter => "top_center",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomCenter => "bottom_center",
            Self::BottomRight => "bottom_right",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "Arriba · Izquierda",
            Self::TopCenter => "Arriba · Centro (Gota)",
            Self::TopRight => "Arriba · Derecha",
            Self::BottomLeft => "Abajo · Izquierda",
            Self::BottomCenter => "Abajo · Centro",
            Self::BottomRight => "Abajo · Derecha",
        }
    }

    pub fn is_waterdrop(self) -> bool {
        matches!(self, Self::TopCenter)
    }

    pub fn is_corner(self) -> bool {
        matches!(
            self,
            Self::TopLeft | Self::TopRight | Self::BottomLeft | Self::BottomRight
        )
    }

    pub fn corner_id(self) -> u8 {
        match self {
            Self::TopLeft => 0,
            Self::TopRight => 1,
            Self::BottomLeft => 2,
            Self::BottomRight => 3,
            _ => 0,
        }
    }
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
    ProjRunner(usize),
    ProjEnv(usize),
    OpenDiskCleaner,
    OpenLiveConsole,
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
    ToggleSounds,
    CycleMonitor,
    SetPosition(IslandPos),
    ToggleTheme,
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
    // Workspace Management
    AddWorkspace,
    EditWorkspace(usize),
    WsNameInput,
    WsIconPick(usize),
    WsSave,
    WsDelete(usize),
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
    // Env Manager Modal
    EnvTab(usize),
    EnvToggleMask,
    EnvSyncExample,
    EnvKeyInput,
    EnvValInput,
    EnvSaveVar,
    EnvCopyVal(usize),
    EnvEditVar(usize),
    EnvDeleteVar(usize),
    EnvSaveToVault(usize),
    EnvVaultInject(usize),
    EnvVaultDelete(usize),
    // Smart Disk Cleaner Modal
    CleanerTab(usize),
    CleanerRescan,
    CleanerSelectProj(usize),
    CleanerCleanOne(usize),
    CleanerCleanAll,
    CleanerRestore(usize),
    // Live Console Modal
    ConsoleSelectSource(usize),
    ConsoleCopyLastError,
    ConsoleCopyAll,
    ConsoleClear,
    ConsoleRestartServer,
    ConsoleStopTask,
    ConsoleOpenExternal,
    // Quick Runner Modal & Active Tasks
    RunnerExec(usize),
    RunnerOpenEnv,
    RunnerRestoreDeps,
    TaskLog(usize),
    KillTask(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveTask {
    pub(crate) title: String,
    pub(crate) short_label: String,
    pub(crate) pid: u32,
    pub(crate) log_path: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum View {
    Compact,
    Expanded,
    Settings,
    Wizard,
    GitModal,
    NoteModal,
    WorkspaceModal,
    CustomEditor,
    EnvModal,
    CleanerModal,
    ConsoleModal,
    RunnerModal,
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
    pub(crate) swallow_lbuttonup: bool,
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
    // modal de grupo de trabajo (Workspace)
    pub(crate) ws_edit_idx: Option<usize>,
    pub(crate) ws_name: String,
    pub(crate) ws_icon: String,
    // editor visual de Librerías, Programas y Frameworks
    pub(crate) ed_tab: usize, // 0 = Librerías, 1 = Programas, 2 = Frameworks
    pub(crate) ed_focus: usize,
    pub(crate) ed_tpl_idx: usize,
    pub(crate) ed_f1: String,
    pub(crate) ed_f2: String,
    pub(crate) ed_f3: String,
    pub(crate) ed_scroll: usize,
    pub(crate) ed_msg: Option<(String, bool)>,
    // Gestor .env y Bóveda
    pub(crate) env_proj_idx: usize,
    pub(crate) env_filename: String,
    pub(crate) env_vars: Vec<(String, String)>,
    pub(crate) env_tab: usize,
    pub(crate) env_masked: bool,
    pub(crate) env_focus: usize,
    pub(crate) env_key_in: String,
    pub(crate) env_val_in: String,
    pub(crate) env_scroll: usize,
    pub(crate) env_msg: Option<(String, bool)>,
    // Limpiador inteligente de cachés y recetas
    pub(crate) clean_tab: usize,
    pub(crate) clean_scanning: bool,
    pub(crate) clean_items: Vec<ProjectDiskInfo>,
    pub(crate) clean_sel_idx: usize,
    pub(crate) clean_scroll: usize,
    pub(crate) clean_msg: Option<(String, bool)>,
    // Consola integrada en vivo
    pub(crate) con_sources: Vec<(String, PathBuf, Option<String>)>,
    pub(crate) con_source_idx: usize,
    pub(crate) con_lines: Vec<String>,
    pub(crate) con_scroll: usize,
    pub(crate) con_msg: Option<(String, bool)>,
    // Quick Runner por proyecto
    pub(crate) run_proj_idx: usize,
    pub(crate) run_items: Vec<RunnerItem>,
    pub(crate) run_scroll: usize,
    pub(crate) last_task_log: Option<(String, PathBuf)>,
    pub(crate) active_tasks: Vec<ActiveTask>,
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
            swallow_lbuttonup: false,
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
            ws_edit_idx: None,
            ws_name: String::new(),
            ws_icon: String::from(G_WORK),
            ed_tab: 0,
            ed_focus: 0,
            ed_tpl_idx: 0,
            ed_f1: String::new(),
            ed_f2: String::new(),
            ed_f3: String::from("React + Vite"),
            ed_scroll: 0,
            ed_msg: None,
            env_proj_idx: 0,
            env_filename: String::from(".env"),
            env_vars: Vec::new(),
            env_tab: 0,
            env_masked: true,
            env_focus: 0,
            env_key_in: String::new(),
            env_val_in: String::new(),
            env_scroll: 0,
            env_msg: None,
            clean_tab: 0,
            clean_scanning: false,
            clean_items: Vec::new(),
            clean_sel_idx: 0,
            clean_scroll: 0,
            clean_msg: None,
            con_sources: Vec::new(),
            con_source_idx: 0,
            con_lines: Vec::new(),
            con_scroll: 0,
            con_msg: None,
            run_proj_idx: 0,
            run_items: Vec::new(),
            run_scroll: 0,
            last_task_log: None,
            active_tasks: Vec::new(),
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
            SetTimer(self.hwnd, TIMER_POLL, 4500, None);
            SetTimer(self.hwnd, TIMER_TOPMOST, 900, None);
        }
        self.kick();
    }

    pub(crate) fn scan_and_sync(&mut self) {
        let scanned = actions::scan_base_dir(Path::new(&self.cfg.base_dir));
        let mut cfg_changed = self.cfg.sync_scanned_projects(scanned);
        for p in &mut self.cfg.projects {
            if p.last_opened == 0 {
                let ts = actions::folder_modified_unix(Path::new(&p.path));
                if ts > 0 {
                    p.last_opened = ts;
                    cfg_changed = true;
                }
            }
        }
        if cfg_changed {
            self.cfg.sort_projects();
            self.save();
        }
        // Limpiar estados de proyectos que ya no existan
        let valid_paths: std::collections::HashSet<String> =
            self.cfg.projects.iter().map(|p| p.path.to_lowercase()).collect();
        self.git_states
            .retain(|k, _| valid_paths.contains(&k.to_lowercase()));
        self.fullstack_map
            .retain(|k, _| valid_paths.contains(&k.to_lowercase()));

        for p in &self.cfg.projects {
            if !self.fullstack_map.contains_key(&p.path) {
                let fs_info = actions::detect_project_targets(
                    Path::new(&p.path),
                    &p.stack,
                    p.linked_path.as_deref(),
                );
                self.fullstack_map.insert(p.path.clone(), fs_info);
            }

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

    /// Actualización ligera e instantánea al desplegar el panel (sin crear subprocesos `git.exe` que muestren cursor de carga).
    pub(crate) fn refresh_on_expand(&mut self) {
        self.scan_and_sync();
        self.clipboard_repo = actions::read_clipboard_github_repo(self.hwnd);
        self.open_windows = actions::scan_open_project_windows_sync(&self.cfg.projects, self.hwnd);
    }

    pub(crate) fn refresh_live_state(&mut self) {
        self.fullstack_map.clear();
        self.scan_and_sync();
        self.clipboard_repo = actions::read_clipboard_github_repo(self.hwnd);
        let paths: Vec<String> = self.cfg.projects.iter().map(|p| p.path.clone()).collect();
        let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
        let hw = self.hwnd.0 as isize;
        self.open_windows = actions::scan_open_project_windows_sync(&self.cfg.projects, self.hwnd);
        actions::inspect_git_all(paths, self.tx.clone(), hw);
        actions::scan_open_ports(extra_ports, self.tx.clone(), hw);
    }

    fn update_fullscreen_and_pos(&mut self) {
        let (mon_rc, work_rc) = actions::target_monitor_rects(&self.cfg.monitor_mode);
        let (new_x, new_y) = self.desired_window_pos(mon_rc, work_rc);
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

        if self.visible && !self.auto_hidden_fs {
            actions::ensure_island_above_taskbar(self.hwnd);
        }
    }

    fn poll_periodic(&mut self) {
        self.update_fullscreen_and_pos();

        // Solo si el panel está abierto, sincronizar carpetas borradas y ventanas abiertas
        if self.view == View::Expanded {
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

    pub(crate) fn island_pos(&self) -> IslandPos {
        IslandPos::from_str(&self.cfg.position)
    }

    pub(crate) fn pal(&self) -> Pal {
        if self.cfg.light_mode {
            Pal {
                light: true,
                body: 0xF4F7FB,
                card: 0xFFFFFF,
                card_hov: 0xE6F0FA,
                border: 0xCBD5E1,
                white: 0x0F172A,
                gray: 0x475569,
                dim: 0x64748B,
                accent: 0x0284C7,
                glacier: 0x0284C7,
                cyan: 0x0EA5E9,
                brand_red: BRAND_RED,
                red: BRAND_RED,
                green: 0x16A34A,
                amber: 0xD97706,
                box_bg: 0xEBF1F7,
                inv_ink: 0xF8FAFC,
                shadow: 0x0F172A,
            }
        } else {
            Pal {
                light: false,
                body: BODY,
                card: CARD,
                card_hov: CARD_HOV,
                border: BORDER,
                white: WHITE,
                gray: GRAY,
                dim: DIM,
                accent: ACCENT,
                glacier: GLACIER,
                cyan: CYAN,
                brand_red: BRAND_RED,
                red: RED,
                green: GREEN,
                amber: AMBER,
                box_bg: 0x07090D,
                inv_ink: 0x05070B,
                shadow: 0x000000,
            }
        }
    }

    pub(crate) fn desired_window_pos(&self, mon_rc: RECT, work_rc: RECT) -> (i32, i32) {
        let pos = self.island_pos();
        let x = match pos {
            IslandPos::TopLeft | IslandPos::BottomLeft => work_rc.left,
            IslandPos::TopCenter | IslandPos::BottomCenter => {
                mon_rc.left + ((mon_rc.right - mon_rc.left) - self.gfx.wpx) / 2
            }
            IslandPos::TopRight | IslandPos::BottomRight => work_rc.right - self.gfx.wpx,
        };
        let y = match pos {
            IslandPos::TopCenter => mon_rc.top,
            IslandPos::TopLeft | IslandPos::TopRight => work_rc.top,
            IslandPos::BottomLeft | IslandPos::BottomCenter | IslandPos::BottomRight => {
                work_rc.bottom - self.gfx.hpx
            }
        };
        (x, y)
    }

    pub(crate) fn panel_origin_ex(&self, w: f32, h: f32, compact: bool) -> (f32, f32) {
        let edge_x = if compact { 0.0 } else { 12.0 };
        let edge_y = if compact { 0.0 } else { 8.0 };
        let pos = self.island_pos();
        let x = match pos {
            IslandPos::TopLeft | IslandPos::BottomLeft => edge_x,
            IslandPos::TopCenter | IslandPos::BottomCenter => WIN_W / 2.0 - w / 2.0,
            IslandPos::TopRight | IslandPos::BottomRight => WIN_W - edge_x - w,
        };
        let y = match pos {
            IslandPos::TopCenter => TOP,
            IslandPos::TopLeft | IslandPos::TopRight => edge_y,
            IslandPos::BottomLeft | IslandPos::BottomCenter | IslandPos::BottomRight => {
                WIN_H - edge_y - h
            }
        };
        (x, y)
    }

    pub(crate) fn panel_origin(&self, w: f32, h: f32) -> (f32, f32) {
        self.panel_origin_ex(w, h, self.view == View::Compact)
    }

    pub(crate) fn compact_dot_positions(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        count: usize,
        dot_r: f32,
    ) -> Vec<(f32, f32)> {
        let pos = self.island_pos();
        if pos.is_waterdrop() {
            return Self::waterdrop_dot_positions(x + w / 2.0, w, h, count, dot_r);
        }
        if pos == IslandPos::BottomCenter {
            let n = count.clamp(1, 6);
            let spacing = (dot_r * 2.0 + 4.5).min((w - 16.0) / (n.max(1) as f32));
            let total_w = (n.saturating_sub(1)) as f32 * spacing;
            let start_x = x + w / 2.0 - total_w / 2.0;
            let cy = y + h * 0.56;
            return (0..n).map(|i| (start_x + i as f32 * spacing, cy)).collect();
        }
        // 4 Esquinas: si está estirada en hover, puntos a la derecha del mini logo; si en reposo triangular, puntos ordenados en el borde interior
        let n = count.clamp(1, 4);
        let is_right = matches!(pos, IslandPos::TopRight | IslandPos::BottomRight);
        let is_bottom = matches!(pos, IslandPos::BottomLeft | IslandPos::BottomRight);
        if w > h + 14.0 {
            let cy = y + h * 0.5;
            let start_x = if is_right { x + 16.0 } else { x + w - 16.0 - (n - 1) as f32 * 9.0 };
            return (0..n).map(|i| (start_x + i as f32 * 9.0, cy)).collect();
        }
        let cx = if is_right { x + w - 14.0 } else { x + 14.0 };
        let cy = if is_bottom { y + h - 14.0 } else { y + 14.0 };
        vec![(cx, cy); 1]
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
                    FilterMode::Workspace(w_idx) => {
                        if let Some(w_def) = self.cfg.workspaces.get(w_idx) {
                            p.workspace.as_deref().map(|ws| ws.eq_ignore_ascii_case(&w_def.name) || ws.eq_ignore_ascii_case(&w_def.id)).unwrap_or(false)
                        } else {
                            false
                        }
                    }
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
                let ws = p
                    .workspace
                    .as_deref()
                    .map(|w| w.to_lowercase())
                    .unwrap_or_default();
                p.name.to_lowercase().contains(&q)
                    || p.stack.to_lowercase().contains(&q)
                    || branch.contains(&q)
                    || note.contains(&q)
                    || ws.contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    // ---------- ventana / sistema ----------

    pub(crate) fn reposition(&mut self) {
        let (mon_rc, work_rc) = actions::target_monitor_rects(&self.cfg.monitor_mode);
        self.pos = self.desired_window_pos(mon_rc, work_rc);
        let flags = if self.visible && !self.auto_hidden_fs {
            SWP_NOACTIVATE | SWP_SHOWWINDOW
        } else {
            SWP_NOACTIVATE
        };
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                self.pos.0,
                self.pos.1,
                self.gfx.wpx,
                self.gfx.hpx,
                flags,
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
        for task in &self.active_tasks {
            actions::stop_dev_server(task.pid);
        }
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_POLL);
            let _ = KillTimer(self.hwnd, TIMER_TOPMOST);
            let _ = KillTimer(self.hwnd, TIMER_CONSOLE);
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
                let pos = self.island_pos();
                let hov = self.hov(Hit::Pill);
                let live = self.has_live_server();
                if pos.is_waterdrop() {
                    if hov {
                        if live { (96.0, 24.0, 12.0) } else { (78.0, 24.0, 12.0) }
                    } else if live {
                        (54.0, 10.0, 5.0)
                    } else {
                        (48.0, 10.0, 5.0)
                    }
                } else if pos == IslandPos::BottomCenter {
                    if hov {
                        if live { (116.0, 28.0, 14.0) } else { (108.0, 28.0, 14.0) }
                    } else if live {
                        (64.0, 13.0, 6.5)
                    } else {
                        (56.0, 12.0, 6.0)
                    }
                } else {
                    // 4 esquinas: triángulo curvo en reposo -> pestaña esquinera al hacer hover
                    if hov {
                        if live { (122.0, 34.0, 14.0) } else { (112.0, 34.0, 14.0) }
                    } else if live {
                        (48.0, 48.0, 14.0)
                    } else {
                        (44.0, 44.0, 14.0)
                    }
                }
            }
            View::Expanded => {
                let filtered_len = self.filtered_projects().len();
                let n = filtered_len.clamp(3, MAX_ROWS) as f32;
                let ports_bar = if self.active_ports.is_empty() && self.active_tasks.is_empty() { 0.0 } else { 32.0 };
                let search_bar = 42.0;
                let chips_bar = 56.0;
                (808.0, 86.0 + ports_bar + search_bar + chips_bar + n * 54.0 + 54.0 + 18.0, 26.0)
            }
            View::Settings => (548.0, 64.0 + 84.0 + 11.0 * 44.0 + 34.0, 26.0),
            View::Wizard => {
                let rows = match self.step {
                    Step::Name => self.cfg.templates.len().min(5) as f32 * 46.0 + 118.0,
                    Step::Libraries => self.cur_libs_len().clamp(1, 6) as f32 * 46.0 + 48.0,
                    Step::Launcher => self.cfg.launchers.len().min(6) as f32 * 46.0 + 48.0,
                };
                (548.0, 64.0 + rows + 68.0, 26.0)
            }
            View::GitModal => (548.0, 340.0, 26.0),
            View::NoteModal => (540.0, 310.0, 26.0),
            View::WorkspaceModal => (500.0, 310.0, 26.0),
            View::CustomEditor => (548.0, 495.0, 26.0),
            View::EnvModal => (660.0, 520.0, 24.0),
            View::CleanerModal => (700.0, 540.0, 24.0),
            View::ConsoleModal => (720.0, 520.0, 24.0),
            View::RunnerModal => (640.0, 500.0, 24.0),
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
        // Si estamos en la gota (View::Compact) y hay un hover activo en Hit::Pill,
        // validar con la posición física real del cursor usando ScreenToClient y self.hit_at
        // para evitar hover fantasma o que se quede palpitando indefinidamente.
        if self.view == View::Compact && self.hover == Some(Hit::Pill) {
            let mut pt = POINT::default();
            if unsafe { GetCursorPos(&mut pt) }.is_ok() {
                unsafe {
                    let _ = ScreenToClient(self.hwnd, &mut pt);
                }
                let scale = self.gfx.scale;
                let cx = pt.x as f32 / scale;
                let cy = pt.y as f32 / scale;
                if self.hit_at(cx, cy) != Some(Hit::Pill) {
                    self.hover = None;
                    self.tracking = false;
                    self.glow_anim = 0.0;
                    self.kick();
                }
            }
        }

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
            if self.glow_anim < 1.0 {
                self.glow_anim = (self.glow_anim + dt * 8.0).min(1.0);
                moving = true;
            }
        } else if self.glow_anim > 0.0 {
            self.glow_anim = (self.glow_anim - dt * 8.0).max(0.0);
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

    pub(crate) fn is_expanded_panel(v: View) -> bool {
        matches!(
            v,
            View::Expanded
                | View::Settings
                | View::Wizard
                | View::GitModal
                | View::NoteModal
                | View::WorkspaceModal
                | View::CustomEditor
                | View::EnvModal
                | View::CleanerModal
                | View::ConsoleModal
                | View::RunnerModal
        )
    }

    /// Cambia la vista activa sin destruir el progreso de creación, búsqueda o formularios.
    pub(crate) fn set_view(&mut self, v: View) {
        if v == self.view {
            return;
        }
        let was_console = self.view == View::ConsoleModal;
        self.view = v;
        self.hover = None;
        self.fade();
        unsafe {
            if was_console && v != View::ConsoleModal {
                let _ = KillTimer(self.hwnd, TIMER_CONSOLE);
            } else if !was_console && v == View::ConsoleModal {
                SetTimer(self.hwnd, TIMER_CONSOLE, 450, None);
            }
        }
        if v == View::Expanded {
            actions::play_ui_sound("pop", self.cfg.sounds_enabled);
            self.refresh_on_expand();
        }
        if Self::is_expanded_panel(v) {
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
        if Self::is_expanded_panel(self.view) {
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
                | Some(Hit::ProjRunner(k))
                | Some(Hit::ProjEnv(k))
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
            "✓",
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
            View::Settings
            | View::GitModal
            | View::NoteModal
            | View::WorkspaceModal
            | View::EnvModal
            | View::CleanerModal
            | View::ConsoleModal
            | View::RunnerModal => self.set_view(View::Expanded),
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
                        workspace: None,
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
                WorkMsg::DiskScanResult(items) => {
                    self.clean_scanning = false;
                    self.clean_items = items;
                    self.clean_sel_idx = self.clean_sel_idx.min(self.clean_items.len().saturating_sub(1));
                    self.clean_scroll = 0;
                    self.kick();
                }
                WorkMsg::DiskCleanDone {
                    manifests,
                    message,
                } => {
                    self.clean_scanning = false;
                    self.bg_progress = None;
                    let is_err = manifests.is_empty();
                    for m in manifests {
                        self.cfg
                            .clean_manifests
                            .retain(|x| !x.project_path.eq_ignore_ascii_case(&m.project_path));
                        self.cfg.clean_manifests.insert(0, m.clone());
                        self.clean_items
                            .retain(|it| !it.project_path.eq_ignore_ascii_case(&m.project_path));
                    }
                    self.save();
                    self.clean_sel_idx = self.clean_sel_idx.min(self.clean_items.len().saturating_sub(1));
                    self.clean_scroll = 0;
                    self.clean_msg = Some((message.clone(), is_err));
                    if self.view == View::CleanerModal {
                        actions::play_ui_sound(if is_err { "click" } else { "save" }, self.cfg.sounds_enabled);
                        self.kick();
                    } else {
                        self.status(
                            if is_err { SKind::Error } else { SKind::Done },
                            "Limpiador de Espacio",
                            &message,
                            if is_err { RED } else { GREEN },
                            "✓",
                        );
                        self.s_deadline = Some(Instant::now() + Duration::from_millis(2600));
                    }
                }
                WorkMsg::RestoreDone {
                    project_path,
                    project_name,
                    ok,
                    message,
                    log_path,
                } => {
                    self.bg_progress = None;
                    if ok {
                        self.cfg
                            .clean_manifests
                            .retain(|x| !x.project_path.eq_ignore_ascii_case(&project_path));
                        self.save();
                        actions::play_ui_sound("save", self.cfg.sounds_enabled);
                    }
                    self.last_task_log = Some((format!("Restaurar: {project_name}"), log_path.clone()));
                    if self.view == View::ConsoleModal {
                        self.refresh_console_lines();
                        self.con_msg = Some((format!("{project_name}: {message}"), !ok));
                        self.kick();
                    } else if ok {
                        self.s_log_path = Some(log_path);
                        self.status(SKind::Done, &format!("{project_name} restaurado"), &message, GREEN, "✓");
                        self.s_deadline = Some(Instant::now() + Duration::from_millis(2400));
                    } else {
                        self.error(&format!("Error al restaurar {project_name}"), &message, Some(log_path));
                    }
                }
                WorkMsg::RunnerTaskStarted {
                    title,
                    short_label,
                    pid,
                    log_path,
                } => {
                    self.active_tasks.retain(|t| t.pid != pid);
                    self.active_tasks.push(ActiveTask {
                        title,
                        short_label,
                        pid,
                        log_path,
                    });
                    self.kick();
                }
                WorkMsg::RunnerTaskDone {
                    title,
                    pid,
                    ok,
                    message,
                    log_path,
                } => {
                    if pid > 0 {
                        self.active_tasks.retain(|t| t.pid != pid);
                    } else {
                        self.active_tasks.retain(|t| t.log_path != log_path);
                    }
                    self.last_task_log = Some((title.clone(), log_path.clone()));
                    if self.view == View::ConsoleModal {
                        self.refresh_console_lines();
                        self.con_msg = Some((format!("{title}: {message}"), !ok));
                        self.kick();
                    } else if !ok {
                        self.error(&format!("Fallo en {title}"), &message, Some(log_path));
                    } else {
                        self.kick();
                    }
                }
            }
        }
    }

    pub(crate) fn open_env_modal(&mut self, proj_idx: usize) {
        let Some(p) = self.cfg.projects.get(proj_idx) else { return };
        let (fname, vars) = actions::read_project_env(Path::new(&p.path));
        self.env_proj_idx = proj_idx;
        self.env_filename = fname;
        self.env_vars = vars;
        self.env_tab = 0;
        self.env_masked = true;
        self.env_focus = 0;
        self.env_key_in.clear();
        self.env_val_in.clear();
        self.env_scroll = 0;
        self.env_msg = None;
        self.set_view(View::EnvModal);
    }

    pub(crate) fn open_cleaner_modal(&mut self) {
        self.clean_tab = 0;
        self.clean_scanning = true;
        self.clean_sel_idx = 0;
        self.clean_scroll = 0;
        self.clean_msg = None;
        let projs: Vec<(String, String)> = self
            .cfg
            .projects
            .iter()
            .map(|p| (p.name.clone(), p.path.clone()))
            .collect();
        actions::scan_all_projects_disk(projs, self.tx.clone(), self.hwnd.0 as isize);
        self.set_view(View::CleanerModal);
    }

    pub(crate) fn open_runner_modal(&mut self, proj_idx: usize) {
        let Some(p) = self.cfg.projects.get(proj_idx) else { return };
        self.run_proj_idx = proj_idx;
        self.run_items = actions::detect_project_runner_items(Path::new(&p.path));
        self.run_scroll = 0;
        self.set_view(View::RunnerModal);
    }

    pub(crate) fn rebuild_console_sources(&mut self, preferred: Option<(String, PathBuf, Option<String>)>) {
        let mut sources: Vec<(String, PathBuf, Option<String>)> = Vec::new();
        if let Some(pref) = preferred {
            sources.push(pref);
        }
        for (cwd, srv) in &self.servers {
            let lbl = if let Some(pt) = srv.port {
                if srv.label == "Main" {
                    format!("{} (:{pt})", srv.project_name)
                } else {
                    format!("{} {} (:{pt})", srv.project_name, srv.label)
                }
            } else {
                srv.project_name.clone()
            };
            if !sources.iter().any(|s| s.1 == srv.log_path) {
                sources.push((lbl, srv.log_path.clone(), Some(cwd.clone())));
            }
        }
        for task in &self.active_tasks {
            if !sources.iter().any(|s| s.1 == task.log_path) {
                sources.push((task.title.clone(), task.log_path.clone(), None));
            }
        }
        if let Some((lbl, lp)) = &self.last_task_log {
            if !sources.iter().any(|s| &s.1 == lp) {
                sources.push((lbl.clone(), lp.clone(), None));
            }
        }
        if let Some(lp) = &self.s_log_path {
            if lp.exists() && !sources.iter().any(|s| &s.1 == lp) {
                sources.push(("Última operación".into(), lp.clone(), None));
            }
        }
        self.con_sources = sources;
        self.con_source_idx = self.con_source_idx.min(self.con_sources.len().saturating_sub(1));
    }

    pub(crate) fn open_console_modal(&mut self, preferred: Option<(String, PathBuf, Option<String>)>) {
        self.con_source_idx = 0;
        self.con_scroll = 0;
        self.con_msg = None;
        self.rebuild_console_sources(preferred);
        self.refresh_console_lines();
        self.set_view(View::ConsoleModal);
    }

    pub(crate) fn refresh_console_lines(&mut self) -> bool {
        let new_lines = self
            .con_sources
            .get(self.con_source_idx)
            .map(|(_, lp, _)| actions::read_log_tail(lp, 220))
            .unwrap_or_default();
        if new_lines != self.con_lines {
            self.con_lines = new_lines;
            true
        } else {
            false
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
                workspace: None,
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
            let is_empty = clean.is_empty();
            p.note = if is_empty { None } else { Some(clean) };
            self.save();
            if is_empty {
                actions::play_ui_sound("delete", self.cfg.sounds_enabled);
            } else {
                actions::play_ui_sound("save", self.cfg.sounds_enabled);
            }
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
                            self.open_console_modal(Some((self.s_title.clone(), lp, None)));
                        } else {
                            self.set_view(View::Compact);
                        }
                    } else {
                        self.set_view(View::Compact);
                    }
                } else if self.view == View::Status && self.skind == SKind::Creating {
                    // Al hacer clic en el banner mientras crea/clona/sube, minimizar a la gota con barra de progreso en el borde
                    self.resume_view = Some(View::Status);
                    self.set_view(View::Compact);
                } else if self.view == View::Compact && self.bg_progress.is_some() {
                    self.set_view(View::Status);
                } else {
                    self.reload_if_changed();
                    self.reopen_preserved_view();
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
                actions::play_ui_sound("tab", self.cfg.sounds_enabled);
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
                    .find(|(_, s)| s.port == Some(port))
                    .map(|(cwd, srv)| {
                        (
                            format!("{} (:{port})", srv.project_name),
                            srv.log_path.clone(),
                            Some(cwd.clone()),
                        )
                    });
                self.open_console_modal(matched);
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
                    self.active_ports.retain(|&p| p != port);
                    let _ = self.tx.send(WorkMsg::Toast {
                        title: format!("Puerto :{port} liberado"),
                        sub: "Servidor detenido correctamente".into(),
                        is_err: false,
                        log_path: None,
                    });
                    let extra_ports: Vec<u16> = self.servers.values().filter_map(|s| s.port).collect();
                    actions::scan_open_ports(extra_ports, self.tx.clone(), self.hwnd.0 as isize);
                    self.kick();
                } else {
                    actions::kill_port(port, self.tx.clone(), self.hwnd.0 as isize);
                }
            }
            Hit::Project(i) => self.open_project(i, None),
            Hit::ProjServer(i) => self.toggle_project_server(i),
            Hit::ProjServerRole(i, role) => self.toggle_project_server_role(i, role),
            Hit::ProjServerTerm(i) => {
                if let Some(p) = self.cfg.projects.get(i).cloned() {
                    let pref = self.servers.get(&p.path).map(|srv| {
                        let lbl = srv
                            .port
                            .map(|pt| format!("{} (:{pt})", p.name))
                            .unwrap_or_else(|| p.name.clone());
                        (lbl, srv.log_path.clone(), Some(p.path.clone()))
                    });
                    self.open_console_modal(pref);
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
            Hit::ProjRunner(i) => self.open_runner_modal(i),
            Hit::ProjEnv(i) => self.open_env_modal(i),
            Hit::OpenDiskCleaner => self.open_cleaner_modal(),
            Hit::OpenLiveConsole => self.open_console_modal(None),
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
            Hit::ToggleSounds => {
                self.cfg.sounds_enabled = !self.cfg.sounds_enabled;
                if self.cfg.sounds_enabled {
                    actions::play_ui_sound("click", true);
                }
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
            Hit::SetPosition(pos) => {
                self.cfg.position = pos.as_str().into();
                self.save();
                self.reposition();
                actions::play_ui_sound("click", self.cfg.sounds_enabled);
                self.kick();
            }
            Hit::ToggleTheme => {
                self.cfg.light_mode = !self.cfg.light_mode;
                self.save();
                actions::play_ui_sound("click", self.cfg.sounds_enabled);
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
                if let Some(p) = self.cfg.projects.get(self.git_proj_idx).cloned() {
                    let lp = config::logs_dir().join(format!("git-{}.log", actions::safe_slug(&p.name)));
                    if lp.exists() {
                        self.open_console_modal(Some((format!("Git · {}", p.name), lp, None)));
                    } else {
                        actions::open_terminal_at(Path::new(&p.path));
                        self.minimize_preserving_state();
                    }
                }
            }
            // Note Modal
            Hit::NoteInput => {}
            Hit::NoteSave => self.save_project_note(),
            Hit::NoteClear => {
                self.note_text.clear();
                self.save_project_note();
            }
            // Workspace Management
            Hit::AddWorkspace => {
                self.ws_edit_idx = None;
                self.ws_name.clear();
                self.ws_icon = G_WORK.into();
                self.set_view(View::WorkspaceModal);
            }
            Hit::EditWorkspace(idx) => {
                if let Some(w) = self.cfg.workspaces.get(idx) {
                    self.ws_edit_idx = Some(idx);
                    self.ws_name = w.name.clone();
                    self.ws_icon = w.icon.clone();
                    self.set_view(View::WorkspaceModal);
                }
            }
            Hit::WsNameInput => {}
            Hit::WsIconPick(idx) => {
                if let Some(&icon) = crate::modals::WS_ICONS.get(idx) {
                    self.ws_icon = icon.into();
                    actions::play_ui_sound("click", self.cfg.sounds_enabled);
                    self.kick();
                }
            }
            Hit::WsSave => {
                let name = self.ws_name.trim().to_string();
                if !name.is_empty() {
                    let icon = if self.ws_icon.trim().is_empty() {
                        G_FOLDER.to_string()
                    } else {
                        crate::config::normalize_ws_icon(&self.ws_icon)
                    };
                    if let Some(idx) = self.ws_edit_idx {
                        if let Some(w) = self.cfg.workspaces.get_mut(idx) {
                            let old_name = w.name.clone();
                            w.name = name.clone();
                            w.icon = icon;
                            for p in &mut self.cfg.projects {
                                if p.workspace.as_deref() == Some(&old_name) {
                                    p.workspace = Some(name.clone());
                                }
                            }
                        }
                    } else {
                        let id = name.to_lowercase().replace(' ', "-");
                        self.cfg.workspaces.push(crate::config::WorkspaceDef {
                            id,
                            name,
                            icon,
                        });
                    }
                    actions::play_ui_sound("save", self.cfg.sounds_enabled);
                    self.save();
                }
                self.set_view(View::Expanded);
            }
            Hit::WsDelete(idx) => {
                if idx < self.cfg.workspaces.len() {
                    let removed = self.cfg.workspaces.remove(idx);
                    for p in &mut self.cfg.projects {
                        if p.workspace.as_deref() == Some(&removed.name) || p.workspace.as_deref() == Some(&removed.id) {
                            p.workspace = None;
                        }
                    }
                    match self.filter_mode {
                        FilterMode::Workspace(cur) if cur == idx => self.filter_mode = FilterMode::All,
                        FilterMode::Workspace(cur) if cur > idx => self.filter_mode = FilterMode::Workspace(cur - 1),
                        _ => {}
                    }
                    actions::play_ui_sound("delete", self.cfg.sounds_enabled);
                    self.save();
                }
                self.set_view(View::Expanded);
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
            // Env Manager Modal
            Hit::EnvTab(t) => {
                self.env_tab = t;
                self.env_scroll = 0;
                self.env_msg = None;
                self.kick();
            }
            Hit::EnvToggleMask => {
                self.env_masked = !self.env_masked;
                self.kick();
            }
            Hit::EnvSyncExample => {
                if let Some(p) = self.cfg.projects.get(self.env_proj_idx) {
                    match actions::sync_env_example(Path::new(&p.path), &self.env_vars) {
                        Ok(n) => {
                            actions::play_ui_sound("save", self.cfg.sounds_enabled);
                            self.env_msg = Some((
                                format!(".env.example sincronizado ({n} claves sin secretos)"),
                                false,
                            ));
                        }
                        Err(e) => {
                            self.env_msg = Some((format!("Error al sincronizar: {e}"), true));
                        }
                    }
                    self.kick();
                }
            }
            Hit::EnvKeyInput => {
                self.env_focus = 0;
                self.kick();
            }
            Hit::EnvValInput => {
                self.env_focus = 1;
                self.kick();
            }
            Hit::EnvSaveVar => self.save_env_var_from_inputs(),
            Hit::EnvCopyVal(idx) => {
                if let Some((k, v)) = self.env_vars.get(idx) {
                    if actions::write_clipboard_text(self.hwnd, v) {
                        actions::play_ui_sound("click", self.cfg.sounds_enabled);
                        self.env_msg = Some((format!("Valor de {k} copiado"), false));
                    }
                    self.kick();
                }
            }
            Hit::EnvEditVar(idx) => {
                if let Some((k, v)) = self.env_vars.get(idx) {
                    self.env_key_in = k.clone();
                    self.env_val_in = v.clone();
                    self.env_focus = 1;
                    self.env_msg = None;
                    self.kick();
                }
            }
            Hit::EnvDeleteVar(idx) => {
                if idx < self.env_vars.len() {
                    let removed = self.env_vars.remove(idx);
                    if let Some(p) = self.cfg.projects.get(self.env_proj_idx) {
                        let _ = actions::write_project_env(
                            Path::new(&p.path),
                            &self.env_filename,
                            &self.env_vars,
                        );
                    }
                    self.env_scroll = self.env_scroll.min(self.env_vars.len().saturating_sub(5));
                    self.env_msg = Some((format!("{} eliminada de {}", removed.0, self.env_filename), false));
                    actions::play_ui_sound("delete", self.cfg.sounds_enabled);
                    self.kick();
                }
            }
            Hit::EnvSaveToVault(idx) => {
                if let Some((k, v)) = self.env_vars.get(idx).cloned() {
                    if let Some(pos) = self.cfg.env_vault.iter().position(|e| e.key == k) {
                        self.cfg.env_vault[pos].value = v;
                    } else {
                        self.cfg.env_vault.push(EnvEntry { key: k.clone(), value: v });
                    }
                    self.save();
                    actions::play_ui_sound("save", self.cfg.sounds_enabled);
                    self.env_msg = Some((format!("{k} guardada en tu Bóveda Personal"), false));
                    self.kick();
                }
            }
            Hit::EnvVaultInject(idx) => {
                if let Some(entry) = self.cfg.env_vault.get(idx).cloned() {
                    if let Some(pos) = self.env_vars.iter().position(|(k, _)| k == &entry.key) {
                        self.env_vars[pos].1 = entry.value;
                    } else {
                        self.env_vars.push((entry.key.clone(), entry.value));
                    }
                    if let Some(p) = self.cfg.projects.get(self.env_proj_idx) {
                        match actions::write_project_env(
                            Path::new(&p.path),
                            &self.env_filename,
                            &self.env_vars,
                        ) {
                            Ok(()) => {
                                actions::play_ui_sound("save", self.cfg.sounds_enabled);
                                self.env_msg = Some((
                                    format!("{} inyectada en {}", entry.key, self.env_filename),
                                    false,
                                ));
                            }
                            Err(e) => {
                                self.env_msg = Some((format!("Error al escribir: {e}"), true));
                            }
                        }
                    }
                    self.kick();
                }
            }
            Hit::EnvVaultDelete(idx) => {
                if idx < self.cfg.env_vault.len() {
                    let rem = self.cfg.env_vault.remove(idx);
                    self.save();
                    actions::play_ui_sound("delete", self.cfg.sounds_enabled);
                    self.env_msg = Some((format!("{} quitada de la bóveda", rem.key), false));
                    self.kick();
                }
            }
            // Smart Disk Cleaner Modal
            Hit::CleanerTab(t) => {
                self.clean_tab = t;
                self.clean_scroll = 0;
                self.clean_sel_idx = 0;
                self.clean_msg = None;
                self.kick();
            }
            Hit::CleanerRescan => {
                self.clean_scanning = true;
                self.clean_msg = None;
                let projs: Vec<(String, String)> = self
                    .cfg
                    .projects
                    .iter()
                    .map(|p| (p.name.clone(), p.path.clone()))
                    .collect();
                actions::scan_all_projects_disk(projs, self.tx.clone(), self.hwnd.0 as isize);
                self.kick();
            }
            Hit::CleanerSelectProj(idx) => {
                self.clean_sel_idx = idx;
                self.kick();
            }
            Hit::CleanerCleanOne(idx) => {
                if let Some(item) = self.clean_items.get(idx).cloned() {
                    self.clean_scanning = true;
                    self.bg_progress = Some(0.15);
                    actions::run_clean_projects_bg(vec![item], self.tx.clone(), self.hwnd.0 as isize);
                    self.kick();
                }
            }
            Hit::CleanerCleanAll => {
                if !self.clean_items.is_empty() && !self.clean_scanning {
                    self.clean_scanning = true;
                    self.bg_progress = Some(0.1);
                    actions::run_clean_projects_bg(
                        self.clean_items.clone(),
                        self.tx.clone(),
                        self.hwnd.0 as isize,
                    );
                    self.kick();
                }
            }
            Hit::CleanerRestore(idx) => {
                if let Some(manifest) = self.cfg.clean_manifests.get(idx).cloned() {
                    let lp = config::logs_dir()
                        .join(format!("restore-{}.log", actions::safe_slug(&manifest.project_name)));
                    let title = format!("Restaurar: {}", manifest.project_name);
                    self.last_task_log = Some((title.clone(), lp.clone()));
                    self.bg_progress = Some(0.1);
                    actions::restore_project_from_manifest(
                        manifest,
                        lp.clone(),
                        self.tx.clone(),
                        self.hwnd.0 as isize,
                    );
                    self.open_console_modal(Some((title, lp, None)));
                }
            }
            // Live Console Modal
            Hit::ConsoleSelectSource(idx) => {
                if idx < self.con_sources.len() {
                    self.con_source_idx = idx;
                    self.con_scroll = 0;
                    self.con_msg = None;
                    self.refresh_console_lines();
                    self.kick();
                }
            }
            Hit::ConsoleCopyLastError => {
                if let Some((_, lp, _)) = self.con_sources.get(self.con_source_idx) {
                    let err_txt = actions::extract_last_error_block(lp);
                    if !err_txt.is_empty() && actions::write_clipboard_text(self.hwnd, &err_txt) {
                        actions::play_ui_sound("save", self.cfg.sounds_enabled);
                        self.con_msg = Some((
                            "Ultimo bloque de error copiado (listo para pegar)".into(),
                            false,
                        ));
                    } else {
                        self.con_msg = Some(("El log no tiene líneas para copiar".into(), true));
                    }
                    self.kick();
                }
            }
            Hit::ConsoleCopyAll => {
                let full = self.con_lines.join("\n");
                if !full.is_empty() && actions::write_clipboard_text(self.hwnd, &full) {
                    actions::play_ui_sound("click", self.cfg.sounds_enabled);
                    self.con_msg = Some(("Log completo copiado al portapapeles".into(), false));
                } else {
                    self.con_msg = Some(("Consola vacía".into(), true));
                }
                self.kick();
            }
            Hit::ConsoleClear => {
                if let Some((_, lp, _)) = self.con_sources.get(self.con_source_idx).cloned() {
                    let _ = actions::clear_log_file(&lp);
                    self.con_scroll = 0;
                    self.refresh_console_lines();
                    self.con_msg = Some(("Consola limpiada".into(), false));
                    self.kick();
                }
            }
            Hit::ConsoleRestartServer => {
                let cur_cwd = self
                    .con_sources
                    .get(self.con_source_idx)
                    .and_then(|s| s.2.clone());
                if let Some(cwd_str) = cur_cwd {
                    let mut target_opt = None;
                    let mut p_name = Path::new(&cwd_str)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Proyecto".into());
                    for p in &self.cfg.projects {
                        if let Some(fs) = self.fullstack_map.get(&p.path) {
                            for t in [&fs.main, &fs.front, &fs.back].into_iter().flatten() {
                                if t.cwd.to_string_lossy().eq_ignore_ascii_case(&cwd_str) {
                                    target_opt = Some(t.clone());
                                    p_name = p.name.clone();
                                    break;
                                }
                            }
                        }
                    }
                    self.stop_single_target(Path::new(&cwd_str));
                    if let Some(t) = target_opt {
                        self.start_single_target(&p_name, &t);
                        if let Some(srv) = self.servers.get(&cwd_str) {
                            let lp = srv.log_path.clone();
                            self.rebuild_console_sources(Some((p_name, lp, Some(cwd_str))));
                            self.con_source_idx = 0;
                            self.con_scroll = 0;
                            self.refresh_console_lines();
                            self.con_msg = Some(("Servidor reiniciado".into(), false));
                        }
                    } else {
                        self.con_msg = Some(("Servidor detenido".into(), false));
                    }
                    self.kick();
                }
            }
            Hit::ConsoleOpenExternal => {
                if let Some((lbl, lp, cwd_opt)) = self.con_sources.get(self.con_source_idx).cloned() {
                    let proj_p = cwd_opt.as_deref().map(Path::new);
                    actions::open_log_in_terminal(&lbl, proj_p, &lp);
                    self.minimize_preserving_state();
                }
            }
            Hit::ConsoleStopTask => {
                if let Some((_, lp, _)) = self.con_sources.get(self.con_source_idx).cloned() {
                    if let Some(pos) = self.active_tasks.iter().position(|t| t.log_path == lp) {
                        let task = self.active_tasks.remove(pos);
                        actions::stop_dev_server(task.pid);
                        self.con_msg = Some((format!("{} detenido", task.short_label), false));
                        actions::play_ui_sound("click", self.cfg.sounds_enabled);
                        self.kick();
                    }
                }
            }
            Hit::TaskLog(idx) => {
                if let Some(task) = self.active_tasks.get(idx).cloned() {
                    self.open_console_modal(Some((task.title, task.log_path, None)));
                }
            }
            Hit::KillTask(idx) => {
                if idx < self.active_tasks.len() {
                    let task = self.active_tasks.remove(idx);
                    actions::stop_dev_server(task.pid);
                    actions::play_ui_sound("click", self.cfg.sounds_enabled);
                    if self.view == View::ConsoleModal {
                        self.con_msg = Some((format!("{} detenido", task.short_label), false));
                    } else {
                        let _ = self.tx.send(WorkMsg::Toast {
                            title: format!("{} detenido", task.short_label),
                            sub: "Proceso finalizado correctamente".into(),
                            is_err: false,
                            log_path: None,
                        });
                    }
                    self.kick();
                }
            }
            // Quick Runner Modal
            Hit::RunnerExec(idx) => {
                if let Some(item) = self.run_items.get(idx).cloned() {
                    let proj_name = self
                        .cfg
                        .projects
                        .get(self.run_proj_idx)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Proyecto".into());
                    let lp = actions::run_runner_item_logged(
                        &proj_name,
                        &item,
                        self.tx.clone(),
                        self.hwnd.0 as isize,
                    );
                    let title = format!("Runner: {}", item.label);
                    self.last_task_log = Some((title.clone(), lp.clone()));
                    self.open_console_modal(Some((title, lp, None)));
                }
            }
            Hit::RunnerOpenEnv => {
                let idx = self.run_proj_idx;
                self.open_env_modal(idx);
            }
            Hit::RunnerRestoreDeps => {
                if let Some(p) = self.cfg.projects.get(self.run_proj_idx) {
                    if let Some(m_idx) = self
                        .cfg
                        .clean_manifests
                        .iter()
                        .position(|m| m.project_path.eq_ignore_ascii_case(&p.path))
                    {
                        self.click(Hit::CleanerRestore(m_idx));
                    }
                }
            }
        }
    }

    pub(crate) fn save_env_var_from_inputs(&mut self) {
        let k = self.env_key_in.trim().to_string();
        let v = self.env_val_in.clone();
        if k.is_empty() {
            self.env_msg = Some(("Escribe el nombre de la variable (ej. DATABASE_URL)".into(), true));
            self.kick();
            return;
        }
        if let Some(pos) = self.env_vars.iter().position(|(ek, _)| ek == &k) {
            self.env_vars[pos].1 = v;
        } else {
            self.env_vars.push((k.clone(), v));
        }
        if let Some(p) = self.cfg.projects.get(self.env_proj_idx) {
            match actions::write_project_env(Path::new(&p.path), &self.env_filename, &self.env_vars) {
                Ok(()) => {
                    actions::play_ui_sound("save", self.cfg.sounds_enabled);
                    self.env_key_in.clear();
                    self.env_val_in.clear();
                    self.env_focus = 0;
                    self.env_msg = Some((format!("{k} guardada en {}", self.env_filename), false));
                }
                Err(e) => {
                    self.env_msg = Some((format!("Error al guardar: {e}"), true));
                }
            }
        }
        self.kick();
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
                215,
                w!("Panel de Scripts y Herramientas (Ctrl+R)"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                216,
                w!("Variables de Entorno (.env y Bóveda) (Ctrl+K)"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                217,
                w!("Consola de Logs en Vivo (Ctrl+L)"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                218,
                w!("Limpiar cachés guardando receta (Ctrl+D)"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                210,
                w!("Editar nota / recordatorio rápido (Mini-TODO)"),
            );

            if let Ok(ws_menu) = CreatePopupMenu() {
                let cur_ws = p.workspace.as_deref().unwrap_or("");
                for (k, w) in self.cfg.workspaces.iter().enumerate() {
                    let is_sel = cur_ws.eq_ignore_ascii_case(&w.name) || cur_ws.eq_ignore_ascii_case(&w.id);
                    let flags = if is_sel { MF_STRING | MF_CHECKED } else { MF_STRING };
                    let _ = AppendMenuW(ws_menu, flags, 500 + k, &HSTRING::from(w.name.as_str()));
                }
                let _ = AppendMenuW(ws_menu, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(ws_menu, MF_STRING, 598, w!("+ Crear nuevo grupo..."));
                let is_none = cur_ws.is_empty();
                let _ = AppendMenuW(
                    ws_menu,
                    if is_none { MF_STRING | MF_CHECKED } else { MF_STRING },
                    599,
                    w!("Sin grupo"),
                );
                let _ = AppendMenuW(menu, MF_POPUP, ws_menu.0 as usize, w!("Asignar a Grupo de Trabajo"));
            }
            if p.linked_path.is_some() {
                let _ = AppendMenuW(
                    menu,
                    MF_STRING,
                    212,
                    w!("Desvincular par Full-Stack (Front/Back)"),
                );
            } else {
                let _ = AppendMenuW(
                    menu,
                    MF_STRING,
                    211,
                    w!("Vincular con otro proyecto (Par Front + Back)"),
                );
            }

            if let Some(fs) = &fs_info {
                if fs.is_fullstack() {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                    let _ = AppendMenuW(menu, MF_STRING, 310, w!("Iniciar / Detener solo Frontend"));
                    let _ = AppendMenuW(menu, MF_STRING, 311, w!("Iniciar / Detener solo Backend"));
                    let _ = AppendMenuW(menu, MF_STRING, 312, w!("Iniciar / Detener Ambos (Combo Full-Stack)"));
                }
            }

            if is_srv_running {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(menu, MF_STRING, 302, w!("Ver terminal del servidor en vivo"));
                let _ = AppendMenuW(menu, MF_STRING, 300, w!("Detener servidor local"));
            }

            if !scripts.is_empty() {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                for (idx, (s_name, _)) in scripts.iter().take(8).enumerate() {
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING,
                        400 + idx,
                        &HSTRING::from(format!("Ejecutar: {pm} run {s_name}")),
                    );
                }
            }

            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(menu, MF_STRING, 201, w!("Quitar proyecto solo de la lista"));
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                204,
                w!("Eliminar proyecto por completo (borrar carpeta del disco)"),
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
            } else if cmd == 215 {
                self.open_runner_modal(i);
            } else if cmd == 216 {
                self.open_env_modal(i);
            } else if cmd == 217 {
                let pref = srv_log.map(|lp| {
                    let lbl = srv_port
                        .map(|pt| format!("{} (:{pt})", p.name))
                        .unwrap_or_else(|| p.name.clone());
                    (lbl, lp, Some(p.path.clone()))
                });
                self.open_console_modal(pref);
            } else if cmd == 218 {
                self.open_cleaner_modal();
            } else if cmd == 210 {
                self.note_proj_idx = i;
                self.note_text = p.note.clone().unwrap_or_default();
                self.set_view(View::NoteModal);
            } else if (500..598).contains(&cmd) {
                let ws_idx = cmd - 500;
                if let Some(w) = self.cfg.workspaces.get(ws_idx) {
                    let w_name = w.name.clone();
                    if let Some(proj) = self.cfg.projects.get_mut(i) {
                        proj.workspace = Some(w_name);
                    }
                    actions::play_ui_sound("tab", self.cfg.sounds_enabled);
                    self.save();
                    self.kick();
                }
            } else if cmd == 598 {
                self.ws_edit_idx = None;
                self.ws_name.clear();
                self.ws_icon = G_WORK.into();
                self.set_view(View::WorkspaceModal);
            } else if cmd == 599 {
                if let Some(proj) = self.cfg.projects.get_mut(i) {
                    proj.workspace = None;
                }
                actions::play_ui_sound("tab", self.cfg.sounds_enabled);
                self.save();
                self.kick();
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
                let pref = srv_log.map(|lp| {
                    let lbl = srv_port
                        .map(|pt| format!("{} (:{pt})", p.name))
                        .unwrap_or_else(|| p.name.clone());
                    (lbl, lp, Some(p.path.clone()))
                });
                self.open_console_modal(pref);
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

    fn workspace_menu(&mut self, k: usize) {
        let Some(w) = self.cfg.workspaces.get(k).cloned() else { return };
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            let _ = AppendMenuW(menu, MF_STRING, 1, &HSTRING::from(format!("Editar grupo \"{}\"", w.name)));
            let _ = AppendMenuW(menu, MF_STRING, 2, &HSTRING::from(format!("Eliminar grupo \"{}\"", w.name)));
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
            match cmd.0 {
                1 => {
                    self.click(Hit::EditWorkspace(k));
                }
                2 => {
                    self.click(Hit::WsDelete(k));
                }
                _ => {}
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
            if let Ok(pos_sub) = CreatePopupMenu() {
                let cur_pos = self.island_pos();
                for (pi, pos) in IslandPos::ALL.iter().enumerate() {
                    let flags = if *pos == cur_pos {
                        MF_STRING | MF_CHECKED
                    } else {
                        MF_STRING
                    };
                    let _ = AppendMenuW(pos_sub, flags, 20 + pi, &HSTRING::from(pos.label()));
                }
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP,
                    pos_sub.0 as usize,
                    w!("Posición en pantalla"),
                );
            }
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                26,
                if self.cfg.light_mode {
                    w!("Cambiar a Modo Oscuro")
                } else {
                    w!("Cambiar a Modo Blanco")
                },
            );
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
                c if (20..26).contains(&c) => {
                    if let Some(&pos) = IslandPos::ALL.get((c - 20) as usize) {
                        self.click(Hit::SetPosition(pos));
                    }
                }
                26 => self.click(Hit::ToggleTheme),
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

    fn pop_word(s: &mut String) {
        while s.chars().last().is_some_and(|c| c.is_whitespace()) {
            s.pop();
        }
        while let Some(c) = s.chars().last() {
            if c.is_whitespace() || matches!(c, '/' | '\\' | '-' | '_') {
                break;
            }
            s.pop();
        }
        while s.chars().last().is_some_and(|c| matches!(c, '/' | '\\' | '-' | '_')) {
            s.pop();
        }
    }

    fn push_clipboard_clean(
        dst: &mut String,
        txt: &str,
        max_chars: usize,
        allow_newlines: bool,
        sanitize_folder: bool,
    ) -> bool {
        let cur_len = dst.chars().count();
        let rem = max_chars.saturating_sub(cur_len);
        if rem == 0 {
            return false;
        }
        let clean: String = if sanitize_folder {
            txt.chars()
                .filter(|c| !c.is_control() && !"\\/:*?\"<>|".contains(*c))
                .map(|c| if c == ' ' { '-' } else { c })
                .take(rem)
                .collect()
        } else if allow_newlines {
            txt.chars()
                .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                .take(rem)
                .collect()
        } else {
            txt.chars().filter(|c| !c.is_control()).take(rem).collect()
        };
        if clean.is_empty() {
            return false;
        }
        dst.push_str(&clean);
        true
    }

    fn selected_project_idx(&self) -> Option<usize> {
        let filtered = self.filtered_projects();
        filtered
            .get(self.sel_idx.min(filtered.len().saturating_sub(1)))
            .copied()
    }

    fn all_filter_modes(&self) -> Vec<FilterMode> {
        let mut modes = Vec::with_capacity(5 + self.cfg.workspaces.len());
        modes.push(FilterMode::All);
        for k in 0..self.cfg.workspaces.len() {
            modes.push(FilterMode::Workspace(k));
        }
        modes.push(FilterMode::Pinned);
        modes.push(FilterMode::GitDirty);
        modes.push(FilterMode::RunningServer);
        modes.push(FilterMode::FullStack);
        modes
    }

    fn key(&mut self, vk: u32) -> Option<isize> {
        let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
        match vk {
            x if x == VK_ESCAPE.0 as u32 => self.back(),
            x if x == VK_TAB.0 as u32 && self.view == View::CustomEditor => {
                self.ed_focus = (self.ed_focus + 1) % 2;
                self.kick();
            }
            x if x == VK_TAB.0 as u32 && self.view == View::EnvModal => {
                self.env_focus = (self.env_focus + 1) % 2;
                self.kick();
            }
            x if x == VK_RETURN.0 as u32 && ctrl && self.view == View::Expanded => {
                if let Some(proj_idx) = self.selected_project_idx() {
                    self.toggle_project_server(proj_idx);
                }
            }
            x if x == VK_RETURN.0 as u32 => match self.view {
                View::Wizard => self.wizard_next(),
                View::GitModal => self.click(Hit::GitDoCommitPush),
                View::WorkspaceModal => self.click(Hit::WsSave),
                View::NoteModal => {
                    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                    if shift {
                        if self.note_text.chars().count() < 1000 {
                            self.note_text.push('\n');
                            self.kick();
                        }
                    } else {
                        self.save_project_note();
                    }
                }
                View::CustomEditor => self.save_custom_editor_item(),
                View::EnvModal => self.save_env_var_from_inputs(),
                View::Expanded => {
                    if let Some(proj_idx) = self.selected_project_idx() {
                        self.open_project(proj_idx, None);
                    }
                }
                _ => {}
            },
            x if (x == VK_RIGHT.0 as u32 || x == VK_LEFT.0 as u32) && self.view == View::Expanded => {
                let modes = self.all_filter_modes();
                let pos = modes.iter().position(|&m| m == self.filter_mode).unwrap_or(0);
                let next_pos = if x == VK_RIGHT.0 as u32 {
                    (pos + 1) % modes.len()
                } else {
                    (pos + modes.len() - 1) % modes.len()
                };
                self.filter_mode = modes[next_pos];
                self.sel_idx = 0;
                self.scroll = 0;
                actions::play_ui_sound("tab", self.cfg.sounds_enabled);
                self.kick();
            }
            x if x == VK_DOWN.0 as u32 && self.view == View::Expanded => {
                let len = self.filtered_projects().len();
                if len > 0 {
                    if self.sel_idx + 1 < len {
                        self.sel_idx += 1;
                    } else {
                        self.sel_idx = 0;
                    }
                    if self.sel_idx >= self.scroll + MAX_ROWS {
                        self.scroll = self.sel_idx + 1 - MAX_ROWS;
                    } else if self.sel_idx < self.scroll {
                        self.scroll = self.sel_idx;
                    }
                    self.hover = None;
                    self.kick();
                }
            }
            x if x == VK_UP.0 as u32 && self.view == View::Expanded => {
                let len = self.filtered_projects().len();
                if len > 0 {
                    if self.sel_idx > 0 {
                        self.sel_idx -= 1;
                    } else {
                        self.sel_idx = len - 1;
                    }
                    if self.sel_idx < self.scroll {
                        self.scroll = self.sel_idx;
                    } else if self.sel_idx >= self.scroll + MAX_ROWS {
                        self.scroll = self.sel_idx + 1 - MAX_ROWS;
                    }
                    self.hover = None;
                    self.kick();
                }
            }
            x if x == VK_BACK.0 as u32 && ctrl => {
                match self.view {
                    View::Expanded => {
                        Self::pop_word(&mut self.search);
                        self.sel_idx = 0;
                        self.scroll = 0;
                        self.kick();
                    }
                    View::GitModal => {
                        Self::pop_word(&mut self.git_commit_msg);
                        self.kick();
                    }
                    View::NoteModal => {
                        Self::pop_word(&mut self.note_text);
                        self.kick();
                    }
                    View::WorkspaceModal => {
                        Self::pop_word(&mut self.ws_name);
                        self.kick();
                    }
                    View::CustomEditor => {
                        let target = if self.ed_focus == 0 {
                            &mut self.ed_f1
                        } else {
                            &mut self.ed_f2
                        };
                        Self::pop_word(target);
                        self.ed_msg = None;
                        self.kick();
                    }
                    View::EnvModal => {
                        let target = if self.env_focus == 0 {
                            &mut self.env_key_in
                        } else {
                            &mut self.env_val_in
                        };
                        Self::pop_word(target);
                        self.env_msg = None;
                        self.kick();
                    }
                    View::Wizard if self.step == Step::Name => {
                        Self::pop_word(&mut self.wiz_name);
                        self.wiz_err = None;
                        self.kick();
                    }
                    _ => return None,
                }
            }
            0x31..=0x39 if ctrl && self.view == View::Expanded => {
                let filtered = self.filtered_projects();
                if let Some(&proj_idx) = filtered.get((vk - 0x31) as usize) {
                    self.open_project(proj_idx, None);
                }
            }
            0x44 if ctrl && self.view == View::Expanded => self.open_cleaner_modal(),
            0x45 if ctrl && self.view == View::Expanded => {
                if let Some(proj_idx) = self.selected_project_idx() {
                    self.click(Hit::ProjExplorer(proj_idx));
                }
            }
            0x47 if ctrl && self.view == View::Expanded => {
                if let Some(proj_idx) = self.selected_project_idx() {
                    self.click(Hit::ProjGit(proj_idx));
                }
            }
            0x4B if ctrl && self.view == View::Expanded => {
                if let Some(proj_idx) = self.selected_project_idx() {
                    self.open_env_modal(proj_idx);
                }
            }
            0x4C if ctrl && self.view == View::Expanded => self.open_console_modal(None),
            0x4E if ctrl && self.view == View::Expanded => self.start_wizard(),
            0x50 if ctrl && self.view == View::Expanded => {
                if let Some(proj_idx) = self.selected_project_idx() {
                    self.toggle_pin(proj_idx);
                }
            }
            0x52 if ctrl && self.view == View::Expanded => {
                if let Some(proj_idx) = self.selected_project_idx() {
                    self.open_runner_modal(proj_idx);
                }
            }
            0x53 if ctrl && self.view == View::Expanded => {
                if let Some(proj_idx) = self.selected_project_idx() {
                    self.toggle_project_server(proj_idx);
                }
            }
            0x54 if ctrl && self.view == View::Expanded => {
                if let Some(proj_idx) = self.selected_project_idx() {
                    self.click(Hit::ProjTerm(proj_idx));
                }
            }
            0x56 if ctrl => {
                if let Some(txt) = actions::read_clipboard_text(self.hwnd) {
                    let changed = match self.view {
                        View::Expanded => {
                            let ok = Self::push_clipboard_clean(&mut self.search, &txt, 120, false, false);
                            if ok {
                                self.sel_idx = 0;
                                self.scroll = 0;
                            }
                            ok
                        }
                        View::GitModal => Self::push_clipboard_clean(&mut self.git_commit_msg, &txt, 80, false, false),
                        View::NoteModal => Self::push_clipboard_clean(&mut self.note_text, &txt, 1000, true, false),
                        View::WorkspaceModal => Self::push_clipboard_clean(&mut self.ws_name, &txt, 24, false, false),
                        View::CustomEditor => {
                            let target = if self.ed_focus == 0 {
                                &mut self.ed_f1
                            } else {
                                &mut self.ed_f2
                            };
                            let ok = Self::push_clipboard_clean(target, &txt, 120, false, false);
                            if ok {
                                self.ed_msg = None;
                            }
                            ok
                        }
                        View::EnvModal => {
                            let clean_line = txt.trim();
                            if self.env_focus == 0
                                && self.env_key_in.is_empty()
                                && !clean_line.contains('\n')
                                && clean_line.contains('=')
                            {
                                if let Some(eq) = clean_line.find('=') {
                                    let k = clean_line[..eq].trim();
                                    let v = clean_line[eq + 1..].trim();
                                    let ok1 = Self::push_clipboard_clean(&mut self.env_key_in, k, 64, false, false);
                                    let ok2 = Self::push_clipboard_clean(&mut self.env_val_in, v, 240, false, false);
                                    if ok1 || ok2 {
                                        self.env_msg = None;
                                    }
                                    ok1 || ok2
                                } else {
                                    false
                                }
                            } else {
                                let (target, max_c) = if self.env_focus == 0 {
                                    (&mut self.env_key_in, 64)
                                } else {
                                    (&mut self.env_val_in, 240)
                                };
                                let ok = Self::push_clipboard_clean(target, clean_line, max_c, false, false);
                                if ok {
                                    self.env_msg = None;
                                }
                                ok
                            }
                        }
                        View::Wizard if self.step == Step::Name => {
                            let ok = Self::push_clipboard_clean(&mut self.wiz_name, &txt, 60, false, true);
                            if ok {
                                self.wiz_err = None;
                            }
                            ok
                        }
                        _ => false,
                    };
                    if changed {
                        self.kick();
                    }
                }
            }
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
                    if !ch.is_control() && self.search.chars().count() < 120 {
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
                    if !ch.is_control() && self.note_text.chars().count() < 1000 {
                        self.note_text.push(ch);
                        self.kick();
                    }
                    return Some(0);
                }
            },
            View::WorkspaceModal => match c {
                8 => {
                    self.ws_name.pop();
                    self.kick();
                    return Some(0);
                }
                13 | 27 | 9 => return Some(0),
                _ => {
                    let Some(ch) = char::from_u32(c) else { return Some(0) };
                    if !ch.is_control() && self.ws_name.chars().count() < 24 {
                        self.ws_name.push(ch);
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
                        if !ch.is_control() && target.chars().count() < 120 {
                            target.push(ch);
                            self.ed_msg = None;
                            self.kick();
                        }
                        return Some(0);
                    }
                }
            }
            View::EnvModal => {
                let is_key = self.env_focus == 0;
                let (target, max_c) = if is_key {
                    (&mut self.env_key_in, 64)
                } else {
                    (&mut self.env_val_in, 240)
                };
                match c {
                    8 => {
                        target.pop();
                        self.env_msg = None;
                        self.kick();
                        return Some(0);
                    }
                    13 | 27 | 9 => return Some(0),
                    _ => {
                        let Some(ch) = char::from_u32(c) else { return Some(0) };
                        if !ch.is_control() && target.chars().count() < max_c {
                            if is_key && ch == ' ' {
                                target.push('_');
                            } else {
                                target.push(ch);
                            }
                            self.env_msg = None;
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
                    } else if wp.0 == TIMER_TOPMOST {
                        self.update_fullscreen_and_pos();
                    } else if wp.0 == TIMER_CONSOLE && self.view == View::ConsoleModal {
                        if self.refresh_console_lines() {
                            self.kick();
                        }
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
                WM_LBUTTONDOWN => {
                    if self.view == View::Compact {
                        let (x, y) = self.mpos(lp);
                        if self.hit_at(x, y) == Some(Hit::Pill) {
                            self.swallow_lbuttonup = true;
                            self.click(Hit::Pill);
                        }
                    }
                    Some(0)
                }
                WM_LBUTTONUP => {
                    if self.swallow_lbuttonup {
                        self.swallow_lbuttonup = false;
                        return Some(0);
                    }
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
                        | Some(Hit::ProjPin(i))
                        | Some(Hit::ProjRunner(i))
                        | Some(Hit::ProjEnv(i)) => self.project_menu(i),
                        Some(Hit::FilterChip(FilterMode::Workspace(k))) | Some(Hit::EditWorkspace(k)) => {
                            self.workspace_menu(k);
                        }
                        Some(Hit::PortBadge(port)) => self.click(Hit::KillPort(port)),
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
                        View::EnvModal => {
                            let list_len = if self.env_tab == 0 {
                                self.env_vars.len()
                            } else {
                                self.cfg.env_vault.len()
                            };
                            let max = list_len.saturating_sub(5);
                            self.env_scroll = if d < 0 {
                                (self.env_scroll + 1).min(max)
                            } else {
                                self.env_scroll.saturating_sub(1)
                            };
                            self.kick();
                        }
                        View::CleanerModal => {
                            let list_len = if self.clean_tab == 0 {
                                self.clean_items.len()
                            } else {
                                self.cfg.clean_manifests.len()
                            };
                            let max = list_len.saturating_sub(5);
                            self.clean_scroll = if d < 0 {
                                (self.clean_scroll + 1).min(max)
                            } else {
                                self.clean_scroll.saturating_sub(1)
                            };
                            self.kick();
                        }
                        View::ConsoleModal => {
                            let max = self.con_lines.len().saturating_sub(16);
                            self.con_scroll = if d > 0 {
                                (self.con_scroll + 3).min(max)
                            } else {
                                self.con_scroll.saturating_sub(3)
                            };
                            self.kick();
                        }
                        View::RunnerModal => {
                            let max = self.run_items.len().saturating_sub(6);
                            self.run_scroll = if d < 0 {
                                (self.run_scroll + 1).min(max)
                            } else {
                                self.run_scroll.saturating_sub(1)
                            };
                            self.kick();
                        }
                        _ => {}
                    }
                    Some(0)
                }
                WM_SETCURSOR => {
                    let cur = if self.hover.is_some() { IDC_HAND } else { IDC_ARROW };
                    let _ = SetCursor(LoadCursorW(None, cur).unwrap_or_default());
                    Some(1)
                }
                WM_KEYDOWN => self.key(wp.0 as u32),
                WM_CHAR => self.chr(wp.0 as u32),
                WM_ACTIVATE => {
                    if (wp.0 & 0xFFFF) as u32 == WA_INACTIVE {
                        if self.hover.is_some() {
                            self.hover = None;
                            self.tracking = false;
                            self.glow_anim = 0.0;
                            self.kick();
                        }
                        if Self::is_expanded_panel(self.view) {
                            self.minimize_preserving_state();
                        } else if self.view == View::Status && self.skind == SKind::Creating {
                            // Si el usuario sigue trabajando en otra app mientras crea o hace git push,
                            // se contrae a la gota mostrando la barra de progreso luminosa en el borde curvo
                            self.resume_view = Some(View::Status);
                            self.set_view(View::Compact);
                        }
                    }
                    if self.visible && !self.auto_hidden_fs {
                        actions::ensure_island_above_taskbar(self.hwnd);
                    }
                    None
                }
                WM_WINDOWPOSCHANGING => {
                    let ppos = lp.0 as *mut WINDOWPOS;
                    if !ppos.is_null() {
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
                WM_QUERYENDSESSION => Some(1),
                WM_CLOSE | WM_DESTROY | WM_ENDSESSION => {
                    self.quit();
                    Some(0)
                }
                _ => None,
            }
        }
    }
}
