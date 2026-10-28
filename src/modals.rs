use crate::actions;
use crate::app::*;
use crate::config::{APP_AUTHOR, APP_VERSION};

impl App {
    pub(crate) fn draw_settings(&self, lx: f32, y: f32, tw: f32) {
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

        let items: [(Hit, &str, String, String, Option<bool>); 9] = [
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

        let mut ry = y + 64.0;
        for (id, icon, label, sub, toggle) in items {
            let quit = id == Hit::Quit;
            self.row_bg(lx + 16.0, ry, tw - 32.0, 42.0, id, false);
            g.rrect(
                lx + 24.0,
                ry + 6.0,
                30.0,
                30.0,
                9.0,
                if quit { RED } else { WHITE },
                if quit { 0.16 } else { 0.07 },
            );
            g.text(
                icon,
                &g.f.icon,
                lx + 24.0,
                ry + 6.0,
                30.0,
                30.0,
                if quit { RED } else { WHITE },
                1.0,
            );
            g.text(
                &label,
                &g.f.body,
                lx + 64.0,
                ry + 3.0,
                tw - 150.0,
                19.0,
                if quit { RED } else { WHITE },
                1.0,
            );
            g.text(&sub, &g.f.sub, lx + 64.0, ry + 21.0, tw - 150.0, 16.0, GRAY, 1.0);
            if let Some(on) = toggle {
                self.toggle_pill(lx + tw - 66.0, ry + 11.0, on);
            } else if !quit {
                g.text(G_CHEV, &g.f.icon_sm, lx + tw - 48.0, ry + 9.0, 24.0, 24.0, DIM, 1.0);
            }
            ry += 48.0;
        }

        // Pie de firma solicitado por el usuario + acceso opcional a config.toml
        let footer = format!("Desarrollado por {} · {}", APP_AUTHOR, APP_VERSION);
        g.text(&footer, &g.f.tiny_c, lx + 16.0, ry + 4.0, tw - 32.0, 18.0, GRAY, 0.9);
        let cfg_hov = self.hov(Hit::EditConfig);
        g.text(
            "Abrir config.toml",
            &g.f.sub_r,
            lx + tw - 140.0,
            ry + 2.0,
            120.0,
            20.0,
            if cfg_hov { WHITE } else { DIM },
            0.85,
        );
        self.hit(lx + tw - 140.0, ry + 2.0, 120.0, 20.0, Hit::EditConfig);
    }

    pub(crate) fn draw_wizard(&self, lx: f32, y: f32, tw: f32) {
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
                g.rrect(bx, ry, bw, 42.0, 12.0, CARD, 1.0);
                g.rrect_stroke(
                    bx + 0.5,
                    ry + 0.5,
                    bw - 1.0,
                    41.0,
                    12.0,
                    if self.wiz_err.is_some() { RED } else { WHITE },
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
                    if empty { DIM } else { WHITE },
                    1.0,
                );
                let cx = if empty {
                    bx + 14.0
                } else {
                    (bx + 14.0 + g.measure(&self.wiz_name, &g.f.input) + 1.5).min(bx + bw - 14.0)
                };
                g.line(cx, ry + 10.0, cx, ry + 32.0, WHITE, 0.95, 1.5);
                self.hit(bx, ry, bw, 42.0, Hit::Input);

                if let Some(err) = &self.wiz_err {
                    g.text(err, &g.f.sub, bx + 4.0, ry + 45.0, bw, 16.0, RED, 1.0);
                } else {
                    let preview = format!("{}\\{}", self.cfg.base_dir, self.wiz_name);
                    g.text(
                        &actions::short(&preview),
                        &g.f.sub,
                        bx + 4.0,
                        ry + 45.0,
                        bw,
                        16.0,
                        GRAY,
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
                    if self.wiz_github { GREEN } else { GRAY },
                    1.0,
                );
                g.text(
                    "Subir a GitHub automáticamente al crear (git init + gh repo create)",
                    &g.f.body,
                    lx + 54.0,
                    ry,
                    tw - 135.0,
                    40.0,
                    WHITE,
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
                    g.text(&t.name, &g.f.body, lx + 62.0, ry, tw - 130.0, 40.0, WHITE, 1.0);
                    if sel {
                        g.circle(lx + tw - 36.0, ry + 20.0, 9.0, WHITE, 1.0);
                        g.text(
                            G_CHECK,
                            &g.f.icon_sm,
                            lx + tw - 45.0,
                            ry + 11.0,
                            18.0,
                            18.0,
                            0x000000,
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
                            if on { WHITE } else { BORDER },
                            1.0,
                        );
                        if on {
                            g.text(G_CHECK, &g.f.icon_sm, lx + 26.0, ry + 10.0, 20.0, 20.0, 0x000000, 1.0);
                        }
                        g.text(&l.name, &g.f.body, lx + 56.0, ry, 200.0, 40.0, WHITE, 1.0);
                        g.text(&l.command, &g.f.sub_r, lx + 250.0, ry, tw - 276.0, 40.0, DIM, 1.0);
                        ry += 46.0;
                    }
                }
                self.row_bg(lx + 16.0, ry, tw - 32.0, 38.0, Hit::WizOpenLibEditor, false);
                g.text(G_ADD, &g.f.icon_sm, lx + 26.0, ry, 24.0, 38.0, GREEN, 1.0);
                g.text(
                    "Configurar o agregar nueva librería a este framework…",
                    &g.f.sub,
                    lx + 54.0,
                    ry,
                    tw - 80.0,
                    38.0,
                    GREEN,
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
                        WHITE,
                        if sel { 0.18 } else { 0.07 },
                    );
                    g.text(G_TERM, &g.f.icon_sm, lx + 24.0, ry + 6.0, 28.0, 28.0, WHITE, 1.0);
                    g.text(&l.name, &g.f.body, lx + 62.0, ry, 210.0, 40.0, WHITE, 1.0);
                    g.text(&l.command, &g.f.sub_r, lx + 270.0, ry, tw - 325.0, 40.0, DIM, 1.0);
                    if sel {
                        g.circle(lx + tw - 36.0, ry + 20.0, 9.0, WHITE, 1.0);
                        g.text(
                            G_CHECK,
                            &g.f.icon_sm,
                            lx + tw - 45.0,
                            ry + 11.0,
                            18.0,
                            18.0,
                            0x000000,
                            1.0,
                        );
                    }
                    ry += 46.0;
                }
                self.row_bg(lx + 16.0, ry, tw - 32.0, 38.0, Hit::WizOpenLauncherEditor, false);
                g.text(G_ADD, &g.f.icon_sm, lx + 26.0, ry, 24.0, 38.0, GREEN, 1.0);
                g.text(
                    "Agregar otro programa (.exe) a 'Abrir con'…",
                    &g.f.sub,
                    lx + 54.0,
                    ry,
                    tw - 80.0,
                    38.0,
                    GREEN,
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
            WHITE,
            if hov { 1.0 } else { 0.90 },
        );
        let btn = if self.step == Step::Launcher {
            "Crear y abrir proyecto"
        } else {
            "Continuar  →"
        };
        g.text(btn, &g.f.btn, lx + 16.0, ry, tw - 32.0, 42.0, 0x000000, 1.0);
        self.hit(lx + 16.0, ry, tw - 32.0, 42.0, Hit::Next);
    }

    pub(crate) fn draw_git_modal(&self, lx: f32, y: f32, tw: f32) {
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
            GRAY,
            1.0,
        );
        ry += 20.0;

        g.rrect(bx, ry, bw, 38.0, 10.0, CARD, 1.0);
        g.rrect_stroke(bx + 0.5, ry + 0.5, bw - 1.0, 37.0, 10.0, WHITE, 0.5, 1.0);
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
            if empty { DIM } else { WHITE },
            1.0,
        );
        let cx = if empty {
            bx + 12.0
        } else {
            (bx + 12.0 + g.measure(&self.git_commit_msg, &g.f.body) + 1.5).min(bx + bw - 14.0)
        };
        g.line(cx, ry + 9.0, cx, ry + 29.0, WHITE, 0.9, 1.4);
        self.hit(bx, ry, bw, 38.0, Hit::GitCommitInput);
        ry += 48.0;

