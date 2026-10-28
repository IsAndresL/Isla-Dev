use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::{self, Launcher, Library, Project, Template};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const WM_WORKER: u32 = 0x8000 + 1;

#[derive(Clone, Debug, Default)]
pub struct GitInfo {
    pub branch: String,
    pub dirty: bool,
    pub changed_count: usize,
    pub remote_url: Option<String>,
}

pub enum WorkMsg {
    Progress(String),
    Done {
        res: std::result::Result<PathBuf, String>,
        warn: Option<(String, PathBuf)>,
    },
    GitStates(HashMap<String, GitInfo>),
    ActivePorts(Vec<u16>),
    ServerPort {
        path: String,
        pid: u32,
        port: u16,
    },
    ServerExited {
        path: String,
        pid: u32,
        err_reason: Option<String>,
        log_path: Option<PathBuf>,
    },
    Toast {
        title: String,
        sub: String,
        is_err: bool,
        log_path: Option<PathBuf>,
    },
}

pub fn notify_hwnd(hwnd: isize) {
    unsafe {
        let _ = PostMessageW(HWND(hwnd as _), WM_WORKER, WPARAM(0), LPARAM(0));
    }
}

pub fn expand(t: &str, name: &str, path: &Path, parent: &Path) -> String {
    t.replace("{path}", &path.to_string_lossy())
        .replace("{name}", name)
        .replace("{parent}", &parent.to_string_lossy())
}

pub fn safe_slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Limpia códigos ANSI de escape para poder leer puertos o mensajes de error legibles.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

/// Extrae el último mensaje de error significativo de un archivo de log.
pub fn extract_last_error_reason(log_path: &Path) -> String {
    let Ok(content) = fs::read_to_string(log_path) else {
        return "Fallo al ejecutar comando (clic para ver terminal)".into();
    };
    let mut best = None;
    for raw in content.lines().rev() {
        let line = strip_ansi(raw).trim().to_string();
        if line.is_empty()
            || line.starts_with("===")
            || line.starts_with("npm notice")
            || line.contains("A complete log of this run")
        {
            continue;
        }
        let low = line.to_lowercase();
        if low.contains("error")
            || low.contains("err!")
            || low.contains("eaddrinuse")
            || low.contains("cannot find")
            || low.contains("failed")
            || low.contains("fatal:")
            || low.contains("no se reconoce")
            || low.contains("is not recognized")
            || low.contains("enosys")
            || low.contains("eacces")
        {
            let clean = line
                .trim_start_matches("npm error ")
                .trim_start_matches("npm ERR! ")
                .trim_start_matches("Error: ")
                .trim_start_matches("error: ")
                .trim_start_matches("fatal: ")
                .trim();
            if !clean.is_empty() {
                best = Some(short(clean));
                break;
            }
        }
        if best.is_none() && !line.starts_with('>') {
            best = Some(short(&line));
        }
    }
    best.unwrap_or_else(|| "El comando terminó con error".into())
}

/// Localiza la ruta real del ejecutable o script de Antigravity IDE (editor de código).
pub fn find_antigravity_executable() -> Option<String> {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let candidates = [
            format!("{}\\Programs\\Antigravity IDE\\bin\\antigravity-ide.cmd", local),
            format!("{}\\Programs\\Antigravity IDE\\bin\\antigravity-ide", local),
            format!("{}\\Programs\\Antigravity IDE\\Antigravity IDE.exe", local),
        ];
        for c in candidates {
            if Path::new(&c).is_file() {
                return Some(c);
            }
        }
    }
    for name in &["antigravity-ide.cmd", "antigravity-ide"] {
        if let Ok(out) = Command::new("where").arg(name).creation_flags(CREATE_NO_WINDOW).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                for line in s.lines() {
                    let p = Path::new(line.trim());
                    if p.is_file() {
                        return Some(line.trim().to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn resolve_command(raw: &str) -> String {
    let t = raw.trim();
    if t.starts_with("antigravity-ide") || t.starts_with("antigravity") {
        if let Some(real_path) = find_antigravity_executable() {
            let prefix = if t.starts_with("antigravity-ide") {
                "antigravity-ide"
            } else {
                "antigravity"
            };
            return raw.replacen(prefix, &format!("\"{}\"", real_path), 1);
        }
    }
    raw.to_string()
}

fn cmd(line: &str) -> Command {
    let mut c = Command::new("cmd");
    c.raw_arg(format!("/S /C \"{line}\""))
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null());
    c
}

pub fn run_detached(line: &str, cwd: &Path) -> std::io::Result<()> {
    let mut c = cmd(line);
    if cwd.exists() {
        c.current_dir(cwd);
    }
    c.stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|_| ())
}

pub fn launch(l: &Launcher, path: &str) -> std::result::Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err("La carpeta ya no existe".into());
    }
    let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let parent = p.parent().unwrap_or(p);

    if l.id == "antigravity" || l.command.trim().starts_with("antigravity") {
        if let Some(exe) = find_antigravity_executable() {
            let mut c = Command::new("cmd");
            c.args(["/C", "call", &exe, path])
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if p.exists() {
                c.current_dir(p);
            }
            return c.spawn().map(|_| ()).map_err(|e| e.to_string());
        } else {
            let mut c = Command::new("cmd");
            c.args(["/C", "antigravity-ide", path])
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if p.exists() {
                c.current_dir(p);
            }
            return c.spawn().map(|_| ()).map_err(|e| e.to_string());
        }
    }

    let expanded = expand(&l.command, &name, p, parent);
    let resolved = resolve_command(&expanded);
    run_detached(&resolved, p).map_err(|e| e.to_string())
}

