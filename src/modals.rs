use crate::actions;
use crate::app::*;
use crate::config::{APP_AUTHOR, APP_VERSION};

pub const WS_ICONS: &[&str] = &[
    "\u{E821}", "\u{E77B}", "\u{E716}", "\u{E945}", "\u{E8B7}", "\u{E943}", "\u{E90F}", "\u{E734}",
    "\u{E7B8}", "\u{E774}", "\u{E99A}", "\u{E790}", "\u{E7FC}", "\u{E72E}", "\u{E753}", "\u{E80F}",
];

impl App {
    pub(crate) fn draw_settings(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let def = self
            .cfg
            .launcher(&self.cfg.default_launcher)
            .map(|l| l.name.as_str())
            .unwrap_or("—");
        self.header(lx, y, tw, "Configuración", "Opciones de la isla", true);

        let monitor_label = match self.cfg.monitor_mode.as_str() {
            "cursor" => "Seguir al cursor".to_string(),
            s if s.starts_with("monitor:") => {
                let idx = s.trim_start_matches("monitor:").parse::<usize>().unwrap_or(0);
                format!("Monitor #{}", idx + 1)
            }
            _ => "Monitor principal".to_string(),
        };

        let items: [(Hit, &str, String, String, Option<bool>); 11] = [
            (
                Hit::ToggleTheme,
                if pal.light { G_SUN } else { G_MOON },
                "Modo Blanco Polar (Tema claro)".into(),
                if pal.light { "Activo: fondo blanco polar · clic para modo oscuro".into() } else { "Inactivo: modo oscuro · clic para modo blanco".into() },
                Some(self.cfg.light_mode),
            ),
            (
                Hit::ToggleVisibility,
                G_POWER,
                "Mostrar la isla en pantalla".into(),
                format!("Atajo rápido: {}", self.cfg.hotkey),
                Some(self.visible),
            ),
            (
                Hit::ToggleFullscreenHide,
                G_POWER,
                "Auto-ocultar en pantalla completa".into(),
                "Se oculta sola en juegos o videos a pantalla completa".into(),
                Some(self.cfg.hide_on_fullscreen),
            ),
            (
                Hit::ToggleSounds,
                G_CHEV,
                "Sonidos de interfaz (Micro-feedback)".into(),
                "Micro-sonidos suaves estilo Windows 11 al interactuar".into(),
                Some(self.cfg.sounds_enabled),
            ),
            (
                Hit::CycleMonitor,
                G_GLOBE,
                "Monitor de la isla (Multi-Monitor)".into(),
                monitor_label,
                None,
            ),
            (
                Hit::Startup,
                G_CHECK,
                "Iniciar junto con Windows".into(),
                "Arranca sola al encender tu PC".into(),
                Some(self.startup),
            ),
            (
                Hit::DefaultLauncher,
                G_CHEV,
                "Abrir proyectos con".into(),
                def.into(),
                None,
            ),
            (
                Hit::BaseDir,
                G_FOLDER,
                "Carpeta raíz de proyectos".into(),
                actions::short(&self.cfg.base_dir),
                None,
            ),
            (
                Hit::OpenCustomEditor,
                G_EDIT,
                "Personalizar Librerías, Programas y Frameworks".into(),
                "Editor visual sin tocar código".into(),
                None,
            ),
            (
                Hit::AddExisting,
                G_ADD,
                "Agregar carpeta de proyecto externa".into(),
                "También puedes arrastrar carpetas a la gota".into(),
                None,
            ),
            (
                Hit::Quit,
                G_EXIT,
                "Cerrar Isla Dev por completo".into(),
                "Detiene servidores activos y sale del programa".into(),
                None,
            ),
        ];

        // Selector visual interactivo de posición en pantalla (6 ubicaciones)
        let cur_pos = self.island_pos();
        let px = lx + 16.0;
        let py = y + 64.0;
        let pw = tw - 32.0;
        let ph = 76.0;
        g.rrect(px, py, pw, ph, 12.0, pal.card, 0.95);
        g.rrect_stroke(px, py, pw, ph, 12.0, pal.glacier, 0.20, 1.0);
        g.text("Posición en pantalla", &g.f.body, px + 14.0, py + 9.0, 290.0, 19.0, pal.white, 1.0);
        g.text(cur_pos.label(), &g.f.sub, px + 14.0, py + 29.0, 290.0, 17.0, pal.glacier, 1.0);
        g.text(
            "Siempre visible · Se oculta en fullscreen",
            &g.f.tiny,
            px + 14.0,
            py + 48.0,
            290.0,
            16.0,
            pal.gray,
            0.95,
        );

        let mx = px + pw - 196.0;
        let my = py + 8.0;
        let mw = 182.0;
        let mh = 60.0;
        g.rrect(mx, my, mw, mh, 9.0, if pal.light { 0xE2E8F0 } else { pal.body }, 0.96);
        g.rrect_stroke(mx, my, mw, mh, 9.0, pal.glacier, 0.25, 1.0);
        g.rrect(mx + 8.0, my + mh - 5.0, mw - 16.0, 2.5, 1.2, pal.glacier, 0.35);

        let spots: [(IslandPos, &str, f32, f32); 6] = [
            (IslandPos::TopLeft, "Izq", mx + 7.0, my + 6.0),
            (IslandPos::TopCenter, "Gota", mx + mw / 2.0 - 22.0, my + 6.0),
            (IslandPos::TopRight, "Der", mx + mw - 51.0, my + 6.0),
            (IslandPos::BottomLeft, "Izq", mx + 7.0, my + mh - 25.0),
            (IslandPos::BottomCenter, "Cen", mx + mw / 2.0 - 22.0, my + mh - 25.0),
            (IslandPos::BottomRight, "Der", mx + mw - 51.0, my + mh - 25.0),
        ];
        for (spot, lbl, sx, sy) in spots {
            let active = cur_pos == spot;
            let hov = self.hov(Hit::SetPosition(spot));
            let bg = if active { pal.brand_red } else { pal.card_hov };
            let ba = if active { 0.95 } else if hov { 0.95 } else { 0.85 };
            let bc = if active { 0xFFFFFF } else if hov { pal.glacier } else { pal.border };
            let bca = if active { 0.90 } else if hov { 0.85 } else { 0.9 };
            g.rrect(sx, sy, 44.0, 16.0, 8.0, bg, ba);
            g.rrect_stroke(sx, sy, 44.0, 16.0, 8.0, bc, bca, 1.0);
            g.text(
                lbl,
                &g.f.tiny_c,
                sx,
                sy,
                44.0,
                16.0,
                if active { 0xFFFFFF } else if hov { pal.white } else { pal.gray },
                1.0,
            );
            self.hit(sx, sy, 44.0, 16.0, Hit::SetPosition(spot));
        }

        let mut ry = y + 64.0 + 84.0;
        for (id, icon, label, sub, toggle) in items {
            let quit = id == Hit::Quit;
            self.row_bg(lx + 16.0, ry, tw - 32.0, 40.0, id, false);
            g.rrect(
                lx + 24.0,
                ry + 5.0,
                30.0,
                30.0,
                9.0,
                if quit { pal.red } else { pal.white },
                if quit { 0.16 } else { 0.07 },
            );
            g.text(
                icon,
                &g.f.icon,
                lx + 24.0,
                ry + 5.0,
                30.0,
                30.0,
                if quit { pal.red } else { pal.white },
                1.0,
            );
            g.text(
                &label,
                &g.f.body,
                lx + 64.0,
                ry + 2.0,
                tw - 150.0,
                19.0,
                if quit { pal.red } else { pal.white },
                1.0,
            );
            g.text(&sub, &g.f.sub, lx + 64.0, ry + 20.0, tw - 150.0, 16.0, pal.gray, 1.0);
            if let Some(on) = toggle {
                self.toggle_pill(lx + tw - 66.0, ry + 10.0, on);
            } else if !quit {
                g.text(G_CHEV, &g.f.icon_sm, lx + tw - 48.0, ry + 8.0, 24.0, 24.0, pal.dim, 1.0);
            }
            ry += 44.0;
        }

        // Pie de firma solicitado por el usuario + acceso opcional a config.toml
        let footer = format!("{} · {}", APP_AUTHOR, APP_VERSION);
        g.text(&footer, &g.f.tiny_c, lx + 16.0, ry + 4.0, tw - 32.0, 18.0, pal.gray, 0.9);
        let cfg_hov = self.hov(Hit::EditConfig);
        g.text(
            "Abrir config.toml",
            &g.f.sub_r,
            lx + tw - 140.0,
            ry + 2.0,
            120.0,
            20.0,
            if cfg_hov { pal.white } else { pal.dim },
            0.85,
        );
        self.hit(lx + tw - 140.0, ry + 2.0, 120.0, 20.0, Hit::EditConfig);
    }

