use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

pub const APP_VERSION: &str = "v0.3.0";
pub const APP_AUTHOR: &str = "Desarrollado por Ing. Andres Luna";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default = "d_hotkey")]
    pub hotkey: String,
    pub base_dir: String,
    pub default_launcher: String,
    #[serde(default = "d_true")]
    pub hide_on_fullscreen: bool,
    #[serde(default = "d_monitor")]
    pub monitor_mode: String,
    #[serde(default)]
    pub launchers: Vec<Launcher>,
    #[serde(default)]
    pub templates: Vec<Template>,
    #[serde(default)]
    pub projects: Vec<Project>,
}

fn d_hotkey() -> String {
    "Ctrl+Alt+Space".into()
}

fn d_true() -> bool {
    true
}

fn d_monitor() -> String {
    "primary".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Launcher {
    pub id: String,
    pub name: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub stack: String,
    pub color: String,
    /// Comandos ejecutados en la carpeta base (crean la carpeta del proyecto).
    #[serde(default)]
    pub create: Vec<String>,
    /// Comandos ejecutados dentro de la carpeta del proyecto.
    #[serde(default)]
    pub setup: Vec<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Library {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub default: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub name: String,
    pub path: String,
    pub stack: String,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher: Option<String>,
    #[serde(default)]
    pub last_opened: u64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_path: Option<String>,
}

impl Config {
    pub fn launcher(&self, id: &str) -> Option<&Launcher> {
        self.launchers
            .iter()
            .find(|l| l.id == id)
            .or_else(|| self.launchers.first())
    }
    pub fn launcher_index(&self, id: &str) -> usize {
        self.launchers.iter().position(|l| l.id == id).unwrap_or(0)
    }
    pub fn sort_projects(&mut self) {
        self.projects.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.last_opened.cmp(&a.last_opened))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }
    pub fn sync_scanned_projects(&mut self, scanned: Vec<Project>) -> bool {
        let mut changed = false;

        // 1. Eliminar automáticamente proyectos cuya carpeta haya sido borrada manualmente en el Explorador
        let before_len = self.projects.len();
        self.projects.retain(|p| std::path::Path::new(&p.path).is_dir());
        if self.projects.len() != before_len {
            changed = true;
        }

        // 2. Agregar carpetas nuevas y actualizar stack si antes era "Solo carpeta"
        for p in scanned {
            if let Some(existing) = self
                .projects
                .iter_mut()
                .find(|existing| existing.path.eq_ignore_ascii_case(&p.path))
            {
                if existing.stack == "Solo carpeta" && p.stack != "Solo carpeta" {
                    existing.stack = p.stack;
                    existing.color = p.color;
                    changed = true;
                }
            } else {
                self.projects.push(p);
                changed = true;
            }
        }
        if changed {
            self.sort_projects();
        }
        changed
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn app_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    let p = PathBuf::from(base).join("IslaDev");
    let _ = std::fs::create_dir_all(&p);
    p
}

pub fn logs_dir() -> PathBuf {
    let p = app_dir().join("logs");
    let _ = std::fs::create_dir_all(&p);
    p
}

pub fn config_path() -> PathBuf {
    app_dir().join("config.toml")
}

pub fn mtime() -> Option<SystemTime> {
    std::fs::metadata(config_path()).and_then(|m| m.modified()).ok()
}

fn default_text() -> String {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".into());
    DEFAULT_TOML.replace("{BASE}", &format!("{home}\\Proyectos"))
}

pub fn default_config() -> Config {
    toml::from_str(&default_text()).expect("config por defecto válida")
}

/// Sincroniza plantillas y librerías nuevas por defecto en configuraciones previas sin borrar personalizaciones.
fn upgrade_templates(c: &mut Config) -> bool {
    let def = default_config();
    let mut changed = false;

    for dt in def.templates {
        if let Some(existing) = c.templates.iter_mut().find(|t| t.id == dt.id) {
            // Actualizar comandos interactivos antiguos que daban problemas en versiones recientes de Astro/Next
            for lib in &mut existing.libraries {
                if lib.command.contains("npx astro add tailwind") {
                    lib.command = "npm i tailwindcss @tailwindcss/vite".into();
                    changed = true;
                }
                if lib.command.contains("npx shadcn@latest init") && !lib.command.contains("--yes") {
                    lib.command = "npx --yes shadcn@latest init -d --yes".into();
                    changed = true;
                }
            }
            // Añadir librerías nuevas compatibles que aún no estén en la plantilla
            for dlib in dt.libraries {
                if !existing.libraries.iter().any(|l| l.name.eq_ignore_ascii_case(&dlib.name)) {
                    existing.libraries.push(dlib);
                    changed = true;
                }
            }
        } else {
            c.templates.push(dt);
            changed = true;
        }
    }
    changed
}

/// Carga la configuración (creándola si no existe). Devuelve error legible si el TOML es inválido.
pub fn load() -> (Config, Option<String>) {
    let path = config_path();
    if !path.exists() {
        let _ = std::fs::write(&path, default_text());
    }
    match std::fs::read_to_string(&path) {
        Ok(s) => match toml::from_str::<Config>(&s) {
            Ok(mut c) => {
                if c.launchers.is_empty() {
                    c.launchers = default_config().launchers;
                }
                if c.templates.is_empty() {
                    c.templates = default_config().templates;
                } else if upgrade_templates(&mut c) {
                    save(&c);
                }
                c.sort_projects();
                (c, None)
            }
            Err(e) => (default_config(), Some(format!("config.toml inválido: {}", e.message()))),
        },
        Err(e) => (default_config(), Some(e.to_string())),
    }
}

pub fn save(c: &Config) {
    if let Ok(s) = toml::to_string_pretty(c) {
        let header = "# Configuración de Isla Dev\n# Placeholders en comandos: {path} ruta del proyecto, {name} nombre, {parent} carpeta base\n\n";
        let _ = std::fs::write(config_path(), format!("{header}{s}"));
    }
}

pub fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0x6366F1)
}