pub fn open_file(path: &Path) {
    let _ = Command::new("notepad").arg(path).spawn();
}

/// Abre una ventana de terminal mostrando en vivo el contenido de un archivo de log (comando ejecutado + salida/errores).
pub fn open_log_in_terminal(title_label: &str, project_path: Option<&Path>, log_path: &Path) {
    let win_title = format!("IslaDev Terminal - {title_label}");
    unsafe {
        let title_h = HSTRING::from(&win_title);
        let mut hw = FindWindowW(w!("ConsoleWindowClass"), &title_h).unwrap_or_default();
        if hw.0.is_null() {
            hw = FindWindowW(None, &title_h).unwrap_or_default();
        }
        if !hw.0.is_null() {
            let _ = ShowWindow(hw, SW_SHOW);
            let _ = ShowWindow(hw, SW_RESTORE);
            let _ = SetForegroundWindow(hw);
            return;
        }
    }

    let log_str = log_path.to_string_lossy().replace('\'', "''");
    let cwd_str = project_path
        .map(|p| p.to_string_lossy().replace('\'', "''"))
        .unwrap_or_else(|| ".".into());
    let title_esc = win_title.replace('\'', "''");

    let ps_cmd = format!(
        "$Host.UI.RawUI.WindowTitle = '{title_esc}'; \
         if (Test-Path '{cwd_str}') {{ Set-Location '{cwd_str}' }}; \
         if (Test-Path '{log_str}') {{ Get-Content -Path '{log_str}' -Encoding UTF8 -Wait }} \
         else {{ Write-Host 'Aún no hay registros en el log.' }}"
    );

    let _ = Command::new("conhost.exe")
        .args(["powershell.exe", "-NoLogo", "-NoExit", "-Command", &ps_cmd])
        .spawn();
}

fn run_logged(line: &str, cwd: &Path, log: &mut File, log_path: &Path) -> std::result::Result<(), String> {
    let _ = writeln!(log, "\n==========================================================");
    let _ = writeln!(log, "$ {line}");
    let _ = writeln!(log, "  Directorio: {}", cwd.display());
    let _ = writeln!(log, "==========================================================");
    let out = log.try_clone().map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    let st = cmd(line)
        .current_dir(cwd)
        .env("CI", "1")
        .stdout(out)
        .stderr(err)
        .status()
        .map_err(|e| e.to_string())?;
    if st.success() {
        Ok(())
    } else {
        let reason = extract_last_error_reason(log_path);
        Err(format!("{} ({})", short(line), reason))
    }
}