    pub(crate) fn draw_wizard(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let (title, sub) = match self.step {
            Step::Name => ("Nuevo proyecto · Paso 1", "Nombre, tecnología y repositorio"),
            Step::Libraries => ("Nuevo proyecto · Paso 2", "Selecciona o agrega librerías"),
            Step::Launcher => ("Nuevo proyecto · Paso 3", "Con qué programa se abrirá"),
        };
        self.header(lx, y, tw, title, sub, true);
        let mut ry = y + 64.0;

        match self.step {
            Step::Name => {
                let bx = lx + 16.0;
                let bw = tw - 32.0;
                g.rrect(bx, ry, bw, 42.0, 12.0, pal.card, 1.0);
                g.rrect_stroke(
                    bx + 0.5,
                    ry + 0.5,
                    bw - 1.0,
                    41.0,
                    12.0,
                    if self.wiz_err.is_some() { pal.red } else { pal.white },
                    0.55,
                    1.2,
                );
                let empty = self.wiz_name.is_empty();
                let txt = if empty {
                    "nombre-del-proyecto"
                } else {
                    &self.wiz_name
                };
                g.text(
                    txt,
                    &g.f.input,
                    bx + 14.0,
                    ry,
                    bw - 28.0,
                    42.0,
                    if empty { pal.dim } else { pal.white },
                    1.0,
                );
                let cx = if empty {
                    bx + 14.0
                } else {
                    (bx + 14.0 + g.measure(&self.wiz_name, &g.f.input) + 1.5).min(bx + bw - 14.0)
                };
                g.line(cx, ry + 10.0, cx, ry + 32.0, pal.white, 0.95, 1.5);
                self.hit(bx, ry, bw, 42.0, Hit::Input);

                if let Some(err) = &self.wiz_err {
                    g.text(err, &g.f.sub, bx + 4.0, ry + 45.0, bw, 16.0, pal.red, 1.0);
                } else {
                    let preview = format!("{}\\{}", self.cfg.base_dir, self.wiz_name);
                    g.text(
                        &actions::short(&preview),
                        &g.f.sub,
                        bx + 4.0,
                        ry + 45.0,
                        bw,
                        16.0,
                        pal.gray,
                        1.0,
                    );
                }
                ry += 66.0;

                // Opción de subir a GitHub automáticamente al crear el proyecto
                self.row_bg(lx + 16.0, ry, tw - 32.0, 40.0, Hit::WizToggleGithub, self.wiz_github);
                g.draw_git_branch_icon(
                    lx + 30.0,
                    ry + 12.0,
                    16.0,
                    if self.wiz_github { pal.green } else { pal.gray },
                    1.0,
                );
                g.text(
                    "Subir a GitHub automáticamente al crear (git init + gh repo create)",
                    &g.f.body,
                    lx + 54.0,
                    ry,
                    tw - 135.0,
                    40.0,
                    pal.white,
                    1.0,
                );
                self.toggle_pill(lx + tw - 66.0, ry + 10.0, self.wiz_github);
                ry += 48.0;

                let max_tpl_vis = 5usize;
                let max_sc = self.cfg.templates.len().saturating_sub(max_tpl_vis);
                let sc = self.wiz_tpl_scroll.min(max_sc);
                for (i, t) in self.cfg.templates.iter().enumerate().skip(sc).take(max_tpl_vis) {
                    let sel = i == self.wiz_tpl;
                    self.row_bg(lx + 16.0, ry, tw - 32.0, 40.0, Hit::Template(i), sel);
                    g.draw_stack_badge(&t.stack, lx + 24.0, ry + 5.0, 30.0);
                    g.text(&t.name, &g.f.body, lx + 62.0, ry, tw - 130.0, 40.0, pal.white, 1.0);
                    if sel {
                        g.circle(lx + tw - 36.0, ry + 20.0, 9.0, pal.white, 1.0);
                        g.text(
                            G_CHECK,
                            &g.f.icon_sm,
                            lx + tw - 45.0,
                            ry + 11.0,
                            18.0,
                            18.0,
                            pal.inv_ink,
                            1.0,
                        );
                    }
                    ry += 46.0;
                }
            }
            Step::Libraries => {
                if let Some(t) = self.cfg.templates.get(self.wiz_tpl) {
                    let max_lib_vis = 6usize;
                    let max_sc = t.libraries.len().saturating_sub(max_lib_vis);
                    let sc = self.wiz_lib_scroll.min(max_sc);
                    for (i, l) in t.libraries.iter().enumerate().skip(sc).take(max_lib_vis) {
                        let on = self.wiz_libs.get(i).copied().unwrap_or(false);
                        self.row_bg(lx + 16.0, ry, tw - 32.0, 40.0, Hit::Library(i), on);
                        g.rrect(
                            lx + 26.0,
                            ry + 10.0,
                            20.0,
                            20.0,
                            6.0,
                            if on { pal.white } else { pal.border },
                            1.0,
                        );
                        if on {
                            g.text(G_CHECK, &g.f.icon_sm, lx + 26.0, ry + 10.0, 20.0, 20.0, pal.inv_ink, 1.0);
                        }
                        g.text(&l.name, &g.f.body, lx + 56.0, ry, 200.0, 40.0, pal.white, 1.0);
                        g.text(&l.command, &g.f.sub_r, lx + 250.0, ry, tw - 276.0, 40.0, pal.dim, 1.0);
                        ry += 46.0;
                    }
                }
                self.row_bg(lx + 16.0, ry, tw - 32.0, 38.0, Hit::WizOpenLibEditor, false);
                g.text(G_ADD, &g.f.icon_sm, lx + 26.0, ry, 24.0, 38.0, pal.green, 1.0);
                g.text(
                    "Configurar o agregar nueva librería a este framework…",
                    &g.f.sub,
                    lx + 54.0,
                    ry,
                    tw - 80.0,
                    38.0,
                    pal.green,
                    1.0,
                );
                ry += 46.0;
            }
            Step::Launcher => {
                for (i, l) in self.cfg.launchers.iter().enumerate().take(6) {
                    let sel = i == self.wiz_launcher;
                    self.row_bg(lx + 16.0, ry, tw - 32.0, 40.0, Hit::Launcher(i), sel);
                    g.rrect(
                        lx + 24.0,
                        ry + 6.0,
                        28.0,
                        28.0,
                        8.0,
                        pal.white,
                        if sel { 0.18 } else { 0.07 },
                    );
                    g.text(G_TERM, &g.f.icon_sm, lx + 24.0, ry + 6.0, 28.0, 28.0, pal.white, 1.0);
                    g.text(&l.name, &g.f.body, lx + 62.0, ry, 210.0, 40.0, pal.white, 1.0);
                    g.text(&l.command, &g.f.sub_r, lx + 270.0, ry, tw - 325.0, 40.0, pal.dim, 1.0);
                    if sel {
                        g.circle(lx + tw - 36.0, ry + 20.0, 9.0, pal.white, 1.0);
                        g.text(
                            G_CHECK,
                            &g.f.icon_sm,
                            lx + tw - 45.0,
                            ry + 11.0,
                            18.0,
                            18.0,
                            pal.inv_ink,
                            1.0,
                        );
                    }
                    ry += 46.0;
                }
                self.row_bg(lx + 16.0, ry, tw - 32.0, 38.0, Hit::WizOpenLauncherEditor, false);
                g.text(G_ADD, &g.f.icon_sm, lx + 26.0, ry, 24.0, 38.0, pal.green, 1.0);
                g.text(
                    "Agregar otro programa (.exe) a 'Abrir con'…",
                    &g.f.sub,
                    lx + 54.0,
                    ry,
                    tw - 80.0,
                    38.0,
                    pal.green,
                    1.0,
                );
                ry += 46.0;
            }
        }

        ry += 4.0;
        let hov = self.hov(Hit::Next);
        g.rrect(
            lx + 16.0,
            ry,
            tw - 32.0,
            42.0,
            12.0,
            pal.white,
            if hov { 1.0 } else { 0.90 },
        );
        let btn = if self.step == Step::Launcher {
            "Crear y abrir proyecto"
        } else {
            "Continuar  →"
        };
        g.text(btn, &g.f.btn, lx + 16.0, ry, tw - 32.0, 42.0, pal.inv_ink, 1.0);
        self.hit(lx + 16.0, ry, tw - 32.0, 42.0, Hit::Next);
    }