const DEFAULT_TOML: &str = r##"# Configuración de Isla Dev
# Placeholders en comandos: {path} ruta del proyecto, {name} nombre, {parent} carpeta base
hotkey = "Ctrl+Alt+Space"
base_dir = '{BASE}'
default_launcher = "antigravity"
hide_on_fullscreen = true
monitor_mode = "primary"

[[launchers]]
id = "antigravity"
name = "Antigravity IDE"
command = 'antigravity-ide "{path}"'

[[launchers]]
id = "vscode"
name = "VS Code"
command = 'code "{path}"'

[[launchers]]
id = "cursor"
name = "Cursor"
command = 'cursor "{path}"'

[[launchers]]
id = "opencode"
name = "OpenCode (terminal)"
command = 'wt -d "{path}" cmd /k opencode'

[[launchers]]
id = "gemini"
name = "Gemini CLI (terminal)"
command = 'wt -d "{path}" cmd /k gemini'

[[launchers]]
id = "terminal"
name = "Terminal"
command = 'wt -d "{path}"'

[[launchers]]
id = "explorer"
name = "Explorador de archivos"
command = 'explorer "{path}"'

[[templates]]
id = "vite-react"
name = "Vite + React (TS)"
stack = "React + Vite"
color = "#3B82F6"
create = ['npm create vite@latest {name} -- --template react-ts --no-interactive']
setup = ['npm install']
libraries = [
  { name = "Tailwind CSS", command = 'npm i tailwindcss @tailwindcss/vite', default = true },
  { name = "Lucide Icons", command = 'npm i lucide-react', default = true },
  { name = "React Router", command = 'npm i react-router-dom' },
  { name = "Zustand", command = 'npm i zustand' },
  { name = "TanStack Query", command = 'npm i @tanstack/react-query' },
  { name = "Framer Motion", command = 'npm i framer-motion' },
  { name = "Axios", command = 'npm i axios' },
  { name = "Hook Form + Zod", command = 'npm i react-hook-form zod @hookform/resolvers' },
  { name = "Supabase JS", command = 'npm i @supabase/supabase-js' },
]

[[templates]]
id = "next"
name = "Next.js (App Router)"
stack = "Next.js"
color = "#71717A"
create = ['npx --yes create-next-app@latest {name} --ts --tailwind --eslint --app --src-dir --import-alias "@/*" --use-npm --yes']
libraries = [
  { name = "Lucide Icons", command = 'npm i lucide-react', default = true },
  { name = "Zustand", command = 'npm i zustand' },
  { name = "TanStack Query", command = 'npm i @tanstack/react-query' },
  { name = "Zod + Hook Form", command = 'npm i zod react-hook-form @hookform/resolvers' },
  { name = "Prisma ORM", command = 'npm i -D prisma && npm i @prisma/client && npx prisma init' },
  { name = "Supabase JS", command = 'npm i @supabase/supabase-js' },
  { name = "Framer Motion", command = 'npm i framer-motion' },
  { name = "Axios", command = 'npm i axios' },
]

[[templates]]
id = "astro"
name = "Astro"
stack = "Astro"
color = "#FF5D01"
create = ['npm create astro@latest {name} -- --template minimal --install --no-git --skip-houston --yes']
libraries = [
  { name = "Tailwind CSS", command = 'npm i tailwindcss @tailwindcss/vite', default = true },
  { name = "Lucide Astro", command = 'npm i @lucide/astro' },
  { name = "React Components", command = 'npm i @astrojs/react react react-dom @types/react @types/react-dom' },
  { name = "MDX", command = 'npm i @astrojs/mdx' },
  { name = "Sitemap", command = 'npm i @astrojs/sitemap' },
  { name = "Motion", command = 'npm i motion' },
]