pub fn short(s: &str) -> String {
    if s.chars().count() > 54 {
        format!("{}…", s.chars().take(53).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Escanea la carpeta base y descubre todas las carpetas preexistentes.
pub fn scan_base_dir(base: &Path) -> Vec<Project> {
    let mut list = Vec::new();
    let Ok(entries) = fs::read_dir(base) else { return list };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let Some(fname) = p.file_name() else { continue };
        let name = fname.to_string_lossy().to_string();
        if name.starts_with('.')
            || name.eq_ignore_ascii_case("node_modules")
            || name.eq_ignore_ascii_case("$recycle.bin")
            || name.eq_ignore_ascii_case("system volume information")
        {
            continue;
        }
        let (stack, color) = detect(&p);
        list.push(Project {
            name,
            path: p.to_string_lossy().to_string(),
            stack,
            color,
            launcher: None,
            last_opened: 0,
            pinned: false,
        });
    }
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

/// Crea el proyecto en un hilo de fondo, instala librerías, opcionalmente lo sube a GitHub, y reporta errores detallados.
pub fn scaffold(
    tpl: Template,
    libs: Vec<Library>,
    name: String,
    base: PathBuf,
    auto_github: bool,
    log: PathBuf,
    tx: Sender<WorkMsg>,
    hwnd: isize,
) {
    let post = |tx: &Sender<WorkMsg>, m: WorkMsg| {
        let _ = tx.send(m);
        notify_hwnd(hwnd);
    };
    let mut warnings: Vec<String> = Vec::new();

    let res = (|| -> std::result::Result<PathBuf, String> {
        let proj = base.join(&name);
        fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let mut logf = File::create(&log).map_err(|e| e.to_string())?;
        let v = |s: &str| expand(s, &name, &proj, &base);
        if tpl.create.is_empty() {
            fs::create_dir_all(&proj).map_err(|e| e.to_string())?;
        }
        for c in &tpl.create {
            let cmd_str = v(c);
            post(&tx, WorkMsg::Progress(format!("> {}", short(&cmd_str))));
            run_logged(&cmd_str, &base, &mut logf, &log)?;
        }
        fs::create_dir_all(&proj).map_err(|e| e.to_string())?;
        for c in &tpl.setup {
            let cmd_str = v(c);
            post(&tx, WorkMsg::Progress(format!("> {}", short(&cmd_str))));
            if let Err(e) = run_logged(&cmd_str, &proj, &mut logf, &log) {
                warnings.push(format!("Setup: {e}"));
            }
        }
        for l in &libs {
            let cmd_str = v(&l.command);
            post(&tx, WorkMsg::Progress(format!("Instalando {} · > {}", l.name, short(&cmd_str))));
            if let Err(_) = run_logged(&cmd_str, &proj, &mut logf, &log) {
                let reason = extract_last_error_reason(&log);
                warnings.push(format!("Librería '{}': {}", l.name, reason));
            }
        }
        if auto_github {
            post(&tx, WorkMsg::Progress("Subiendo proyecto a GitHub (git init + commit + gh)…".into()));
            let _ = run_logged("git init", &proj, &mut logf, &log);
            let _ = run_logged("git branch -M main", &proj, &mut logf, &log);
            let _ = run_logged("git add -A", &proj, &mut logf, &log);
            let _ = run_logged("git commit -m \"Initial commit\"", &proj, &mut logf, &log);
            let gh_cmd = format!("gh repo create \"{name}\" --private --source=. --remote=origin --push");
            if let Err(_) = run_logged(&gh_cmd, &proj, &mut logf, &log) {
                let reason = extract_last_error_reason(&log);
                warnings.push(format!("GitHub CLI: {reason} (repositorio Git local inicializado)"));
            }
        }
        Ok(proj)
    })();

    let warn = if warnings.is_empty() {
        None
    } else {
        Some((warnings.join(" | "), log))
    };
    post(&tx, WorkMsg::Done { res, warn });
}

/// Detecta el stack de una carpeta existente.
pub fn detect(path: &Path) -> (String, String) {
    let pj = fs::read_to_string(path.join("package.json")).unwrap_or_default();
    let r = if !pj.is_empty() {
        if pj.contains("\"@angular/core\"") {
            ("Angular", "#DD0031")
        } else if pj.contains("\"astro\"") {
            ("Astro", "#FF5D01")
        } else if pj.contains("\"next\"") {
            ("Next.js", "#71717A")
        } else if pj.contains("\"react\"") && pj.contains("\"vite\"") {
            ("React + Vite", "#3B82F6")
        } else if pj.contains("\"react\"") {
            ("React", "#3B82F6")
        } else if pj.contains("\"vue\"") {
            ("Vue + Vite", "#42B883")
        } else if pj.contains("\"svelte\"") {
            ("Svelte", "#FF3E00")
        } else if pj.contains("\"vite\"") {
            ("TypeScript", "#3178C6")
        } else {
            ("Node.js", "#22C55E")
        }
    } else if path.join("requirements.txt").exists()
        || path.join("pyproject.toml").exists()
        || path.join(".venv").exists()
        || path.join("main.py").exists()
    {
        ("Python", "#3776AB")
    } else if path.join("Cargo.toml").exists() {
        ("Rust", "#CE422B")
    } else if path.join("go.mod").exists() {
        ("Go", "#00ADD8")
    } else {
        ("Solo carpeta", "#6366F1")
    };
    (r.0.into(), r.1.into())
}

// ============================================================================
// 1. GESTIÓN DE GIT Y GITHUB EN TIEMPO REAL
// ============================================================================

pub fn read_git_branch_fast(path: &Path) -> Option<String> {
    let git_entry = path.join(".git");
    let git_dir = if git_entry.is_dir() {
        git_entry
    } else if git_entry.is_file() {
        let s = fs::read_to_string(&git_entry).ok()?;
        let rel = s.trim().strip_prefix("gitdir:")?.trim();
        path.join(rel)
    } else {
        return None;
    };
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let trimmed = head.trim();
    if let Some(rest) = trimmed.strip_prefix("ref: refs/heads/") {
        Some(rest.to_string())
    } else if let Some(rest) = trimmed.strip_prefix("ref: ") {
        Some(rest.rsplit('/').next().unwrap_or(rest).to_string())
    } else if trimmed.len() >= 7 {
        Some(trimmed[..7].to_string())
    } else {
        None
    }
}

pub fn read_git_remote(path: &Path) -> Option<String> {
    let cfg = fs::read_to_string(path.join(".git").join("config")).ok()?;
    for line in cfg.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("url =") {
            let raw = rest.trim();
            if raw.starts_with("http://") || raw.starts_with("https://") {
                return Some(raw.trim_end_matches(".git").to_string());
            } else if let Some(ssh) = raw.strip_prefix("git@") {
                if let Some((host, repo)) = ssh.split_once(':') {
                    return Some(format!("https://{}/{}", host, repo.trim_end_matches(".git")));
                }
            }
        }
    }
    None
}

pub fn inspect_git_all(paths: Vec<String>, tx: Sender<WorkMsg>, hwnd: isize) {
    std::thread::spawn(move || {
        let mut map = HashMap::new();
        for p_str in paths {
            let p = Path::new(&p_str);
            let Some(branch) = read_git_branch_fast(p) else { continue };
            let remote_url = read_git_remote(p);
            let mut dirty = false;
            let mut changed_count = 0;
            if let Ok(out) = Command::new("git")
                .args(["status", "--porcelain", "-uno"])
                .current_dir(p)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    changed_count = s.lines().filter(|l| !l.trim().is_empty()).count();
                    dirty = changed_count > 0;
                }
            }
            map.insert(
                p_str,
                GitInfo {
                    branch,
                    dirty,
                    changed_count,
                    remote_url,
                },
            );
        }
        let _ = tx.send(WorkMsg::GitStates(map));
        notify_hwnd(hwnd);
    });
}