    pub(crate) fn draw_git_modal(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let Some(p) = self.cfg.projects.get(self.git_proj_idx) else { return };
        let git_info = self.git_states.get(&p.path);
        let branch = git_info.map(|gi| gi.branch.as_str()).unwrap_or("sin inicializar");
        let changed = git_info.map(|gi| gi.changed_count).unwrap_or(0);
        let has_remote = git_info.and_then(|gi| gi.remote_url.as_ref()).is_some();

        let sub = format!("Rama: {} · {} archivos por confirmar", branch, changed);
        self.header(lx, y, tw, &format!("Git & GitHub · {}", p.name), &sub, true);

        let mut ry = y + 66.0;
        let bx = lx + 16.0;
        let bw = tw - 32.0;

        g.text(
            "Mensaje del Commit:",
            &g.f.sub,
            bx + 2.0,
            ry,
            bw,
            18.0,
            pal.gray,
            1.0,
        );
        ry += 20.0;

        g.rrect(bx, ry, bw, 38.0, 10.0, pal.card, 1.0);
        g.rrect_stroke(bx + 0.5, ry + 0.5, bw - 1.0, 37.0, 10.0, pal.white, 0.5, 1.0);
        let empty = self.git_commit_msg.is_empty();
        let msg_txt = if empty {
            "Escribe el nombre del commit (ej. feat: login y mejoras)"
        } else {
            &self.git_commit_msg
        };
        g.text(
            msg_txt,
            &g.f.body,
            bx + 12.0,
            ry,
            bw - 24.0,
            38.0,
            if empty { pal.dim } else { pal.white },
            1.0,
        );
        let cx = if empty {
            bx + 12.0
        } else {
            (bx + 12.0 + g.measure(&self.git_commit_msg, &g.f.body) + 1.5).min(bx + bw - 14.0)
        };
        g.line(cx, ry + 9.0, cx, ry + 29.0, pal.white, 0.9, 1.4);
        self.hit(bx, ry, bw, 38.0, Hit::GitCommitInput);
        ry += 48.0;

        if !has_remote {
            self.row_bg(bx, ry, bw, 38.0, Hit::GitTogglePublic, false);
            let vis_txt = if self.git_public {
                "Crear repositorio en GitHub como: Público"
            } else {
                "Crear repositorio en GitHub como: Privado"
            };
            g.text(vis_txt, &g.f.body, bx + 14.0, ry, bw - 80.0, 38.0, pal.white, 1.0);
            self.toggle_pill(bx + bw - 50.0, ry + 9.0, self.git_public);
            ry += 46.0;
        }

        let hov_push = self.hov(Hit::GitDoCommitPush);
        g.rrect(bx, ry, bw, 42.0, 11.0, pal.green, if hov_push { 0.28 } else { 0.18 });
        g.rrect_stroke(bx + 0.5, ry + 0.5, bw - 1.0, 41.0, 11.0, pal.green, 0.75, 1.2);
        let push_label = if has_remote {
            "↑ Hacer Commit y Push a GitHub (Enter)"
        } else {
            "↑ Inicializar Git, Crear Repo en GitHub y hacer Push (Enter)"
        };
        g.text(push_label, &g.f.btn, bx, ry, bw, 42.0, pal.green, 1.0);
        self.hit(bx, ry, bw, 42.0, Hit::GitDoCommitPush);
        ry += 52.0;

        let half_w = (bw - 10.0) / 2.0;
        self.row_bg(bx, ry, half_w, 38.0, Hit::GitDoPull, false);
        g.text(G_DOWNLOAD, &g.f.icon_sm, bx + 10.0, ry, 24.0, 38.0, pal.white, 1.0);
        g.text("Git Pull (bajar cambios)", &g.f.body, bx + 38.0, ry, half_w - 44.0, 38.0, pal.white, 1.0);

        if has_remote {
            self.row_bg(bx + half_w + 10.0, ry, half_w, 38.0, Hit::GitOpenRepoWeb, false);
            g.text(G_GLOBE, &g.f.icon_sm, bx + half_w + 20.0, ry, 24.0, 38.0, pal.white, 1.0);
            g.text(
                "Abrir en GitHub.com",
                &g.f.body,
                bx + half_w + 48.0,
                ry,
                half_w - 54.0,
                38.0,
                pal.white,
                1.0,
            );
        } else {
            self.row_bg(bx + half_w + 10.0, ry, half_w, 38.0, Hit::GitOpenLogTerm, false);
            g.text(G_TERM, &g.f.icon_sm, bx + half_w + 20.0, ry, 24.0, 38.0, pal.white, 1.0);
            g.text(
                "Ver consola Git",
                &g.f.body,
                bx + half_w + 48.0,
                ry,
                half_w - 54.0,
                38.0,
                pal.white,
                1.0,
            );
        }
    }