        if !has_remote {
            self.row_bg(bx, ry, bw, 38.0, Hit::GitTogglePublic, false);
            let vis_txt = if self.git_public {
                "Crear repositorio en GitHub como: Público"
            } else {
                "Crear repositorio en GitHub como: Privado"
            };
            g.text(vis_txt, &g.f.body, bx + 14.0, ry, bw - 80.0, 38.0, WHITE, 1.0);
            self.toggle_pill(bx + bw - 50.0, ry + 9.0, self.git_public);
            ry += 46.0;
        }

        let hov_push = self.hov(Hit::GitDoCommitPush);
        g.rrect(bx, ry, bw, 42.0, 11.0, GREEN, if hov_push { 0.28 } else { 0.18 });
        g.rrect_stroke(bx + 0.5, ry + 0.5, bw - 1.0, 41.0, 11.0, GREEN, 0.75, 1.2);
        let push_label = if has_remote {
            "↑ Hacer Commit y Push a GitHub (Enter)"
        } else {
            "↑ Inicializar Git, Crear Repo en GitHub y hacer Push (Enter)"
        };
        g.text(push_label, &g.f.btn, bx, ry, bw, 42.0, GREEN, 1.0);
        self.hit(bx, ry, bw, 42.0, Hit::GitDoCommitPush);
        ry += 52.0;