pub fn git_pull(path: PathBuf, name: String, tx: Sender<WorkMsg>, hwnd: isize) {
    std::thread::spawn(move || {
        let log_path = config::logs_dir().join(format!("git-{}.log", safe_slug(&name)));
        let mut logf = File::create(&log_path).ok();
        if let Some(f) = logf.as_mut() {
            let _ = writeln!(f, "$ git pull (en {})", path.display());
        }
        let out = Command::new("git")
            .arg("pull")
            .current_dir(&path)
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if let Some(f) = logf.as_mut() {
                    let _ = writeln!(f, "{s}");
                }
                let msg = if s.contains("Already up to date") || s.contains("Ya está actualizado") {
                    "Ya está actualizado".to_string()
                } else {
                    short(s.lines().next().unwrap_or("Cambios descargados"))
                };
                let _ = tx.send(WorkMsg::Toast {
                    title: format!("Git Pull · {name}"),
                    sub: msg,
                    is_err: false,
                    log_path: Some(log_path),
                });
            }
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                if let Some(f) = logf.as_mut() {
                    let _ = writeln!(f, "{err}");
                }
                let _ = tx.send(WorkMsg::Toast {
                    title: format!("Error Git Pull · {name}"),
                    sub: format!("{} · clic para ver terminal", short(err.lines().next().unwrap_or("Error"))),
                    is_err: true,
                    log_path: Some(log_path),
                });
            }
            Err(e) => {
                let _ = tx.send(WorkMsg::Toast {
                    title: format!("Error Git · {name}"),
                    sub: e.to_string(),
                    is_err: true,
                    log_path: None,
                });
            }
        }
        notify_hwnd(hwnd);
    });
}

/// Hace `git add -A`, `git commit -m "<msg>"`, configura/crea remoto en GitHub si hace falta y hace `git push`.
pub fn git_commit_and_push(
    path: PathBuf,
    name: String,
    commit_msg: String,
    create_gh_repo: bool,
    is_public: bool,
    all_paths: Vec<String>,
    tx: Sender<WorkMsg>,
    hwnd: isize,
) {
    std::thread::spawn(move || {
        let log_path = config::logs_dir().join(format!("git-{}.log", safe_slug(&name)));
        let Ok(mut logf) = File::create(&log_path) else { return };

        let step = |cmd_line: &str, logf: &mut File| -> std::result::Result<(), String> {
            run_logged(cmd_line, &path, logf, &log_path)
        };

        let res = (|| -> std::result::Result<String, String> {
            if !path.join(".git").exists() {
                step("git init", &mut logf)?;
                let _ = step("git branch -M main", &mut logf);
            }
            step("git add -A", &mut logf)?;

            let msg_clean = if commit_msg.trim().is_empty() {
                "Update project".to_string()
            } else {
                commit_msg.trim().replace('"', "'")
            };
            // Hacer commit si hay cambios pendientes
            let status_out = Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&path)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&status_out.stdout).trim().is_empty() {
                step(&format!("git commit -m \"{msg_clean}\""), &mut logf)?;
            }

            let has_remote = read_git_remote(&path).is_some();
            if !has_remote || create_gh_repo {
                if !has_remote {
                    let vis = if is_public { "--public" } else { "--private" };
                    step(
                        &format!("gh repo create \"{name}\" {vis} --source=. --remote=origin --push"),
                        &mut logf,
                    )?;
                    return Ok("Repositorio creado en GitHub y código subido".into());
                }
            }

            step("git push -u origin HEAD", &mut logf)?;
            Ok(format!("Commit '{}' subido a GitHub", short(&msg_clean)))
        })();

        match res {
            Ok(msg) => {
                let _ = tx.send(WorkMsg::Toast {
                    title: format!("GitHub · {name}"),
                    sub: msg,
                    is_err: false,
                    log_path: Some(log_path),
                });
            }
            Err(_) => {
                let reason = extract_last_error_reason(&log_path);
                let _ = tx.send(WorkMsg::Toast {
                    title: format!("Error Git/GitHub · {name}"),
                    sub: format!("{reason} · clic para ver el error en terminal"),
                    is_err: true,
                    log_path: Some(log_path),
                });
            }
        }
        notify_hwnd(hwnd);
        inspect_git_all(all_paths, tx, hwnd);
    });
}