    pub(crate) fn draw_note_modal(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let Some(p) = self.cfg.projects.get(self.note_proj_idx) else { return };
        self.header(
            lx,
            y,
            tw,
            &format!("{} · Nota", p.name),
            "",
            true,
        );

        let mut ry = y + 58.0;
        let bx = lx + 16.0;
        let bw = tw - 32.0;
        let box_h = 160.0;

        // Caja de texto amplia estilo tarjeta / textarea limpia
        g.rrect(bx, ry, bw, box_h, 10.0, pal.box_bg, 0.95);
        g.rrect_stroke(bx + 0.5, ry + 0.5, bw - 1.0, box_h - 1.0, 10.0, pal.accent, 0.45, 1.0);
        self.hit(bx, ry, bw, box_h, Hit::NoteInput);

        let cur_count = self.note_text.chars().count();
        let blink = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0) / 530) % 2 == 0;

        let display_text = if blink {
            format!("{}|", self.note_text)
        } else {
            self.note_text.clone()
        };
        g.text(
            &display_text,
            &g.f.body,
            bx + 14.0,
            ry + 12.0,
            bw - 28.0,
            box_h - 34.0,
            pal.white,
            0.95,
        );

        // Contador minimalista en la esquina inferior derecha
        let count_str = format!("{cur_count} / 1000");
        g.text(
            &count_str,
            &g.f.tiny,
            bx + bw - 74.0,
            ry + box_h - 22.0,
            64.0,
            16.0,
            if cur_count > 900 { pal.amber } else { pal.gray },
            0.6,
        );
        ry += box_h + 14.0;

        // Botones minimalistas: Guardar (✓), Papelera (G_TRASH monocromático) y Cerrar (✕)
        let btn_sz = 36.0;
        let mut ax = bx + bw - btn_sz;

        // Guardar
        self.mini_action_btn(ax, ry, btn_sz, G_CHECK, pal.green, true, Hit::NoteSave);
        ax -= btn_sz + 10.0;

        // Papelera monocromática
        let trash_hov = self.hov(Hit::NoteClear);
        self.mini_action_btn(ax, ry, btn_sz, G_TRASH, if trash_hov { pal.red } else { pal.gray }, false, Hit::NoteClear);
        ax -= btn_sz + 10.0;

        // Cancelar / Cerrar
        self.mini_action_btn(ax, ry, btn_sz, G_CLOSE, pal.gray, false, Hit::Back);
    }

    pub(crate) fn draw_workspace_modal(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let is_edit = self.ws_edit_idx.is_some();
        let title = if is_edit { "Editar Grupo de Trabajo" } else { "Nuevo Grupo de Trabajo" };
        self.header(lx, y, tw, title, "", true);

        let mut ry = y + 58.0;
        let bx = lx + 20.0;
        let bw = tw - 40.0;

        // Campo de Nombre
        g.text("Nombre del grupo:", &g.f.sub, bx, ry, bw, 18.0, pal.gray, 1.0);
        ry += 22.0;
        self.draw_input_box(bx, ry, bw, 36.0, "Ej. Trabajo, Clientes, Estudio…", &self.ws_name, true, Hit::WsNameInput);
        ry += 48.0;

        // Selector de Icono / Emoji
        g.text("Icono del grupo:", &g.f.sub, bx, ry, bw, 18.0, pal.gray, 1.0);
        ry += 24.0;

        let icon_sz = 34.0;
        let spacing = 8.0;
        let mut ix = bx;
        for (k, &ico) in WS_ICONS.iter().enumerate() {
            let is_sel = self.ws_icon == ico;
            let hov = self.hov(Hit::WsIconPick(k));
            g.rrect(
                ix,
                ry,
                icon_sz,
                icon_sz,
                8.0,
                if is_sel { pal.accent } else { pal.card },
                if is_sel { 0.28 } else if hov { 0.9 } else { 0.5 },
            );
            g.rrect_stroke(
                ix + 0.5,
                ry + 0.5,
                icon_sz - 1.0,
                icon_sz - 1.0,
                8.0,
                if is_sel { pal.accent } else { pal.border },
                if is_sel { 0.9 } else { 0.6 },
                1.0,
            );
            g.text(
                ico,
                &g.f.icon,
                ix,
                ry,
                icon_sz,
                icon_sz,
                if is_sel || hov { pal.white } else { pal.gray },
                1.0,
            );
            self.hit(ix, ry, icon_sz, icon_sz, Hit::WsIconPick(k));
            ix += icon_sz + spacing;
            if (k + 1) % 8 == 0 {
                ix = bx;
                ry += icon_sz + spacing;
            }
        }
        ry += 18.0;

        // Botones minimalistas
        let btn_sz = 36.0;
        let mut ax = bx + bw - btn_sz;

        // Guardar
        self.mini_action_btn(ax, ry, btn_sz, G_CHECK, pal.green, true, Hit::WsSave);
        ax -= btn_sz + 10.0;

        // Eliminar (solo si es edición)
        if let Some(idx) = self.ws_edit_idx {
            let trash_hov = self.hov(Hit::WsDelete(idx));
            self.mini_action_btn(ax, ry, btn_sz, G_TRASH, if trash_hov { pal.red } else { pal.gray }, false, Hit::WsDelete(idx));
            ax -= btn_sz + 10.0;
        }

        // Cancelar
        self.mini_action_btn(ax, ry, btn_sz, G_CLOSE, pal.gray, false, Hit::Back);
    }

    pub(crate) fn draw_custom_editor(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        self.header(
            lx,
            y,
            tw,
            "Personalizar Isla Dev",
            "Agrega librerías, programas (.exe) o frameworks visualmente",
            true,
        );

        let mut ry = y + 62.0;
        let bx = lx + 16.0;
        let bw = tw - 32.0;

        // Pestañas superiores
        let tabs = ["Librerías", "Programas (Abrir con)", "Frameworks"];
        let tab_w = (bw - 12.0) / 3.0;
        for (idx, t_label) in tabs.iter().enumerate() {
            let tx = bx + idx as f32 * (tab_w + 6.0);
            let active = self.ed_tab == idx;
            let hov = self.hov(Hit::EdTab(idx));
            g.rrect(
                tx,
                ry,
                tab_w,
                30.0,
                8.0,
                if active { pal.white } else { pal.card },
                if active {
                    0.95
                } else if hov {
                    0.9
                } else {
                    0.6
                },
            );
            g.rrect_stroke(tx, ry, tab_w, 30.0, 8.0, pal.border, 0.8, 1.0);
            g.text(
                t_label,
                &g.f.badge,
                tx,
                ry,
                tab_w,
                30.0,
                if active { pal.inv_ink } else { pal.white },
                1.0,
            );
            self.hit(tx, ry, tab_w, 30.0, Hit::EdTab(idx));
        }
        ry += 38.0;

        match self.ed_tab {
            0 => {
                let tpl_idx = self.ed_tpl_idx.min(self.cfg.templates.len().saturating_sub(1));
                let tpl_name = self
                    .cfg
                    .templates
                    .get(tpl_idx)
                    .map(|t| t.name.as_str())
                    .unwrap_or("—");

                g.rrect(bx, ry, bw, 34.0, 9.0, pal.card, 1.0);
                g.rrect_stroke(bx, ry, bw, 34.0, 9.0, pal.border, 0.8, 1.0);
                self.mini_action_btn(bx + 4.0, ry + 4.0, 26.0, G_BACK, pal.white, false, Hit::EdPrevTpl);
                g.text(
                    &format!("Framework: {tpl_name}"),
                    &g.f.badge,
                    bx + 36.0,
                    ry,
                    bw - 72.0,
                    34.0,
                    pal.white,
                    1.0,
                );
                self.mini_action_btn(
                    bx + bw - 30.0,
                    ry + 4.0,
                    26.0,
                    G_CHEV,
                    pal.white,
                    false,
                    Hit::EdNextTpl,
                );
                ry += 40.0;

                self.draw_input_box(
                    bx,
                    ry,
                    bw,
                    34.0,
                    "Nombre de la librería (ej. Axios, Shadcn, Prisma)",
                    &self.ed_f1,
                    self.ed_focus == 0,
                    Hit::EdField(0),
                );
                ry += 38.0;
                self.draw_input_box(
                    bx,
                    ry,
                    bw - 118.0,
                    34.0,
                    "Comando (ej. npm i axios)",
                    &self.ed_f2,
                    self.ed_focus == 1,
                    Hit::EdField(1),
                );
                self.pill_action_btn(
                    bx + bw - 112.0,
                    ry,
                    112.0,
                    34.0,
                    "+ Agregar",
                    pal.green,
                    true,
                    Hit::EdSave,
                );
                ry += 40.0;

                if let Some((msg, is_err)) = &self.ed_msg {
                    g.text(
                        msg,
                        &g.f.sub,
                        bx + 4.0,
                        ry - 4.0,
                        bw,
                        18.0,
                        if *is_err { pal.red } else { pal.green },
                        1.0,
                    );
                    ry += 18.0;
                }

                if let Some(t) = self.cfg.templates.get(tpl_idx) {
                    let max_vis = 5usize;
                    let max_sc = t.libraries.len().saturating_sub(max_vis);
                    let sc = self.ed_scroll.min(max_sc);
                    for (i, l) in t.libraries.iter().enumerate().skip(sc).take(max_vis) {
                        self.row_bg(bx, ry, bw, 38.0, Hit::EdToggleLibDefault(i), l.default);
                        g.rrect(
                            bx + 10.0,
                            ry + 9.0,
                            20.0,
                            20.0,
                            5.0,
                            if l.default { pal.white } else { pal.border },
                            1.0,
                        );
                        if l.default {
                            g.text(G_CHECK, &g.f.icon_sm, bx + 10.0, ry + 9.0, 20.0, 20.0, pal.inv_ink, 1.0);
                        }
                        g.text(&l.name, &g.f.body, bx + 38.0, ry, 180.0, 38.0, pal.white, 1.0);
                        g.text(&l.command, &g.f.sub_r, bx + 215.0, ry, bw - 256.0, 38.0, pal.dim, 1.0);
                        self.mini_action_btn(
                            bx + bw - 32.0,
                            ry + 6.0,
                            26.0,
                            G_TRASH,
                            pal.red,
                            false,
                            Hit::EdDeleteItem(i),
                        );
                        ry += 42.0;
                    }
                }
            }
            1 => {
                self.draw_input_box(
                    bx,
                    ry,
                    bw - 150.0,
                    36.0,
                    "Nombre del programa (ej. Cursor, WebStorm, Zed)",
                    &self.ed_f1,
                    self.ed_focus == 0,
                    Hit::EdField(0),
                );
                self.pill_action_btn(
                    bx + bw - 144.0,
                    ry,
                    144.0,
                    36.0,
                    "Elegir archivo .exe…",
                    pal.white,
                    false,
                    Hit::EdPickExe,
                );
                ry += 42.0;

                self.draw_input_box(
                    bx,
                    ry,
                    bw - 118.0,
                    36.0,
                    "Comando o ruta (ej. \"C:\\...\\Cursor.exe\" \"{path}\")",
                    &self.ed_f2,
                    self.ed_focus == 1,
                    Hit::EdField(1),
                );
                self.pill_action_btn(
                    bx + bw - 112.0,
                    ry,
                    112.0,
                    36.0,
                    "+ Guardar",
                    pal.green,
                    true,
                    Hit::EdSave,
                );
                ry += 44.0;

                if let Some((msg, is_err)) = &self.ed_msg {
                    g.text(
                        msg,
                        &g.f.sub,
                        bx + 4.0,
                        ry - 4.0,
                        bw,
                        18.0,
                        if *is_err { pal.red } else { pal.green },
                        1.0,
                    );
                    ry += 18.0;
                }

                let max_vis = 5usize;
                let max_sc = self.cfg.launchers.len().saturating_sub(max_vis);
                let sc = self.ed_scroll.min(max_sc);
                for (i, l) in self.cfg.launchers.iter().enumerate().skip(sc).take(max_vis) {
                    self.row_bg(bx, ry, bw, 40.0, Hit::Launcher(i), false);
                    g.text(&l.name, &g.f.body, bx + 14.0, ry, 175.0, 40.0, pal.white, 1.0);
                    g.text(&l.command, &g.f.sub_r, bx + 190.0, ry, bw - 230.0, 40.0, pal.dim, 1.0);
                    if self.cfg.launchers.len() > 1 {
                        self.mini_action_btn(
                            bx + bw - 32.0,
                            ry + 7.0,
                            26.0,
                            G_TRASH,
                            pal.red,
                            false,
                            Hit::EdDeleteItem(i),
                        );
                    }
                    ry += 44.0;
                }
            }
            _ => {
                self.draw_input_box(
                    bx,
                    ry,
                    bw - 170.0,
                    36.0,
                    "Nombre de la plantilla (ej. Nuxt 3, Remix, Tauri)",
                    &self.ed_f1,
                    self.ed_focus == 0,
                    Hit::EdField(0),
                );
                self.pill_action_btn(
                    bx + bw - 164.0,
                    ry,
                    164.0,
                    36.0,
                    &format!("Icono: {}", self.ed_f3),
                    pal.white,
                    false,
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

                self.pill_action_btn(
                    bx,
                    ry,
                    bw,
                    36.0,
                    "+ Guardar nuevo Framework",
                    pal.green,
                    true,
                    Hit::EdSave,
                );
                ry += 44.0;

                if let Some((msg, is_err)) = &self.ed_msg {
                    g.text(
                        msg,
                        &g.f.sub,
                        bx + 4.0,
                        ry - 4.0,
                        bw,
                        18.0,
                        if *is_err { pal.red } else { pal.green },
                        1.0,
                    );
                    ry += 18.0;
                }

                let max_vis = 4usize;
                let max_sc = self.cfg.templates.len().saturating_sub(max_vis);
                let sc = self.ed_scroll.min(max_sc);
                for (i, t) in self.cfg.templates.iter().enumerate().skip(sc).take(max_vis) {
                    self.row_bg(lx, ry, tw, 44.0, Hit::Template(i), false);
                    g.draw_stack_badge(&t.stack, lx + 20.0, ry + 6.0, 32.0);
                    g.text(&t.name, &g.f.body, lx + 60.0, ry, 180.0, 44.0, pal.white, 1.0);
                    let cmd_preview = t
                        .create
                        .first()
                        .or(t.setup.first())
                        .map(|s| s.as_str())
                        .unwrap_or("Carpeta limpia");
                    g.text(cmd_preview, &g.f.sub_r, lx + 240.0, ry, tw - 295.0, 44.0, pal.dim, 1.0);
                    if self.cfg.templates.len() > 1 {
                        self.mini_action_btn(
                            lx + tw - 44.0,
                            ry + 9.0,
                            26.0,
                            G_TRASH,
                            pal.red,
                            false,
                            Hit::EdDeleteItem(i),
                        );
                    }
                    ry += 44.0;
                }
            }
        }
    }

    pub(crate) fn draw_input_box(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        placeholder: &str,
        val: &str,
        focused: bool,
        id: Hit,
    ) {
        let pal = self.pal();
        let g = &self.gfx;
        g.rrect(x, y, w, h, 8.0, pal.white, if focused { 0.08 } else { 0.04 });
        g.rrect_stroke(
            x + 0.5,
            y + 0.5,
            w - 1.0,
            h - 1.0,
            8.0,
            if focused { pal.white } else { pal.border },
            if focused { 0.65 } else { 0.7 },
            1.0,
        );
        if val.is_empty() {
            g.text(placeholder, &g.f.sub, x + 10.0, y, w - 20.0, h, pal.dim, 1.0);
        } else {
            g.text(val, &g.f.body, x + 10.0, y, w - 20.0, h, pal.white, 1.0);
        }
        if focused {
            let cx = (x + 10.0 + g.measure(val, &g.f.body) + 1.5).min(x + w - 10.0);
            g.line(cx, y + 9.0, cx, y + h - 9.0, pal.white, 0.95, 1.4);
        }
        self.hit(x, y, w, h, id);
    }

    pub(crate) fn draw_status(&self, lx: f32, y: f32, tw: f32, h: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let cy = y + h / 2.0;
        match self.skind {
            SKind::Error => {
                g.circle(lx + 30.0, cy, 18.0, pal.red, 1.0);
                g.text(G_ERR, &g.f.icon, lx + 12.0, cy - 18.0, 36.0, 36.0, 0xFFFFFF, 1.0);
            }
            _ => {
                g.draw_logo(lx + 14.0, cy - 18.0, 36.0, 36.0, 1.0);
            }
        }
        g.text(&self.s_title, &g.f.title, lx + 60.0, cy - 19.0, tw - 110.0, 19.0, pal.white, 1.0);
        g.text(
            &self.s_sub,
            &g.f.sub,
            lx + 60.0,
            cy + 1.0,
            tw - 110.0,
            18.0,
            if self.skind == SKind::Creating {
                pal.white
            } else if self.skind == SKind::Error {
                pal.amber
            } else {
                pal.gray
            },
            1.0,
        );

        // Si está creando/clonando/subiendo, mostrar barra de progreso fina también en la parte inferior del banner
        if let Some(prog) = self.bg_progress {
            let bar_x = lx + 60.0;
            let bar_w = tw - 110.0;
            let bar_y = y + h - 7.0;
            g.rrect(bar_x, bar_y, bar_w, 3.0, 1.5, pal.white, 0.12);
            g.rrect(
                bar_x,
                bar_y,
                (bar_w * prog.clamp(0.05, 1.0)).max(8.0),
                3.0,
                1.5,
                pal.green,
                0.95,
            );
        }

        let rx = lx + tw - 32.0;
        match self.skind {
            SKind::Opening | SKind::Creating => {
                for i in 0..8 {
                    let a = i as f32 / 8.0 * std::f32::consts::TAU + self.spin * 5.0;
                    let fade = 0.2 + 0.8 * (i as f32 / 8.0);
                    g.circle(rx + a.cos() * 8.0, cy + a.sin() * 8.0, 1.8, pal.white, fade);
                }
            }
            SKind::Done => {
                g.circle(rx, cy, 11.0, pal.white, 1.0);
                g.text(G_CHECK, &g.f.icon_sm, rx - 11.0, cy - 11.0, 22.0, 22.0, pal.inv_ink, 1.0);
            }
            SKind::Error => {
                g.circle(rx, cy, 13.0, pal.white, 0.14);
                g.text(G_TERM, &g.f.icon_sm, rx - 11.0, cy - 11.0, 22.0, 22.0, pal.white, 1.0);
            }
        }
        self.hit(lx, y, tw, h, Hit::Pill);
    }

    fn draw_modal_tabs(&self, lx: f32, ry: f32, tabs: &[(&str, f32, Hit)], active: usize) {
        let pal = self.pal();
        let mut cx = lx;
        for (i, &(label, w, hit_id)) in tabs.iter().enumerate() {
            let is_act = i == active;
            let hov = self.hov(hit_id);
            let bg = if is_act { pal.accent } else if hov { pal.card_hov } else { pal.card };
            let alpha = if is_act { 0.22 } else { 0.92 };
            let border = if is_act { pal.accent } else { pal.border };
            self.gfx.rrect(cx, ry, w, 28.0, 7.0, bg, alpha);
            self.gfx.rrect_stroke(cx, ry, w, 28.0, 7.0, border, if is_act { 0.85 } else { 0.7 }, 1.0);
            let tc = if is_act { pal.white } else { pal.gray };
            self.gfx.text(label, &self.gfx.f.sub_c, cx + 8.0, ry, w - 16.0, 28.0, tc, 1.0);
            self.hit(cx, ry, w, 28.0, hit_id);
            cx += w + 8.0;
        }
    }

    fn draw_vscroll(&self, x: f32, y: f32, h: f32, scroll: usize, visible: usize, total: usize) {
        let pal = self.pal();
        if total <= visible || h <= 16.0 {
            return;
        }
        self.gfx.rrect(x, y, 4.0, h, 2.0, pal.white, 0.08);
        let max_scroll = total - visible;
        let thumb_h = ((visible as f32 / total as f32) * h).clamp(24.0, h);
        let frac = (scroll.min(max_scroll) as f32) / (max_scroll.max(1) as f32);
        let thumb_y = y + frac * (h - thumb_h);
        self.gfx.rrect(x, thumb_y, 4.0, thumb_h, 2.0, pal.gray, 0.55);
    }

    fn draw_footer_status(&self, lx: f32, y: f32, tw: f32, msg: &Option<(String, bool)>, fallback: &str) {
        let pal = self.pal();
        if let Some((text, is_err)) = msg {
            let col = if *is_err { pal.red } else { pal.green };
            self.gfx.rrect(lx, y, tw, 26.0, 6.0, col, 0.14);
            self.gfx.rrect_stroke(lx, y, tw, 26.0, 6.0, col, 0.45, 1.0);
            self.gfx.text(text, &self.gfx.f.tiny, lx + 12.0, y, tw - 24.0, 26.0, col, 1.0);
        } else {
            self.gfx.text(fallback, &self.gfx.f.tiny, lx, y, tw, 26.0, pal.dim, 1.0);
        }
    }

    pub(crate) fn draw_env_modal(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let proj_name = self.cfg.projects.get(self.env_proj_idx).map(|p| p.name.as_str()).unwrap_or("Proyecto");
        let env_file = if self.env_filename.is_empty() { ".env" } else { &self.env_filename };
        let sub = format!("{} - {}", proj_name, env_file);
        self.header(lx, y, tw, "Variables de Entorno", &sub, true);

        let lx = lx + 20.0;
        let tw = tw - 40.0;
        let mut ry = y + 68.0;
        let t0 = format!("Proyecto ({})", self.env_vars.len());
        let t1 = format!("Boveda ({})", self.cfg.env_vault.len());
        self.draw_modal_tabs(lx, ry, &[(&t0, 126.0, Hit::EnvTab(0)), (&t1, 126.0, Hit::EnvTab(1))], self.env_tab);

        let sync_w = 176.0;
        let mask_w = 104.0;
        self.pill_action_btn(
            lx + tw - mask_w,
            ry,
            mask_w,
            28.0,
            if self.env_masked { "Mostrar" } else { "Ocultar" },
            if self.env_masked { pal.accent } else { pal.amber },
            true,
            Hit::EnvToggleMask,
        );
        self.pill_action_btn(
            lx + tw - mask_w - sync_w - 8.0,
            ry,
            sync_w,
            28.0,
            "Sincronizar .env.example",
            pal.cyan,
            true,
            Hit::EnvSyncExample,
        );

        ry += 38.0;
        let list_bottom = y + 452.0;

        if self.env_tab == 0 {
            let key_w = 188.0;
            let save_w = 88.0;
            let val_w = (tw - key_w - save_w - 16.0).max(120.0);
            self.draw_input_box(lx, ry, key_w, 34.0, "CLAVE (ej. API_KEY)", &self.env_key_in, self.env_focus == 0, Hit::EnvKeyInput);
            self.draw_input_box(lx + key_w + 8.0, ry, val_w, 34.0, "Valor o pega KEY=VAL", &self.env_val_in, self.env_focus == 1, Hit::EnvValInput);
            self.pill_action_btn(lx + tw - save_w, ry + 1.0, save_w, 32.0, "Guardar", pal.green, !self.env_key_in.trim().is_empty(), Hit::EnvSaveVar);
            ry += 40.0;
            g.text("Tab cambia de campo | Puedes pegar texto con formato CLAVE=VALOR directamente", &g.f.tiny, lx, ry, tw, 16.0, pal.dim, 1.0);
            ry += 22.0;

            let avail_h = (list_bottom - ry).max(45.0);
            let step = 45.0;
            let vis = ((avail_h + 5.0) / step).floor().max(1.0) as usize;

            if self.env_vars.is_empty() {
                g.rrect(lx, ry, tw, 64.0, 10.0, pal.card, 0.85);
                g.rrect_stroke(lx, ry, tw, 64.0, 10.0, pal.border, 0.7, 1.0);
                g.text("No hay variables en este archivo. Agrega una arriba o inyecta desde tu Boveda.", &g.f.sub_c, lx + 16.0, ry + 22.0, tw - 32.0, 20.0, pal.gray, 1.0);
            } else {
                let row_w = if self.env_vars.len() > vis { tw - 12.0 } else { tw };
                let max_s = self.env_vars.len().saturating_sub(vis);
                let scroll = self.env_scroll.min(max_s);
                for (r, (k, v)) in self.env_vars.iter().enumerate().skip(scroll).take(vis) {
                    let row_y = ry + (r - scroll) as f32 * step;
                    g.rrect(lx, row_y, row_w, 40.0, 8.0, pal.card, 0.90);
                    g.rrect_stroke(lx, row_y, row_w, 40.0, 8.0, pal.border, 0.65, 1.0);

                    g.text(k, &g.f.body, lx + 14.0, row_y, 184.0, 40.0, pal.cyan, 1.0);
                    let val_disp = if v.is_empty() {
                        "(vacio)".to_string()
                    } else if self.env_masked {
                        "********".to_string()
                    } else {
                        v.clone()
                    };
                    let val_col = if v.is_empty() { pal.dim } else if self.env_masked { pal.gray } else { pal.body };
                    g.text(&val_disp, &g.f.sub, lx + 204.0, row_y, (row_w - 344.0).max(80.0), 40.0, val_col, 1.0);

                    let bx = lx + row_w - 126.0;
                    let by = row_y + 7.0;
                    self.mini_action_btn(bx, by, 26.0, G_COPY, pal.cyan, false, Hit::EnvCopyVal(r));
                    self.mini_action_btn(bx + 30.0, by, 26.0, G_EDIT, pal.accent, false, Hit::EnvEditVar(r));
                    self.mini_action_btn(bx + 60.0, by, 26.0, G_DOWNLOAD, pal.amber, false, Hit::EnvSaveToVault(r));
                    self.mini_action_btn(bx + 90.0, by, 26.0, G_TRASH, pal.red, false, Hit::EnvDeleteVar(r));
                }
                self.draw_vscroll(lx + tw - 5.0, ry, avail_h, scroll, vis, self.env_vars.len());
            }
        } else {
            g.rrect(lx, ry, tw, 36.0, 8.0, pal.card, 0.85);
            g.rrect_stroke(lx, ry, tw, 36.0, 8.0, pal.border, 0.65, 1.0);
            g.text("Claves personales persistentes. Haz clic en Inyectar para copiarlas al proyecto actual.", &g.f.sub, lx + 14.0, ry, tw - 28.0, 36.0, pal.gray, 1.0);
            ry += 44.0;

            let avail_h = (list_bottom - ry).max(45.0);
            let step = 45.0;
            let vis = ((avail_h + 5.0) / step).floor().max(1.0) as usize;

            if self.cfg.env_vault.is_empty() {
                g.rrect(lx, ry, tw, 64.0, 10.0, pal.card, 0.85);
                g.rrect_stroke(lx, ry, tw, 64.0, 10.0, pal.border, 0.7, 1.0);
                g.text("Tu Boveda esta vacia. Guarda variables desde la pestana Proyecto para reutilizarlas aqui.", &g.f.sub_c, lx + 16.0, ry + 22.0, tw - 32.0, 20.0, pal.gray, 1.0);
            } else {
                let row_w = if self.cfg.env_vault.len() > vis { tw - 12.0 } else { tw };
                let max_s = self.cfg.env_vault.len().saturating_sub(vis);
                let scroll = self.env_scroll.min(max_s);
                for (r, entry) in self.cfg.env_vault.iter().enumerate().skip(scroll).take(vis) {
                    let row_y = ry + (r - scroll) as f32 * step;
                    g.rrect(lx, row_y, row_w, 40.0, 8.0, pal.card, 0.90);
                    g.rrect_stroke(lx, row_y, row_w, 40.0, 8.0, pal.border, 0.65, 1.0);

                    g.text(&entry.key, &g.f.body, lx + 14.0, row_y, 184.0, 40.0, pal.amber, 1.0);
                    let val_disp = if entry.value.is_empty() {
                        "(vacio)".to_string()
                    } else if self.env_masked {
                        "********".to_string()
                    } else {
                        entry.value.clone()
                    };
                    g.text(&val_disp, &g.f.sub, lx + 204.0, row_y, (row_w - 342.0).max(80.0), 40.0, pal.gray, 1.0);

                    let bx = lx + row_w - 124.0;
                    self.pill_action_btn(bx, row_y + 7.0, 82.0, 26.0, "Inyectar", pal.green, true, Hit::EnvVaultInject(r));
                    self.mini_action_btn(lx + row_w - 36.0, row_y + 7.0, 26.0, G_TRASH, pal.red, false, Hit::EnvVaultDelete(r));
                }
                self.draw_vscroll(lx + tw - 5.0, ry, avail_h, scroll, vis, self.cfg.env_vault.len());
            }
        }

        self.draw_footer_status(lx, y + 462.0, tw, &self.env_msg, "Los cambios se escriben directo en disco | Esc: Volver");
    }

    pub(crate) fn draw_cleaner_modal(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let total_bytes: u64 = self.clean_items.iter().map(|i| i.cache_bytes).sum();
        self.header(lx, y, tw, "Limpiador Inteligente y Recetas", "Inspecciona librerias, libera espacio y restaura con 1 clic", true);

        let lx = lx + 20.0;
        let tw = tw - 40.0;
        let mut ry = y + 68.0;
        let t0 = format!("Recuperable ({})", actions::format_bytes(total_bytes));
        let t1 = format!("Recetas ({})", self.cfg.clean_manifests.len());
        self.draw_modal_tabs(lx, ry, &[(&t0, 176.0, Hit::CleanerTab(0)), (&t1, 132.0, Hit::CleanerTab(1))], self.clean_tab);

        let rescan_w = 108.0;
        let clean_all_w = 122.0;
        self.pill_action_btn(
            lx + tw - clean_all_w,
            ry,
            clean_all_w,
            28.0,
            "Limpiar todo",
            pal.red,
            !self.clean_items.is_empty() && !self.clean_scanning,
            Hit::CleanerCleanAll,
        );
        self.pill_action_btn(
            lx + tw - clean_all_w - rescan_w - 8.0,
            ry,
            rescan_w,
            28.0,
            "Reescanear",
            pal.cyan,
            !self.clean_scanning,
            Hit::CleanerRescan,
        );

        ry += 38.0;
        let area_h = (y + 472.0 - ry).max(120.0);

        if self.clean_tab == 0 {
            if self.clean_scanning {
                g.rrect(lx, ry, tw, area_h, 12.0, pal.card, 0.88);
                g.rrect_stroke(lx, ry, tw, area_h, 12.0, pal.border, 0.7, 1.0);
                let cx = lx + tw * 0.5;
                let cy = ry + area_h * 0.42;
                for i in 0..8 {
                    let a = i as f32 / 8.0 * std::f32::consts::TAU + self.spin * 5.0;
                    let fade = 0.2 + 0.8 * (i as f32 / 8.0);
                    g.circle(cx + a.cos() * 14.0, cy + a.sin() * 14.0, 2.4, pal.cyan, fade);
                }
                g.text("Escaneando node_modules, target, .next, .venv y analizando librerias...", &g.f.sub_c, lx + 24.0, cy + 28.0, tw - 48.0, 22.0, pal.white, 1.0);
            } else if self.clean_items.is_empty() {
                g.rrect(lx, ry, tw, area_h, 12.0, pal.card, 0.88);
                g.rrect_stroke(lx, ry, tw, area_h, 12.0, pal.border, 0.7, 1.0);
                g.text("No hay caches pesados detectados en tus proyectos.", &g.f.body, lx + 24.0, ry + 130.0, tw - 48.0, 24.0, pal.green, 1.0);
                g.text("Si limpiaste alguno recientemente, revisa la pestana Recetas para restaurar sus dependencias.", &g.f.sub, lx + 24.0, ry + 160.0, tw - 48.0, 22.0, pal.gray, 1.0);
            } else {
                let left_w = 240.0;
                let step = 50.0;
                let vis = ((area_h + 4.0) / step).floor().max(1.0) as usize;
                let max_s = self.clean_items.len().saturating_sub(vis);
                let scroll = self.clean_scroll.min(max_s);
                let card_w = if self.clean_items.len() > vis { left_w - 10.0 } else { left_w };

                for (i, item) in self.clean_items.iter().enumerate().skip(scroll).take(vis) {
                    let row_y = ry + (i - scroll) as f32 * step;
                    let sel = i == self.clean_sel_idx;
                    self.row_bg(lx, row_y, card_w, 46.0, Hit::CleanerSelectProj(i), sel);
                    g.text(&item.project_name, &g.f.body, lx + 14.0, row_y + 6.0, card_w - 28.0, 18.0, pal.white, 1.0);
                    let meta = format!("{}  |  {} objetivos", actions::format_bytes(item.cache_bytes), item.targets_found.len());
                    g.text(&meta, &g.f.tiny, lx + 14.0, row_y + 26.0, card_w - 28.0, 15.0, pal.amber, 1.0);
                }
                self.draw_vscroll(lx + left_w - 5.0, ry, area_h, scroll, vis, self.clean_items.len());

                let detail_x = lx + left_w + 12.0;
                let detail_w = (tw - left_w - 12.0).max(200.0);
                g.rrect(detail_x, ry, detail_w, area_h, 12.0, pal.card, 0.92);
                g.rrect_stroke(detail_x, ry, detail_w, area_h, 12.0, pal.border, 0.75, 1.0);

                let sel_idx = self.clean_sel_idx.min(self.clean_items.len() - 1);
                if let Some(sel) = self.clean_items.get(sel_idx) {
                    let pad_x = detail_x + 16.0;
                    let inner_w = detail_w - 32.0;
                    let mut dy = ry + 14.0;

                    g.text(&sel.project_name, &g.f.title, pad_x, dy, inner_w - 92.0, 22.0, pal.white, 1.0);
                    let sz_txt = actions::format_bytes(sel.cache_bytes);
                    g.rrect(pad_x + inner_w - 82.0, dy, 82.0, 22.0, 6.0, pal.amber, 0.16);
                    g.text(&sz_txt, &g.f.badge, pad_x + inner_w - 82.0, dy, 82.0, 22.0, pal.amber, 1.0);
                    dy += 24.0;

                    g.text(&sel.project_path, &g.f.tiny, pad_x, dy, inner_w, 15.0, pal.dim, 1.0);
                    dy += 18.0;

                    g.text("CARPETAS A ELIMINAR", &g.f.tiny, pad_x, dy, inner_w, 14.0, pal.gray, 1.0);
                    dy += 15.0;
                    let targets_str = sel.targets_found.join(", ");
                    g.rrect(pad_x, dy, inner_w, 38.0, 7.0, pal.white, 0.04);
                    g.text(&targets_str, &g.f.tiny, pad_x + 10.0, dy + 5.0, inner_w - 20.0, 28.0, pal.red, 1.0);
                    dy += 46.0;

                    let lib_hdr = format!("LIBRERIAS QUE SE GUARDARAN EN RECETA ({})", sel.libraries_detected.len());
                    g.text(&lib_hdr, &g.f.tiny, pad_x, dy, inner_w, 14.0, pal.gray, 1.0);
                    dy += 15.0;
                    let libs_preview = if sel.libraries_detected.is_empty() {
                        "Solo artefactos de compilacion (sin dependencias externas declaradas)".to_string()
                    } else {
                        let shown: Vec<String> = sel.libraries_detected.iter().take(12).cloned().collect();
                        let extra = sel.libraries_detected.len().saturating_sub(12);
                        if extra > 0 {
                            format!("{} (+{} mas guardadas en .isladev-restore.txt)", shown.join(", "), extra)
                        } else {
                            shown.join(", ")
                        }
                    };
                    g.rrect(pad_x, dy, inner_w, 100.0, 7.0, pal.cyan, 0.06);
                    g.text(&libs_preview, &g.f.tiny, pad_x + 10.0, dy + 6.0, inner_w - 20.0, 88.0, pal.cyan, 1.0);
                    dy += 108.0;

                    g.text("COMANDO DE RESTAURACION", &g.f.tiny, pad_x, dy, inner_w, 14.0, pal.gray, 1.0);
                    dy += 15.0;
                    let cmds = if sel.restore_commands.is_empty() {
                        "Sin reinstalacion requerida".to_string()
                    } else {
                        sel.restore_commands.join(" && ")
                    };
                    g.rrect(pad_x, dy, inner_w, 34.0, 7.0, pal.green, 0.08);
                    g.text(&cmds, &g.f.tiny, pad_x + 10.0, dy + 5.0, inner_w - 20.0, 24.0, pal.green, 1.0);

                    let btn_y = ry + area_h - 44.0;
                    self.pill_action_btn(pad_x, btn_y, inner_w, 30.0, "Limpiar y guardar receta de restauracion", pal.red, true, Hit::CleanerCleanOne(sel_idx));
                }
            }
        } else if self.cfg.clean_manifests.is_empty() {
            g.rrect(lx, ry, tw, area_h, 12.0, pal.card, 0.88);
            g.rrect_stroke(lx, ry, tw, area_h, 12.0, pal.border, 0.7, 1.0);
            g.text("Aun no hay recetas guardadas. Al limpiar espacio, aqui veras que librerias tenia cada repo para reinstalarlas.", &g.f.sub_c, lx + 24.0, ry + 150.0, tw - 48.0, 36.0, pal.gray, 1.0);
        } else {
            let step = 90.0;
            let vis = ((area_h + 6.0) / step).floor().max(1.0) as usize;
            let row_w = if self.cfg.clean_manifests.len() > vis { tw - 12.0 } else { tw };
            let max_s = self.cfg.clean_manifests.len().saturating_sub(vis);
            let scroll = self.clean_scroll.min(max_s);

            for (i, m) in self.cfg.clean_manifests.iter().enumerate().skip(scroll).take(vis) {
                let row_y = ry + (i - scroll) as f32 * step;
                g.rrect(lx, row_y, row_w, 84.0, 10.0, pal.card, 0.92);
                g.rrect_stroke(lx, row_y, row_w, 84.0, 10.0, pal.border, 0.7, 1.0);

                g.text(&m.project_name, &g.f.body, lx + 14.0, row_y + 8.0, 220.0, 20.0, pal.white, 1.0);
                let badge = format!("Liberado: {} | {}", actions::format_bytes(m.freed_bytes), m.cleaned_at);
                g.text(&badge, &g.f.tiny, lx + 240.0, row_y + 10.0, (row_w - 416.0).max(80.0), 16.0, pal.amber, 1.0);

                let targets = format!("Eliminado: {}", m.targets_removed.join(", "));
                g.text(&targets, &g.f.tiny, lx + 14.0, row_y + 30.0, row_w - 190.0, 15.0, pal.gray, 1.0);

                let libs_txt = if m.libraries_summary.is_empty() {
                    "Librerias: solo caches/compilacion".to_string()
                } else {
                    let preview: Vec<String> = m.libraries_summary.iter().take(6).cloned().collect();
                    format!("Librerias ({}): {}", m.libraries_summary.len(), preview.join(", "))
                };
                g.text(&libs_txt, &g.f.tiny, lx + 14.0, row_y + 47.0, row_w - 190.0, 15.0, pal.cyan, 1.0);

                let cmd_txt = format!("Comando: {}", m.restore_commands.join(" && "));
                g.text(&cmd_txt, &g.f.tiny, lx + 14.0, row_y + 64.0, row_w - 190.0, 15.0, pal.dim, 1.0);

                self.pill_action_btn(lx + row_w - 162.0, row_y + 27.0, 148.0, 30.0, "Restaurar librerias", pal.green, true, Hit::CleanerRestore(i));
            }
            self.draw_vscroll(lx + tw - 5.0, ry, area_h, scroll, vis, self.cfg.clean_manifests.len());
        }

        self.draw_footer_status(lx, y + 482.0, tw, &self.clean_msg, "Cada limpieza guarda .isladev-restore.txt en el repo y en config.toml | Ctrl+D");
    }

    pub(crate) fn draw_console_modal(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let active_src = self.con_sources.get(self.con_source_idx);
        let sub = active_src.map(|s| s.0.as_str()).unwrap_or("Sin logs activos");
        self.header(lx, y, tw, "Consola de Logs en Vivo", sub, true);

        let lx = lx + 20.0;
        let tw = tw - 40.0;
        let mut ry = y + 68.0;
        let max_tabs = 4usize;
        let tab_w = ((tw - (max_tabs as f32 - 1.0) * 8.0) / max_tabs as f32).min(164.0);
        for (i, src) in self.con_sources.iter().take(max_tabs).enumerate() {
            let tx = lx + i as f32 * (tab_w + 8.0);
            let is_act = i == self.con_source_idx;
            let hov = self.hov(Hit::ConsoleSelectSource(i));
            g.rrect(tx, ry, tab_w, 26.0, 6.0, if is_act { pal.cyan } else if hov { pal.card_hov } else { pal.card }, if is_act { 0.20 } else { 0.90 });
            g.rrect_stroke(tx, ry, tab_w, 26.0, 6.0, if is_act { pal.cyan } else { pal.border }, 0.75, 1.0);
            g.text(&src.0, &g.f.tiny_c, tx + 8.0, ry, tab_w - 16.0, 26.0, if is_act { pal.white } else { pal.gray }, 1.0);
            self.hit(tx, ry, tab_w, 26.0, Hit::ConsoleSelectSource(i));
        }

        ry += 34.0;
        let has_srv = active_src.and_then(|s| s.2.as_ref()).is_some();
        let has_task = !has_srv
            && active_src
                .map(|s| self.active_tasks.iter().any(|t| t.log_path == s.1))
                .unwrap_or(false);
        self.pill_action_btn(lx, ry, 152.0, 28.0, "Copiar ultimo error", pal.red, !self.con_lines.is_empty(), Hit::ConsoleCopyLastError);
        self.pill_action_btn(lx + 160.0, ry, 102.0, 28.0, "Copiar todo", pal.cyan, !self.con_lines.is_empty(), Hit::ConsoleCopyAll);
        self.pill_action_btn(lx + 270.0, ry, 84.0, 28.0, "Limpiar", pal.amber, active_src.is_some(), Hit::ConsoleClear);
        if has_srv {
            self.pill_action_btn(lx + 362.0, ry, 144.0, 28.0, "Reiniciar servidor", pal.green, true, Hit::ConsoleRestartServer);
        } else if has_task {
            self.pill_action_btn(lx + 362.0, ry, 136.0, 28.0, "Detener proceso", pal.red, true, Hit::ConsoleStopTask);
        }
        self.pill_action_btn(lx + tw - 134.0, ry, 134.0, 28.0, "Terminal externa", pal.gray, active_src.is_some(), Hit::ConsoleOpenExternal);

        ry += 36.0;
        let box_h = (y + 452.0 - ry).max(160.0);
        g.rrect(lx, ry, tw, box_h, 10.0, pal.box_bg, 0.96);
        g.rrect_stroke(lx, ry, tw, box_h, 10.0, pal.border, 0.85, 1.0);

        if self.con_lines.is_empty() {
            g.text("Esperando salida del proceso... Inicia un servidor, script o restauracion para verlo aqui.", &g.f.sub_c, lx + 20.0, ry + box_h * 0.45, tw - 40.0, 22.0, pal.dim, 1.0);
        } else {
            let line_h = 18.0;
            let max_vis = ((box_h - 16.0) / line_h).floor().max(1.0) as usize;
            let total = self.con_lines.len();
            let end = total.saturating_sub(self.con_scroll.min(total.saturating_sub(max_vis)));
            let start = end.saturating_sub(max_vis);

            for (idx, line) in self.con_lines[start..end].iter().enumerate() {
                let ly = ry + 8.0 + idx as f32 * line_h;
                let lower = line.to_lowercase();
                let col = if lower.contains("error") || lower.contains("err!") || lower.contains("failed") || lower.contains("fatal") || lower.contains("panic") || lower.contains("exception") || lower.contains("traceback") {
                    pal.red
                } else if lower.contains("warn") || lower.contains("deprecated") {
                    pal.amber
                } else if lower.contains("http://") || lower.contains("https://") || lower.starts_with("[isla-dev]") || lower.starts_with("---") {
                    pal.cyan
                } else if lower.contains("ready") || lower.contains("listening") || lower.contains("compiled") || lower.contains("success") || lower.contains("completad") {
                    pal.green
                } else {
                    pal.white
                };
                g.text(line, &g.f.tiny, lx + 14.0, ly, tw - 30.0, line_h, col, 1.0);
            }
            let scroll_from_top = start;
            self.draw_vscroll(lx + tw - 8.0, ry + 8.0, box_h - 16.0, scroll_from_top, max_vis, total);
        }

        self.draw_footer_status(lx, y + 462.0, tw, &self.con_msg, "Actualizacion en vivo cada 450 ms | Ctrl+L: Consola | Esc: Volver");
    }

    pub(crate) fn draw_runner_modal(&self, lx: f32, y: f32, tw: f32) {
        let pal = self.pal();
        let g = &self.gfx;
        let proj = self.cfg.projects.get(self.run_proj_idx);
        let proj_name = proj.map(|p| p.name.as_str()).unwrap_or("Proyecto");
        let proj_path = proj.map(|p| p.path.as_str()).unwrap_or("");
        let has_manifest = !proj_path.is_empty() && self.cfg.clean_manifests.iter().any(|m| m.project_path.eq_ignore_ascii_case(proj_path));

        self.header(lx, y, tw, "Quick Runner y Herramientas", proj_name, true);

        let lx = lx + 20.0;
        let tw = tw - 40.0;
        let mut ry = y + 68.0;
        self.pill_action_btn(lx, ry, 130.0, 28.0, "Variables .env", pal.cyan, true, Hit::RunnerOpenEnv);
        if has_manifest {
            self.pill_action_btn(lx + 138.0, ry, 232.0, 28.0, "Restaurar librerias eliminadas", pal.green, true, Hit::RunnerRestoreDeps);
        } else {
            let cnt = format!("{} comandos detectados en el proyecto", self.run_items.len());
            g.text(&cnt, &g.f.sub, lx + 144.0, ry, tw - 150.0, 28.0, pal.gray, 1.0);
        }

        ry += 38.0;
        let list_bottom = y + 434.0;
        let avail_h = (list_bottom - ry).max(46.0);
        let step = 46.0;
        let vis = ((avail_h + 4.0) / step).floor().max(1.0) as usize;

        if self.run_items.is_empty() {
            g.rrect(lx, ry, tw, 72.0, 10.0, pal.card, 0.88);
            g.rrect_stroke(lx, ry, tw, 72.0, 10.0, pal.border, 0.7, 1.0);
            g.text("No se detectaron scripts (package.json, Cargo.toml, Python o Docker) en este proyecto.", &g.f.sub_c, lx + 16.0, ry + 26.0, tw - 32.0, 22.0, pal.gray, 1.0);
        } else {
            let row_w = if self.run_items.len() > vis { tw - 12.0 } else { tw };
            let max_s = self.run_items.len().saturating_sub(vis);
            let scroll = self.run_scroll.min(max_s);
            for (i, item) in self.run_items.iter().enumerate().skip(scroll).take(vis) {
                let row_y = ry + (i - scroll) as f32 * step;
                let hov = self.hov(Hit::RunnerExec(i));
                self.row_bg(lx, row_y, row_w, 42.0, Hit::RunnerExec(i), false);

                let cat_l = item.category.to_lowercase();
                let badge_col = if cat_l.contains("batch") || cat_l.contains("prisma") {
                    pal.accent
                } else if cat_l.contains("cargo") || cat_l.contains("rust") {
                    pal.amber
                } else if cat_l.contains("py") || cat_l.contains("django") {
                    pal.green
                } else {
                    pal.cyan
                };
                g.rrect(lx + 12.0, row_y + 11.0, 68.0, 20.0, 5.0, badge_col, 0.16);
                g.text(&item.category, &g.f.tiny_c, lx + 12.0, row_y + 11.0, 68.0, 20.0, badge_col, 1.0);

                g.text(&item.label, &g.f.body, lx + 90.0, row_y + 4.0, row_w - 138.0, 18.0, pal.white, 1.0);
                let sub = if item.subtitle.is_empty() { &item.command } else { &item.subtitle };
                g.text(sub, &g.f.tiny, lx + 90.0, row_y + 22.0, row_w - 138.0, 15.0, pal.dim, 1.0);

                let expected_title = format!("{proj_name} · {}", item.label);
                if let Some(t_idx) = self.active_tasks.iter().position(|t| t.title == expected_title) {
                    self.mini_action_btn(lx + row_w - 36.0, row_y + 8.0, 26.0, G_STOP, pal.red, true, Hit::KillTask(t_idx));
                } else {
                    g.text(G_PLAY, &g.f.icon_sm, lx + row_w - 34.0, row_y + 13.0, 20.0, 16.0, if hov { pal.green } else { pal.gray }, 1.0);
                }
            }
            self.draw_vscroll(lx + tw - 5.0, ry, avail_h, scroll, vis, self.run_items.len());
        }

        g.text("Haz clic en un comando para ejecutarlo en vivo dentro de la Consola | Ctrl+R | Esc: Volver", &g.f.tiny, lx, y + 444.0, tw, 24.0, pal.dim, 1.0);
    }
}