[[templates]]
id = "vite-vue"
name = "Vite + Vue 3 (TS)"
stack = "Vue + Vite"
color = "#42B883"
create = ['npm create vite@latest {name} -- --template vue-ts --no-interactive']
setup = ['npm install']
libraries = [
  { name = "Tailwind CSS", command = 'npm i tailwindcss @tailwindcss/vite', default = true },
  { name = "Pinia", command = 'npm i pinia' },
  { name = "Vue Router", command = 'npm i vue-router' },
  { name = "VueUse", command = 'npm i @vueuse/core' },
  { name = "Lucide Vue", command = 'npm i lucide-vue-next' },
  { name = "Axios", command = 'npm i axios' },
]

[[templates]]
id = "vite-svelte"
name = "Vite + Svelte (TS)"
stack = "Svelte"
color = "#FF3E00"
create = ['npm create vite@latest {name} -- --template svelte-ts --no-interactive']
setup = ['npm install']
libraries = [
  { name = "Tailwind CSS", command = 'npm i tailwindcss @tailwindcss/vite', default = true },
  { name = "Lucide Svelte", command = 'npm i lucide-svelte' },
  { name = "Axios", command = 'npm i axios' },
  { name = "Zod", command = 'npm i zod' },
]

[[templates]]
id = "angular"
name = "Angular"
stack = "Angular"
color = "#DD0031"
create = ['npx --yes @angular/cli@latest new {name} --routing --style=css --skip-git --defaults']
libraries = [
  { name = "Lucide Angular", command = 'npm i lucide-angular' },
  { name = "Axios", command = 'npm i axios' },
  { name = "Zod", command = 'npm i zod' },
]

[[templates]]
id = "node"
name = "Node.js + Express API"
stack = "Node.js"
color = "#22C55E"
setup = ['npm init -y']
libraries = [
  { name = "Express + CORS", command = 'npm i express cors', default = true },
  { name = "dotenv", command = 'npm i dotenv', default = true },
  { name = "TypeScript + tsx", command = 'npm i -D typescript tsx @types/node @types/express && npx tsc --init' },
  { name = "Prisma ORM", command = 'npm i -D prisma && npm i @prisma/client && npx prisma init' },
  { name = "Mongoose (MongoDB)", command = 'npm i mongoose' },
  { name = "JWT + Bcrypt", command = 'npm i jsonwebtoken bcryptjs' },
  { name = "Socket.io", command = 'npm i socket.io' },
  { name = "Zod", command = 'npm i zod' },
  { name = "whatsapp-web.js", command = 'npm i whatsapp-web.js qrcode-terminal' },
]

[[templates]]
id = "python"
name = "Python (.venv)"
stack = "Python"
color = "#3776AB"
setup = ['python -m venv .venv']
libraries = [
  { name = "FastAPI + Uvicorn", command = '.venv\Scripts\pip install fastapi uvicorn', default = true },
  { name = "Flask", command = '.venv\Scripts\pip install flask' },
  { name = "Requests + BS4", command = '.venv\Scripts\pip install requests beautifulsoup4' },
  { name = "Pandas + NumPy", command = '.venv\Scripts\pip install pandas numpy' },
  { name = "Python-Dotenv", command = '.venv\Scripts\pip install python-dotenv' },
  { name = "SQLAlchemy", command = '.venv\Scripts\pip install sqlalchemy' },
]

[[templates]]
id = "rust"
name = "Rust (Cargo Binario)"
stack = "Rust"
color = "#CE422B"
create = ['cargo new {name}']
libraries = [
  { name = "Serde + JSON", command = 'cargo add serde --features derive && cargo add serde_json', default = true },
  { name = "Tokio Async", command = 'cargo add tokio --features full' },
  { name = "Reqwest HTTP", command = 'cargo add reqwest --features json' },
  { name = "Clap CLI", command = 'cargo add clap --features derive' },
  { name = "Anyhow", command = 'cargo add anyhow' },
]

[[templates]]
id = "vanilla-vite"
name = "Vanilla TS + Vite"
stack = "TypeScript"
color = "#3178C6"
create = ['npm create vite@latest {name} -- --template vanilla-ts --no-interactive']
setup = ['npm install']
libraries = [
  { name = "Tailwind CSS", command = 'npm i tailwindcss @tailwindcss/vite', default = true },
  { name = "GSAP Animations", command = 'npm i gsap' },
  { name = "Three.js 3D", command = 'npm i three @types/three' },
  { name = "Lucide Icons", command = 'npm i lucide' },
]

[[templates]]
id = "vacio"
name = "Proyecto vacío"
stack = "Solo carpeta"
color = "#6366F1"
"##;