// ============================================================================
// 2. SCRIPTS DE PACKAGE.JSON Y ACCIONES SECUNDARIAS RÁPIDAS
// ============================================================================

pub fn detect_package_scripts(path: &Path) -> Vec<(String, String)> {
    let Ok(content) = fs::read_to_string(path.join("package.json")) else {
        return Vec::new();
    };
    let Some(idx) = content.find("\"scripts\"") else {
        return Vec::new();
    };
    let after = &content[idx + 9..];
    let Some(open_brace) = after.find('{') else {
        return Vec::new();
    };
    let body = &after[open_brace + 1..];
    let mut scripts = Vec::new();
    let mut chars = body.chars().peekable();

    let read_quoted = |chars: &mut std::iter::Peekable<std::str::Chars>| -> Option<String> {
        while let Some(&c) = chars.peek() {
            if c == '}' {
                return None;
            }
            if c == '"' {
                chars.next();
                break;
            }
            chars.next();
        }
        let mut out = String::new();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(esc) = chars.next() {
                    out.push(esc);
                }
            } else if c == '"' {
                return Some(out);
            } else {
                out.push(c);
            }
        }
        None
    };

    while let Some(key) = read_quoted(&mut chars) {
        while let Some(&c) = chars.peek() {
            if c == ':' {
                chars.next();
                break;
            }
            if c == '}' {
                return scripts;
            }
            chars.next();
        }
        if let Some(val) = read_quoted(&mut chars) {
            scripts.push((key, val));
        } else {
            break;
        }
    }
    scripts
}

pub fn detect_dev_command(path: &Path, stack: &str) -> Option<(String, u16)> {
    let scripts = detect_package_scripts(path);
    let default_port = if stack.contains("Astro") {
        4321
    } else if stack.contains("Angular") {
        4200
    } else if stack.contains("Vite") || stack.contains("Vue") || stack.contains("Svelte") || stack.contains("TypeScript") {
        5173
    } else if stack.contains("Python") {
        8000
    } else {
        3000
    };

    if scripts.iter().any(|(k, _)| k == "dev") {
        return Some(("npm run dev".into(), default_port));
    }
    if scripts.iter().any(|(k, _)| k == "start") {
        return Some(("npm start".into(), default_port));
    }
    if path.join("manage.py").is_file() {
        return Some(("python manage.py runserver".into(), 8000));
    }
    if path.join("app.py").is_file() {
        return Some(("python app.py".into(), 5000));
    }
    if path.join("main.py").is_file() {
        return Some(("python main.py".into(), 8000));
    }
    if path.join("Cargo.toml").is_file() {
        return Some(("cargo run".into(), 8080));
    }
    None
}