        let half_w = (bw - 10.0) / 2.0;
        self.row_bg(bx, ry, half_w, 38.0, Hit::GitDoPull, false);
        g.text(G_DOWNLOAD, &g.f.icon_sm, bx + 10.0, ry, 24.0, 38.0, WHITE, 1.0);
        g.text("Git Pull (bajar cambios)", &g.f.body, bx + 38.0, ry, half_w - 44.0, 38.0, WHITE, 1.0);

        if has_remote {
            self.row_bg(bx + half_w + 10.0, ry, half_w, 38.0, Hit::GitOpenRepoWeb, false);
            g.text(G_GLOBE, &g.f.icon_sm, bx + half_w + 20.0, ry, 24.0, 38.0, WHITE, 1.0);
            g.text(
                "Abrir en GitHub.com",
                &g.f.body,
                bx + half_w + 48.0,
                ry,
                half_w - 54.0,
                38.0,
                WHITE,
                1.0,
            );
        } else {
            self.row_bg(bx + half_w + 10.0, ry, half_w, 38.0, Hit::GitOpenLogTerm, false);
            g.text(G_TERM, &g.f.icon_sm, bx + half_w + 20.0, ry, 24.0, 38.0, WHITE, 1.0);
            g.text(
                "Ver consola Git",
                &g.f.body,
                bx + half_w + 48.0,
                ry,
                half_w - 54.0,
                38.0,
                WHITE,
                1.0,
            );
        }
    }

    pub(crate) fn draw_note_modal(&self, lx: f32, y: f32, tw: f32) {
        let g = &self.gfx;
        let Some(p) = self.cfg.projects.get(self.note_proj_idx) else { return };
        self.header(
            lx,
            y,
            tw,
            &format!("Nota Rápida (Mini-TODO) · {}", p.name),
            "Se mostrará debajo del nombre del proyecto para recordar qué te falta",
            true,
        );

        let mut ry = y + 70.0;
        let bx = lx + 16.0;
        let bw = tw - 32.0;

        g.text(
            "Recordatorio o tarea pendiente del proyecto:",
            &g.f.sub,
            bx + 2.0,
            ry,
            bw,
            18.0,
            GRAY,
            1.0,
        );
        ry += 22.0;

        self.draw_input_box(
            bx,
            ry,
            bw,
            38.0,
            "Ej. Falta conectar pasarela de pagos / Revisar login…",
            &self.note_text,
            true,
            Hit::NoteInput,
        );
        ry += 52.0;

        let save_w = bw - 130.0;
        self.pill_action_btn(
            bx,
            ry,
            save_w,
            40.0,
            "✓ Guardar nota (Enter)",
            GREEN,
            true,
            Hit::NoteSave,
        );
        self.pill_action_btn(
            bx + save_w + 10.0,
            ry,
            120.0,
            40.0,
            "Borrar nota",
            RED,
            false,
            Hit::NoteClear,
        );
    }

    pub(crate) fn draw_custom_editor(&self, lx: f32, y: f32, tw: f32) {
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
                if active { WHITE } else { CARD },
                if active {
                    0.95
                } else if hov {
                    0.9
                } else {
                    0.6
                },
            );
            g.rrect_stroke(tx, ry, tab_w, 30.0, 8.0, BORDER, 0.8, 1.0);
            g.text(
                t_label,
                &g.f.badge,
                tx,
                ry,
                tab_w,
                30.0,
                if active { 0x000000 } else { WHITE },
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

                g.rrect(bx, ry, bw, 34.0, 9.0, CARD, 1.0);
                g.rrect_stroke(bx, ry, bw, 34.0, 9.0, BORDER, 0.8, 1.0);
                self.mini_action_btn(bx + 4.0, ry + 4.0, 26.0, G_BACK, WHITE, false, Hit::EdPrevTpl);
                g.text(
                    &format!("Framework: {tpl_name}"),
                    &g.f.badge,
                    bx + 36.0,
                    ry,
                    bw - 72.0,
                    34.0,
                    WHITE,
                    1.0,
                );
                self.mini_action_btn(
                    bx + bw - 30.0,
                    ry + 4.0,
                    26.0,
                    G_CHEV,
                    WHITE,
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
                    GREEN,
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
                        if *is_err { RED } else { GREEN },
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
                            if l.default { WHITE } else { BORDER },
                            1.0,
                        );
                        if l.default {
                            g.text(G_CHECK, &g.f.icon_sm, bx + 10.0, ry + 9.0, 20.0, 20.0, 0x000000, 1.0);
                        }
                        g.text(&l.name, &g.f.body, bx + 38.0, ry, 180.0, 38.0, WHITE, 1.0);
                        g.text(&l.command, &g.f.sub_r, bx + 215.0, ry, bw - 256.0, 38.0, DIM, 1.0);
                        self.mini_action_btn(
                            bx + bw - 32.0,
                            ry + 6.0,
                            26.0,
                            G_TRASH,
                            RED,
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
                    WHITE,
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
                    GREEN,
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
                        if *is_err { RED } else { GREEN },
                        1.0,
                    );
                    ry += 18.0;
                }

                let max_vis = 5usize;
                let max_sc = self.cfg.launchers.len().saturating_sub(max_vis);
                let sc = self.ed_scroll.min(max_sc);
                for (i, l) in self.cfg.launchers.iter().enumerate().skip(sc).take(max_vis) {
                    self.row_bg(bx, ry, bw, 40.0, Hit::Launcher(i), false);
                    g.text(&l.name, &g.f.body, bx + 14.0, ry, 175.0, 40.0, WHITE, 1.0);
                    g.text(&l.command, &g.f.sub_r, bx + 190.0, ry, bw - 230.0, 40.0, DIM, 1.0);
                    if self.cfg.launchers.len() > 1 {
                        self.mini_action_btn(
                            bx + bw - 32.0,
                            ry + 7.0,
                            26.0,
                            G_TRASH,
                            RED,
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
                    WHITE,
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
                    GREEN,
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
                        if *is_err { RED } else { GREEN },
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
                    g.text(&t.name, &g.f.body, lx + 60.0, ry, 180.0, 44.0, WHITE, 1.0);
                    let cmd_preview = t
                        .create
                        .first()
                        .or(t.setup.first())
                        .map(|s| s.as_str())
                        .unwrap_or("Carpeta limpia");
                    g.text(cmd_preview, &g.f.sub_r, lx + 240.0, ry, tw - 295.0, 44.0, DIM, 1.0);
                    if self.cfg.templates.len() > 1 {
                        self.mini_action_btn(
                            lx + tw - 44.0,
                            ry + 9.0,
                            26.0,
                            G_TRASH,
                            RED,
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

    pub(crate) fn draw_status(&self, lx: f32, y: f32, tw: f32, h: f32) {
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

        // Si está creando/clonando/subiendo, mostrar barra de progreso fina también en la parte inferior del banner
        if let Some(prog) = self.bg_progress {
            let bar_x = lx + 60.0;
            let bar_w = tw - 110.0;
            let bar_y = y + h - 7.0;
            g.rrect(bar_x, bar_y, bar_w, 3.0, 1.5, WHITE, 0.12);
            g.rrect(
                bar_x,
                bar_y,
                (bar_w * prog.clamp(0.05, 1.0)).max(8.0),
                3.0,
                1.5,
                GREEN,
                0.95,
            );
        }

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
}