pub fn open_terminal_at(path: &Path) {
    let p_str = path.to_string_lossy();
    let res = Command::new("cmd")
        .args(["/C", "wt", "-d", &p_str])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if res.is_err() {
        let _ = Command::new("cmd")
            .raw_arg(format!("/C start cmd /K \"cd /d \"{p_str}\"\""))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

pub fn open_explorer_at(path: &Path) {
    let _ = Command::new("explorer").arg(path).spawn();
}

pub fn open_browser(url: &str) {
    let _ = Command::new("cmd")
        .args(["/C", "start", "", url])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

pub fn run_npm_script_in_terminal(path: &Path, script: &str) {
    let p_str = path.to_string_lossy();
    let _ = Command::new("cmd")
        .args(["/C", "wt", "-d", &p_str, "cmd", "/k", &format!("npm run {script}")])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

// ============================================================================
// 3. SERVIDORES LOCALES ("LIVE DEV SERVERS") Y GESTIÓN DE PUERTOS
// ============================================================================

pub const COMMON_DEV_PORTS: &[u16] = &[
    3000, 3001, 3002, 4000, 4200, 4321, 4322, 5000, 5173, 5174, 5175, 5176, 8000, 8080, 8888,
];

/// Comprueba si un puerto local está abierto tanto en IPv4 (`127.0.0.1`) como en IPv6 (`[::1]`).
pub fn is_port_open(port: u16) -> bool {
    let v4 = SocketAddr::from(([127, 0, 0, 1], port));
    if TcpStream::connect_timeout(&v4, Duration::from_millis(14)).is_ok() {
        return true;
    }
    let v6 = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], port));
    TcpStream::connect_timeout(&v6, Duration::from_millis(14)).is_ok()
}

pub fn current_open_ports() -> Vec<u16> {
    let mut open = Vec::new();
    for &port in COMMON_DEV_PORTS {
        if is_port_open(port) {
            open.push(port);
        }
    }
    open
}

/// Extrae el número de puerto desde una línea de consola como `http://localhost:5174/` o `127.0.0.1:4321`.
pub fn extract_port_from_line(raw_line: &str) -> Option<u16> {
    let clean = strip_ansi(raw_line);
    for host in &["localhost:", "127.0.0.1:", "[::1]:", "0.0.0.0:", "[::]:"] {
        let mut search = clean.as_str();
        while let Some(idx) = search.find(host) {
            let after = &search[idx + host.len()..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(port) = digits.parse::<u16>() {
                if port >= 80 {
                    return Some(port);
                }
            }
            search = after;
        }
    }
    None
}

pub fn server_log_path(project_name: &str) -> PathBuf {
    config::logs_dir().join(format!("srv-{}.log", safe_slug(project_name)))
}

/// Inicia el servidor de desarrollo capturando su salida en vivo en un archivo de log,
/// detectando su puerto exacto desde stdout/stderr (y por socket IPv4/IPv6 sin colisionar con otros servidores),
/// y notificando inmediatamente si ocurre un error al iniciar.
pub fn start_dev_server(
    project_name: String,
    project_path: PathBuf,
    cmd_str: String,
    _fallback_port: u16,
    already_used_ports: Vec<u16>,
    tx: Sender<WorkMsg>,
    hwnd: isize,
) -> std::result::Result<(u32, PathBuf), String> {
    let mut before_ports = current_open_ports();
    for p in already_used_ports {
        if !before_ports.contains(&p) {
            before_ports.push(p);
        }
    }

    let log_path = server_log_path(&project_name);
    {
        let mut f = File::create(&log_path).map_err(|e| e.to_string())?;
        let _ = writeln!(f, "==========================================================");
        let _ = writeln!(f, "  Isla Dev — Servidor en Vivo: {project_name}");
        let _ = writeln!(f, "  Directorio: {}", project_path.display());
        let _ = writeln!(f, "  Comando:    $ {cmd_str}");
        let _ = writeln!(f, "==========================================================\n");
    }

    let mut child = Command::new("cmd")
        .raw_arg(format!("/S /C \"{cmd_str}\""))
        .current_dir(&project_path)
        .env("FORCE_COLOR", "1")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let path_str = project_path.to_string_lossy().to_string();
    let found_port = Arc::new(Mutex::new(None::<u16>));

    // Lector en tiempo real de stdout
    if let Some(out) = stdout {
        let log_c = log_path.clone();
        let tx_c = tx.clone();
        let path_c = path_str.clone();
        let fp_c = Arc::clone(&found_port);
        std::thread::spawn(move || {
            let reader = BufReader::new(out);
            for line in reader.lines().flatten() {
                if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_c) {
                    let _ = writeln!(f, "{line}");
                }
                if let Some(port) = extract_port_from_line(&line) {
                    let mut guard = fp_c.lock().unwrap();
                    if *guard != Some(port) {
                        *guard = Some(port);
                        let _ = tx_c.send(WorkMsg::ServerPort {
                            path: path_c.clone(),
                            pid,
                            port,
                        });
                        notify_hwnd(hwnd);
                    }
                }
            }
        });
    }

    // Lector en tiempo real de stderr
    if let Some(err) = stderr {
        let log_c = log_path.clone();
        let tx_c = tx.clone();
        let path_c = path_str.clone();
        let fp_c = Arc::clone(&found_port);
        std::thread::spawn(move || {
            let reader = BufReader::new(err);
            for line in reader.lines().flatten() {
                if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_c) {
                    let _ = writeln!(f, "{line}");
                }
                if let Some(port) = extract_port_from_line(&line) {
                    let mut guard = fp_c.lock().unwrap();
                    if *guard != Some(port) {
                        *guard = Some(port);
                        let _ = tx_c.send(WorkMsg::ServerPort {
                            path: path_c.clone(),
                            pid,
                            port,
                        });
                        notify_hwnd(hwnd);
                    }
                }
            }
        });
    }

    // Hilo vigilante del proceso y respaldo de escaneo de nuevos puertos (sin colisionar con puertos previos)
    let log_w = log_path.clone();
    std::thread::spawn(move || {
        let mut checks = 0;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let had_port = found_port.lock().unwrap().is_some();
                    let err_reason = if !status.success() || !had_port {
                        Some(extract_last_error_reason(&log_w))
                    } else {
                        None
                    };
                    let _ = tx.send(WorkMsg::ServerExited {
                        path: path_str,
                        pid,
                        err_reason,
                        log_path: Some(log_w),
                    });
                    notify_hwnd(hwnd);
                    break;
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(550));
                    checks += 1;
                    let has_p = found_port.lock().unwrap().is_some();
                    if !has_p && checks <= 22 {
                        let now_ports = current_open_ports();
                        if let Some(new_port) = now_ports.iter().copied().find(|p| !before_ports.contains(p)) {
                            let mut guard = found_port.lock().unwrap();
                            if guard.is_none() {
                                *guard = Some(new_port);
                                let _ = tx.send(WorkMsg::ServerPort {
                                    path: path_str.clone(),
                                    pid,
                                    port: new_port,
                                });
                                let _ = tx.send(WorkMsg::ActivePorts(now_ports));
                                notify_hwnd(hwnd);
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });

    Ok((pid, log_path))
}

/// Abre o enfoca la terminal en vivo del servidor mostrando el comando ejecutado (`npm run dev`, etc.) y toda su salida.
pub fn show_server_terminal(
    project_name: Option<&str>,
    project_path: Option<&Path>,
    port: Option<u16>,
    log_path: Option<&Path>,
) {
    if let Some(lp) = log_path {
        if lp.exists() {
            let label = project_name.unwrap_or("Servidor");
            open_log_in_terminal(label, project_path, lp);
            return;
        }
    }
    if let Some(name) = project_name {
        let lp = server_log_path(name);
        if lp.exists() {
            open_log_in_terminal(name, project_path, &lp);
            return;
        }
    }
    if let Some(p) = port {
        if focus_window_for_port(p) {
            return;
        }
    }
    if let Some(path) = project_path {
        open_terminal_at(path);
    }
}

fn focus_window_for_port(port: u16) -> bool {
    let Ok(out) = Command::new("netstat")
        .args(["-ano"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let needle = format!(":{port}");
    let mut target_pid = 0u32;
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 && parts[1].ends_with(&needle) && parts[3].eq_ignore_ascii_case("LISTENING") {
            if let Ok(pid) = parts[parts.len() - 1].parse::<u32>() {
                if pid > 0 {
                    target_pid = pid;
                    break;
                }
            }
        }
    }
    if target_pid == 0 {
        return false;
    }

    let mut chain = vec![target_pid];
    if let Ok(p_out) = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "$id={target_pid}; for($i=0;$i -lt 6;$i++){{ $p=(Get-CimInstance Win32_Process -Filter \"ProcessId=$id\" -ErrorAction SilentlyContinue).ParentProcessId; if(!$p){{break}}; Write-Output $p; $id=$p }}"
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        for line in String::from_utf8_lossy(&p_out.stdout).lines() {
            if let Ok(ppid) = line.trim().parse::<u32>() {
                if ppid > 0 {
                    chain.push(ppid);
                }
            }
        }
    }

    unsafe {
        for &pid in &chain {
            if let Some(hw) = find_window_by_pid(pid) {
                let _ = ShowWindow(hw, SW_SHOW);
                let _ = ShowWindow(hw, SW_RESTORE);
                let _ = SetForegroundWindow(hw);
                return true;
            }
        }
    }
    false
}

unsafe fn find_window_by_pid(target_pid: u32) -> Option<HWND> {
    struct SearchCtx {
        pid: u32,
        found: Option<HWND>,
    }
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut SearchCtx);
        let mut wnd_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut wnd_pid));
        if wnd_pid == ctx.pid {
            let mut cls = [0u16; 64];
            let len = GetClassNameW(hwnd, &mut cls);
            let cname = String::from_utf16_lossy(&cls[..len as usize]);
            if cname == "ConsoleWindowClass"
                || cname == "CASCADIA_HOSTING_WINDOW_CLASS"
                || IsWindowVisible(hwnd).as_bool()
            {
                ctx.found = Some(hwnd);
                return BOOL(0);
            }
        }
        BOOL(1)
    }
    let mut ctx = SearchCtx {
        pid: target_pid,
        found: None,
    };
    let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
    ctx.found
}

/// Detiene el árbol completo del proceso de un servidor de desarrollo (`taskkill /F /T /PID`).
pub fn stop_dev_server(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Escanea en segundo plano qué puertos de desarrollo están activos en localhost (IPv4 e IPv6).
pub fn scan_open_ports(extra_ports: Vec<u16>, tx: Sender<WorkMsg>, hwnd: isize) {
    std::thread::spawn(move || {
        let mut ports: Vec<u16> = COMMON_DEV_PORTS.to_vec();
        for ep in extra_ports {
            if !ports.contains(&ep) {
                ports.push(ep);
            }
        }
        ports.sort_unstable();
        let mut active = Vec::new();
        for port in ports {
            if is_port_open(port) {
                active.push(port);
            }
        }
        let _ = tx.send(WorkMsg::ActivePorts(active));
        notify_hwnd(hwnd);
    });
}

/// Mata cualquier proceso que esté ocupando un puerto específico (`netstat -ano` -> `taskkill`).
pub fn kill_port(port: u16, tx: Sender<WorkMsg>, hwnd: isize) {
    std::thread::spawn(move || {
        let mut killed = false;
        if let Ok(out) = Command::new("netstat")
            .args(["-ano"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            let needle = format!(":{port}");
            let mut pids = Vec::new();
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[1].ends_with(&needle) {
                    if let Ok(pid) = parts[parts.len() - 1].parse::<u32>() {
                        if pid > 0 && !pids.contains(&pid) {
                            pids.push(pid);
                        }
                    }
                }
            }
            for pid in pids {
                if let Ok(st) = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .status()
                {
                    if st.success() {
                        killed = true;
                    }
                }
            }
        }
        let _ = tx.send(WorkMsg::Toast {
            title: if killed {
                format!("Puerto :{port} liberado")
            } else {
                format!("Puerto :{port}")
            },
            sub: if killed {
                "Se cerró el proceso que ocupaba el puerto".into()
            } else {
                "No se encontró un proceso activo bloqueando el puerto".into()
            },
            is_err: !killed,
            log_path: None,
        });
        notify_hwnd(hwnd);
        scan_open_ports(Vec::new(), tx, hwnd);
    });
}

// ============================================================================
// 4. DIÁLOGOS WIN32, MULTI-MONITOR Y DETECCIÓN DE PANTALLA COMPLETA
// ============================================================================

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VAL: PCWSTR = w!("IslaDev");

pub fn startup_enabled() -> bool {
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VAL, RRF_RT_REG_SZ, None, None, None) == ERROR_SUCCESS }
}

pub fn set_startup(on: bool) {
    unsafe {
        if on {
            let exe = std::env::current_exe().unwrap_or_default();
            let v: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain(Some(0)).collect();
            let _ = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                RUN_VAL,
                REG_SZ.0,
                Some(v.as_ptr() as *const _),
                (v.len() * 2) as u32,
            );
        } else {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VAL);
        }
    }
}

pub fn pick_folder(hwnd: HWND, title: &str) -> Option<PathBuf> {
    unsafe {
        let d: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let o = d.GetOptions().ok()?;
        d.SetOptions(o | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM).ok()?;
        let _ = d.SetTitle(&HSTRING::from(title));
        d.Show(hwnd).ok()?;
        let item = d.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s.map(PathBuf::from)
    }
}

pub fn pick_exe_file(hwnd: HWND, title: &str) -> Option<PathBuf> {
    unsafe {
        let d: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let o = d.GetOptions().ok()?;
        d.SetOptions(o | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST).ok()?;
        let _ = d.SetTitle(&HSTRING::from(title));
        d.Show(hwnd).ok()?;
        let item = d.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s.map(PathBuf::from)
    }
}

/// Enumera los monitores conectados devolviendo sus rectángulos de pantalla completa.
pub fn list_monitors() -> Vec<RECT> {
    unsafe extern "system" fn mon_enum(hmon: HMONITOR, _hdc: HDC, _rc: *mut RECT, lparam: LPARAM) -> BOOL {
        let vec = &mut *(lparam.0 as *mut Vec<RECT>);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut mi).as_bool() {
            vec.push(mi.rcMonitor);
        }
        BOOL(1)
    }
    let mut list = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(HDC::default(), None, Some(mon_enum), LPARAM(&mut list as *mut _ as isize));
    }
    if list.is_empty() {
        let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        list.push(RECT {
            left: 0,
            top: 0,
            right: sw,
            bottom: sh,
        });
    }
    list
}

/// Devuelve el rectángulo del monitor objetivo según `monitor_mode` ("primary", "cursor", "monitor:0", etc.).
pub fn target_monitor_rect(mode: &str) -> RECT {
    if mode == "cursor" {
        unsafe {
            let mut pt = POINT::default();
            if GetCursorPos(&mut pt).is_ok() {
                let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
                let mut mi = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if GetMonitorInfoW(hmon, &mut mi).as_bool() {
                    return mi.rcMonitor;
                }
            }
        }
    } else if let Some(idx_str) = mode.strip_prefix("monitor:") {
        if let Ok(idx) = idx_str.parse::<usize>() {
            let mons = list_monitors();
            if let Some(&rc) = mons.get(idx) {
                return rc;
            }
        }
    }
    unsafe {
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        RECT {
            left: 0,
            top: 0,
            right: sw,
            bottom: sh,
        }
    }
}

/// Detecta si otra aplicación (juego o video) está en pantalla completa exclusiva/borderless sobre el monitor de la isla.
pub fn is_other_fullscreen(our_hwnd: HWND, mon_rc: RECT) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() || fg == our_hwnd || !IsWindowVisible(fg).as_bool() {
            return false;
        }
        let mut cls = [0u16; 64];
        let len = GetClassNameW(fg, &mut cls);
        let cname = String::from_utf16_lossy(&cls[..len as usize]);
        if matches!(
            cname.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Windows.UI.Core.CoreWindow"
        ) {
            return false;
        }
        let mut rc = RECT::default();
        if GetWindowRect(fg, &mut rc).is_err() {
            return false;
        }
        let style = GetWindowLongW(fg, GWL_STYLE) as u32;
        let has_caption = (style & WS_CAPTION.0) == WS_CAPTION.0;
        let covers_monitor = rc.left <= mon_rc.left
            && rc.top <= mon_rc.top
            && rc.right >= mon_rc.right
            && rc.bottom >= mon_rc.bottom;
        covers_monitor && !has_caption
    }
}

pub fn parse_hotkey(s: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    let mut m = MOD_NOREPEAT;
    let mut vk = None;
    for part in s.split('+').map(|p| p.trim().to_lowercase()) {
        match part.as_str() {
            "ctrl" | "control" => m = m | MOD_CONTROL,
            "alt" => m = m | MOD_ALT,
            "shift" => m = m | MOD_SHIFT,
            "win" => m = m | MOD_WIN,
            "space" | "espacio" => vk = Some(0x20),
            p if p.chars().count() == 1 => vk = Some(p.to_uppercase().chars().next()? as u32),
            p if p.starts_with('f') => {
                let n: u32 = p[1..].parse().ok()?;
                vk = Some(0x70 + n - 1)
            }
            _ => return None,
        }
    }
    Some((m, vk?))
}
