use crate::actions;
use crate::app::*;
use crate::config::hex;

impl App {
    pub(crate) fn draw(&self) {
        let g = &self.gfx;
        let (w, h, r) = (self.sw.x.max(30.0), self.sh.x.max(8.0), self.sr.x.max(3.0));
        let cx = WIN_W / 2.0;
        let y = TOP;
        g.alpha.set(1.0);

        let is_compact = self.view == View::Compact && w < 260.0 && h < 42.0;

        if is_compact {
            let glow_t = self.glow_anim.sin() * 0.5 + 0.5;
            if self.glow_anim > 0.01 {
                g.draw_glow_ring(cx, w, h, glow_t);
            }
            if let Ok(path) = g.waterdrop_path(cx, w, h) {
                g.fill_path(&path, BODY, 1.0);
            }
            if let Ok(curve) = g.waterdrop_curve_path(cx, w, h) {
                let border_col = if self.has_live_server() {
                    GREEN
                } else {
                    WHITE
                };
                let border_alpha = if self.glow_anim > 0.01 {
                    0.28 + 0.25 * glow_t
                } else if self.has_live_server() {
                    0.35
                } else {
                    0.15
                };
                g.stroke_path(&curve, border_col, border_alpha, 1.0);
            }

            // Barra de progreso luminosa en el borde inferior curvo de la gota (cuando crea/clona/sube en 2do plano)
            if let Some(prog) = self.bg_progress {
                g.draw_waterdrop_progress(cx, w, h, prog, GREEN);
            }
        } else {
            let x = cx - w / 2.0;
            if let Ok(shadow) = g.top_panel_path(x - 2.0, w + 4.0, h + 3.0, r + 2.0) {
                g.fill_path(&shadow, 0x000000, 0.55);
            }
            if let Ok(panel) = g.top_panel_path(x, w, h, r) {
                g.fill_path(&panel, BODY, 0.99);
            }
            if let Ok(border) = g.top_panel_curve_path(x + 0.5, w - 1.0, h - 0.5, r) {
                g.stroke_path(&border, WHITE, 0.14, 1.0);
            }
        }

        let (tw, th, _) = self.target();
        let lx = cx - tw / 2.0;
        let a = self.sa.x.clamp(0.0, 1.0);
        g.alpha.set(a);

        let clip_x = cx - w / 2.0;
        let clipped = !is_compact;
        if clipped {
            g.clip(clip_x, y, w, h);
        }
        match self.view {
            View::Compact => self.draw_compact(cx, y, w, h),
            View::Expanded => self.draw_expanded(lx, y, tw),
            View::Settings => self.draw_settings(lx, y, tw),
            View::Wizard => self.draw_wizard(lx, y, tw),
            View::GitModal => self.draw_git_modal(lx, y, tw),
            View::NoteModal => self.draw_note_modal(lx, y, tw),
            View::CustomEditor => self.draw_custom_editor(lx, y, tw),
            View::Status => self.draw_status(lx, y, tw, th),
        }
        if self.view != View::Compact {
            self.draw_tooltip(cx - w / 2.0, y, w, h);
        }
        if clipped {
            g.unclip();
        }
    }

    pub(crate) fn draw_compact(&self, cx: f32, y: f32, w: f32, h: f32) {
        let g = &self.gfx;
        let srv_count = self.active_server_count();
        let live = srv_count > 0;
        let expanded_pill = h > 16.0;

        if live {
            let dot_r = if expanded_pill { 3.4 } else { 2.6 };
            let positions = Self::waterdrop_dot_positions(cx, w, h, srv_count, dot_r);
            for (dx, dy) in positions {
                g.circle(dx, y + dy, dot_r + 1.8, GREEN, 0.25);
                g.circle(dx, y + dy, dot_r, GREEN, 1.0);
            }
        }

        let hit_w = w.max(96.0);
        let hit_h = h.max(26.0);
        self.hit(cx - hit_w / 2.0, 0.0, hit_w, hit_h, Hit::Pill);
    }

    pub(crate) fn draw_expanded(&self, lx: f32, y: f32, tw: f32) {
        let g = &self.gfx;
        let srv_count = self.active_server_count();
        let open_count = self.open_windows.len();
        let sub = match (srv_count > 0, open_count > 0) {
            (true, true) => format!(
                "{} proyectos · {} {} · {} {}",
                self.cfg.projects.len(),
                open_count,
                if open_count == 1 { "abierto" } else { "abiertos" },
                srv_count,
                if srv_count == 1 { "servidor" } else { "servidores" }
            ),
            (true, false) => format!(
                "{} proyectos · {} {}",
                self.cfg.projects.len(),
                srv_count,
                if srv_count == 1 { "servidor activo" } else { "servidores activos" }
            ),
            (false, true) => format!(
                "{} proyectos · {} {}",
                self.cfg.projects.len(),
                open_count,
                if open_count == 1 { "abierto" } else { "abiertos" }
            ),
            (false, false) => format!("{} proyectos", self.cfg.projects.len()),
        };
        self.header(lx, y, tw, "Mis Proyectos", &sub, false);

        let mut cur_y = y + 62.0;

        // 1. Barra de Puertos Locales Activos
        if !self.active_ports.is_empty() {
            g.circle(lx + 24.0, cur_y + 12.0, 3.8, GREEN, 1.0);
            g.text("PUERTOS:", &g.f.tiny, lx + 34.0, cur_y, 65.0, 24.0, GRAY, 1.0);
            let mut px = lx + 96.0;
            for &port in self.active_ports.iter().take(7) {
                let label = format!(":{port}");
                let pw = 88.0;
                let bh = 22.0;
                let hov_open = self.hov(Hit::PortBadge(port));
                let hov_term = self.hov(Hit::PortTerm(port));
                let hov_kill = self.hov(Hit::KillPort(port));
                g.rrect(
                    px,
                    cur_y + 1.0,
                    pw,
                    bh,
                    7.0,
                    GREEN,
                    if hov_open || hov_term { 0.22 } else { 0.12 },
                );
                g.rrect_stroke(px, cur_y + 1.0, pw, bh, 7.0, GREEN, 0.45, 1.0);
                g.text(&label, &g.f.badge, px + 7.0, cur_y + 1.0, 42.0, bh, GREEN, 1.0);
                self.hit(px, cur_y + 1.0, 45.0, bh, Hit::PortBadge(port));

                g.rrect(
                    px + 46.0,
                    cur_y + 3.0,
                    18.0,
                    18.0,
                    4.0,
                    WHITE,
                    if hov_term { 0.18 } else { 0.0 },
                );
                g.text(
                    G_TERM,
                    &g.f.icon_sm,
                    px + 46.0,
                    cur_y + 1.0,
                    18.0,
                    bh,
                    if hov_term { WHITE } else { GREEN },
                    0.95,
                );
                self.hit(px + 46.0, cur_y + 1.0, 18.0, bh, Hit::PortTerm(port));

                g.text(
                    G_CLOSE,
                    &g.f.icon_sm,
                    px + pw - 21.0,
                    cur_y + 1.0,
                    19.0,
                    bh,
                    if hov_kill { RED } else { GRAY },
                    0.9,
                );
                self.hit(px + pw - 21.0, cur_y + 1.0, 20.0, bh, Hit::KillPort(port));
                px += pw + 6.0;
            }
            cur_y += 32.0;
        }

        // 2. Buscador Instantáneo ("Spotlight") con Keycaps discretos a la derecha
        let sb_x = lx + 16.0;
        let sb_w = tw - 32.0;
        let sb_h = 34.0;
        let has_q = !self.search.is_empty();
        g.rrect(sb_x, cur_y, sb_w, sb_h, 10.0, CARD, 1.0);
        g.rrect_stroke(
            sb_x + 0.5,
            cur_y + 0.5,
            sb_w - 1.0,
            sb_h - 1.0,
            10.0,
            if has_q { WHITE } else { BORDER },
            if has_q { 0.45 } else { 0.8 },
            1.0,
        );
        g.text(G_SEARCH, &g.f.icon_sm, sb_x + 8.0, cur_y, 24.0, sb_h, GRAY, 0.9);
        if has_q {
            g.text(&self.search, &g.f.body, sb_x + 34.0, cur_y, sb_w - 68.0, sb_h, WHITE, 1.0);
            let tx = (sb_x + 34.0 + g.measure(&self.search, &g.f.body) + 1.5).min(sb_x + sb_w - 36.0);
            g.line(tx, cur_y + 8.0, tx, cur_y + sb_h - 8.0, WHITE, 0.9, 1.4);
            let clr_hov = self.hov(Hit::ClearSearch);
            g.text(
                G_CLOSE,
                &g.f.icon_sm,
                sb_x + sb_w - 30.0,
                cur_y,
                26.0,
                sb_h,
                if clr_hov { WHITE } else { GRAY },
                1.0,
            );
            self.hit(sb_x + sb_w - 30.0, cur_y, 26.0, sb_h, Hit::ClearSearch);
        } else {
            g.text(
                "Buscar proyecto, stack, rama o nota…",
                &g.f.sub,
                sb_x + 34.0,
                cur_y,
                sb_w - 220.0,
                sb_h,
                DIM,
                1.0,
            );
            // Keycaps discretos a la derecha del buscador: [↑↓] [↵ Abrir] [Ctrl N]
            let mut kx = sb_x + sb_w - 8.0;
            for keycap in &["Ctrl N", "↵ Abrir", "↑↓"] {
                let kw = g.measure(keycap, &g.f.tiny) + 12.0;
                kx -= kw;
                g.rrect(kx, cur_y + 7.0, kw, 20.0, 5.0, WHITE, 0.05);
                g.rrect_stroke(kx, cur_y + 7.0, kw, 20.0, 5.0, BORDER, 0.85, 1.0);
                g.text(keycap, &g.f.tiny_c, kx, cur_y + 7.0, kw, 20.0, DIM, 0.95);
                kx -= 5.0;
            }
        }
        self.hit(sb_x, cur_y, sb_w - 32.0, sb_h, Hit::SearchBox);
        cur_y += 40.0;

        // 3. Fila de Filtros Rápidos por Chips + Botón 1-Click Git Clone desde Portapapeles
        let mut chip_x = lx + 16.0;
        let chip_h = 22.0;
        let dirty_total = self.git_states.values().filter(|g| g.dirty).count();
        let pinned_total = self.cfg.projects.iter().filter(|p| p.pinned).count();
        let fs_total = self.fullstack_map.values().filter(|f| f.is_fullstack()).count();

        let chips: &[(FilterMode, String, u32, bool)] = &[
            (
                FilterMode::All,
                format!("Todos ({})", self.cfg.projects.len()),
                WHITE,
                false,
            ),
            (
                FilterMode::Pinned,
                format!("📌 Fijados ({pinned_total})"),
                WHITE,
                false,
            ),
            (
                FilterMode::GitDirty,
                format!("Cambios Git ({dirty_total})"),
                if dirty_total > 0 { AMBER } else { GRAY },
                true,
            ),
            (
                FilterMode::RunningServer,
                format!("⚡ Con Servidor ({srv_count})"),
                if srv_count > 0 { GREEN } else { GRAY },
                false,
            ),
            (
                FilterMode::FullStack,
                format!("🔗 Full-Stack ({fs_total})"),
                CYAN,
                false,
            ),
        ];

        for (mode, label, accent_col, has_git_icon) in chips {
            let active = self.filter_mode == *mode;
            let hov = self.hov(Hit::FilterChip(*mode));
            let extra_icon_w = if *has_git_icon { 15.0 } else { 0.0 };
            let cw = g.measure(label, &g.f.tiny) + 18.0 + extra_icon_w;
            g.rrect(
                chip_x,
                cur_y,
                cw,
                chip_h,
                7.0,
                if active { *accent_col } else { CARD },
                if active {
                    0.18
                } else if hov {
                    0.95
                } else {
                    0.75
                },
            );
            g.rrect_stroke(
                chip_x + 0.5,
                cur_y + 0.5,
                cw - 1.0,
                chip_h - 1.0,
                7.0,
                if active || hov { *accent_col } else { BORDER },
                if active { 0.75 } else { 0.65 },
                1.0,
            );
            let txt_col = if active {
                *accent_col
            } else if hov {
                WHITE
            } else {
                GRAY
            };
            if *has_git_icon {
                g.draw_git_branch_icon(chip_x + 7.0, cur_y + 4.5, 13.0, txt_col, 0.95);
                g.text(
                    label,
                    &g.f.tiny,
                    chip_x + 22.0,
                    cur_y,
                    cw - 26.0,
                    chip_h,
                    txt_col,
                    1.0,
                );
            } else {
                g.text(label, &g.f.tiny_c, chip_x, cur_y, cw, chip_h, txt_col, 1.0);
            }
            self.hit(chip_x, cur_y, cw, chip_h, Hit::FilterChip(*mode));
            chip_x += cw + 6.0;
        }

        // Si hay una URL de GitHub en el portapapeles, mostrar pill directo a la derecha de los filtros
        if let Some((repo_name, _)) = &self.clipboard_repo {
            let clone_lbl = format!("📥 Clonar {repo_name}");
            let max_clone_w = (lx + tw - 16.0 - chip_x).max(0.0);
            if max_clone_w >= 110.0 {
                let cw = (g.measure(&clone_lbl, &g.f.tiny) + 18.0).min(max_clone_w);
                let cx_btn = lx + tw - 16.0 - cw;
                let hov = self.hov(Hit::CloneClipboardRepo);
                g.rrect(cx_btn, cur_y, cw, chip_h, 7.0, GREEN, if hov { 0.25 } else { 0.14 });
                g.rrect_stroke(cx_btn + 0.5, cur_y + 0.5, cw - 1.0, chip_h - 1.0, 7.0, GREEN, 0.65, 1.0);
                g.text(&clone_lbl, &g.f.tiny_c, cx_btn + 4.0, cur_y, cw - 8.0, chip_h, GREEN, 1.0);
                self.hit(cx_btn, cur_y, cw, chip_h, Hit::CloneClipboardRepo);
            }
        }
        cur_y += 30.0;

        // 4. Layout en dos columnas balanceadas:
        //    - Izquierda: Lista de Proyectos (con Micro-Badges, Hover-Reveal Linear style, Full-Stack combos y Scrollbar 3px)
        //    - Derecha: Dos tarjetas que siempre igualan la altura exacta de la columna izquierda:
        //        1) ABIERTOS AHORA
        //        2) SERVIDORES Y ACCESOS RÁPIDOS
        let right_w = 236.0;
        let left_w = tw - right_w - 10.0;
        let right_x = lx + left_w + 2.0;

        let filtered = self.filtered_projects();
        let max_s = filtered.len().saturating_sub(MAX_ROWS);
        let sc = self.scroll.min(max_s);
        let sel = self.sel_idx.min(filtered.len().saturating_sub(1));

        let list_top_y = cur_y;
        let mut ry = cur_y;

        if filtered.is_empty() {
            let empty_h = 3.0 * 54.0 - 6.0;
            g.rrect(lx + 16.0, ry, left_w - 24.0, empty_h, 14.0, CARD, 0.5);
            let msg = if self.cfg.projects.is_empty() {
                "No tienes proyectos aún · Crea el primero abajo"
            } else {
                "Sin resultados para este filtro · Pulsa Esc para limpiar"
            };
            g.text(msg, &g.f.sub_c, lx + 16.0, ry, left_w - 24.0, empty_h, GRAY, 1.0);
            ry += 3.0 * 54.0;
        } else {
            for (vis_pos, &i) in filtered.iter().enumerate().skip(sc).take(MAX_ROWS) {
                let p = &self.cfg.projects[i];
                let is_sel = vis_pos == sel && !self.search.is_empty();
                let row_hov = self.is_row_hov(i);
                let row_x = lx + 16.0;
                let row_w = left_w - 24.0;
                let row_h = 48.0;

                // Fondo de la fila + Acento lateral de 3px del color de la tecnología al hacer hover
                g.rrect(
                    row_x,
                    ry,
                    row_w,
                    row_h,
                    13.0,
                    if row_hov || is_sel { CARD_HOV } else { CARD },
                    1.0,
                );
                let stack_col = hex(&p.color);
                if row_hov || is_sel {
                    g.rrect_stroke(
                        row_x + 0.5,
                        ry + 0.5,
                        row_w - 1.0,
                        row_h - 1.0,
                        13.0,
                        stack_col,
                        0.42,
                        1.0,
                    );
                    // Barra vertical de acento (3px) en el color oficial de su stack
                    g.rrect(row_x + 2.5, ry + 8.0, 3.2, row_h - 16.0, 1.6, stack_col, 0.95);
                } else {
                    g.rrect_stroke(row_x + 0.5, ry + 0.5, row_w - 1.0, row_h - 1.0, 13.0, BORDER, 0.65, 1.0);
                }
                self.hit(row_x, ry, row_w, row_h, Hit::Project(i));

                // Icono vectorial oficial del stack
                g.draw_stack_badge(&p.stack, lx + 24.0, ry + 7.0, 34.0);

                let git_info = self.git_states.get(&p.path);
                let fs_info = self.fullstack_map.get(&p.path);
                let is_fullstack = fs_info.map(|f| f.is_fullstack()).unwrap_or(false);

                let open_progs: Vec<&str> = self
                    .open_windows
                    .iter()
                    .filter(|ow| ow.project_idx == i)
                    .map(|ow| ow.program_name.as_str())
                    .collect();

                // Calcular espacio ocupado por botones a la derecha (Estilo Linear / Raycast: limpio en reposo, completo en hover)
                let mut rx = row_x + row_w - 8.0;
                let btn_y = ry + 10.0;
                let btn_h = 28.0;

                if row_hov {
                    // --- FILA EN HOVER: Mostrar todos los controles interactivos con sus cajas ---
                    rx -= 28.0;
                    self.mini_action_btn(
                        rx,
                        btn_y,
                        28.0,
                        G_PIN,
                        if p.pinned { WHITE } else { GRAY },
                        p.pinned,
                        Hit::ProjPin(i),
                    );

                    rx -= 31.0;
                    self.mini_action_btn(rx, btn_y, 28.0, G_FOLDER, GRAY, false, Hit::ProjExplorer(i));

                    rx -= 31.0;
                    self.mini_action_btn(rx, btn_y, 28.0, G_TERM, GRAY, false, Hit::ProjTerm(i));

                    rx -= 31.0;
                    self.mini_action_btn(
                        rx,
                        btn_y,
                        28.0,
                        G_EDIT,
                        if p.note.is_some() { AMBER } else { GRAY },
                        p.note.is_some(),
                        Hit::ProjNote(i),
                    );

                    // Botón de Git con icono vectorial real de rama Git (nunca ▯)
                    let is_dirty = git_info.map(|g| g.dirty).unwrap_or(false);
                    rx -= 32.0;
                    let git_hov = self.hov(Hit::ProjGit(i));
                    g.rrect(
                        rx,
                        btn_y,
                        28.0,
                        btn_h,
                        8.0,
                        if is_dirty { AMBER } else { WHITE },
                        if git_hov {
                            0.18
                        } else if is_dirty {
                            0.12
                        } else {
                            0.05
                        },
                    );
                    g.rrect_stroke(
                        rx + 0.5,
                        btn_y + 0.5,
                        27.0,
                        btn_h - 1.0,
                        8.0,
                        if is_dirty {
                            AMBER
                        } else if git_hov {
                            WHITE
                        } else {
                            BORDER
                        },
                        if git_hov || is_dirty { 0.7 } else { 0.85 },
                        1.0,
                    );
                    g.draw_git_branch_icon(
                        rx + 6.5,
                        btn_y + 6.5,
                        15.0,
                        if is_dirty {
                            AMBER
                        } else if git_hov {
                            WHITE
                        } else {
                            GRAY
                        },
                        1.0,
                    );
                    if is_dirty {
                        g.circle(rx + 23.0, btn_y + 5.0, 3.0, AMBER, 1.0);
                    }
                    self.hit(rx, btn_y, 28.0, btn_h, Hit::ProjGit(i));

                    // Controles de Servidor (Full-Stack F / B / 2x o Individual)
                    if is_fullstack {
                        if let Some(fs) = fs_info {
                            let f_srv = fs
                                .front
                                .as_ref()
                                .and_then(|t| self.servers.get(&t.cwd.to_string_lossy().to_string()));
                            let b_srv = fs
                                .back
                                .as_ref()
                                .and_then(|t| self.servers.get(&t.cwd.to_string_lossy().to_string()));
                            let both_on = f_srv.is_some() && b_srv.is_some();

                            // Botón Combo 2x
                            rx -= 38.0;
                            self.pill_action_btn(
                                rx,
                                btn_y,
                                34.0,
                                btn_h,
                                "⚡2x",
                                if both_on { GREEN } else { CYAN },
                                both_on,
                                Hit::ProjServerRole(i, ServerRole::Both),
                            );

                            // Botón Back
                            let b_lbl = if let Some(s) = b_srv {
                                s.port.map(|pt| format!("B:{pt}")).unwrap_or_else(|| "B:ON".into())
                            } else {
                                "▶B".into()
                            };
                            let bw = if b_srv.is_some() { 50.0 } else { 32.0 };
                            rx -= bw + 4.0;
                            self.pill_action_btn(
                                rx,
                                btn_y,
                                bw,
                                btn_h,
                                &b_lbl,
                                if b_srv.is_some() { CYAN } else { GRAY },
                                b_srv.is_some(),
                                Hit::ProjServerRole(i, ServerRole::Back),
                            );

                            // Botón Front
                            let f_lbl = if let Some(s) = f_srv {
                                s.port.map(|pt| format!("F:{pt}")).unwrap_or_else(|| "F:ON".into())
                            } else {
                                "▶F".into()
                            };
                            let fw = if f_srv.is_some() { 50.0 } else { 32.0 };
                            rx -= fw + 4.0;
                            self.pill_action_btn(
                                rx,
                                btn_y,
                                fw,
                                btn_h,
                                &f_lbl,
                                if f_srv.is_some() { GREEN } else { GRAY },
                                f_srv.is_some(),
                                Hit::ProjServerRole(i, ServerRole::Front),
                            );
                        }
                    } else if let Some(srv) = self.servers.get(&p.path) {
                        rx -= 31.0;
                        self.mini_action_btn(rx, btn_y, 28.0, G_STOP, RED, true, Hit::ProjServer(i));
                        rx -= 31.0;
                        self.mini_action_btn(rx, btn_y, 28.0, G_TERM, GREEN, true, Hit::ProjServerTerm(i));
                        let port_txt = srv
                            .port
                            .map(|pt| format!("● :{pt}"))
                            .unwrap_or_else(|| "● LIVE".into());
                        let pw = 62.0;
                        rx -= pw + 4.0;
                        let hit_id = srv
                            .port
                            .map(|pt| Hit::ProjPort(i, pt))
                            .unwrap_or(Hit::ProjServerTerm(i));
                        self.pill_action_btn(rx, btn_y, pw, btn_h, &port_txt, GREEN, true, hit_id);
                    } else if fs_info.and_then(|f| f.main.as_ref()).is_some() {
                        rx -= 32.0;
                        self.mini_action_btn(rx, btn_y, 28.0, G_PLAY, GREEN, false, Hit::ProjServer(i));
                    }
                } else {
                    // --- FILA EN REPOSO (Estilo Linear / Raycast): Solo mostrar indicadores activos sin ruido de cajas ---
                    if p.pinned {
                        rx -= 24.0;
                        g.text(G_PIN, &g.f.icon_sm, rx, btn_y, 22.0, btn_h, GRAY, 0.75);
                        self.hit(rx, btn_y, 22.0, btn_h, Hit::ProjPin(i));
                    }

                    if let Some(git) = git_info {
                        if git.dirty {
                            let badge_txt = format!("{}", git.changed_count.max(1));
                            let gw = 38.0;
                            rx -= gw + 4.0;
                            g.rrect(rx, btn_y + 3.0, gw, 22.0, 6.0, AMBER, 0.12);
                            g.rrect_stroke(rx, btn_y + 3.0, gw, 22.0, 6.0, AMBER, 0.45, 1.0);
                            g.draw_git_branch_icon(rx + 5.0, btn_y + 7.5, 12.0, AMBER, 0.95);
                            g.text(&badge_txt, &g.f.tiny, rx + 19.0, btn_y + 3.0, gw - 21.0, 22.0, AMBER, 1.0);
                            self.hit(rx, btn_y, gw, btn_h, Hit::ProjGit(i));
                        }
                    }

                    // Si tiene servidor corriendo en esta fila, mostrar píldora activa siempre visible
                    if is_fullstack {
                        if let Some(fs) = fs_info {
                            if let Some(b_srv) = fs
                                .back
                                .as_ref()
                                .and_then(|t| self.servers.get(&t.cwd.to_string_lossy().to_string()))
                            {
                                let lbl = b_srv.port.map(|pt| format!("● B:{pt}")).unwrap_or_else(|| "● B".into());
                                rx -= 56.0;
                                self.pill_action_btn(
                                    rx,
                                    btn_y + 2.0,
                                    52.0,
                                    24.0,
                                    &lbl,
                                    CYAN,
                                    true,
                                    Hit::ProjServerRole(i, ServerRole::Back),
                                );
                            }
                            if let Some(f_srv) = fs
                                .front
                                .as_ref()
                                .and_then(|t| self.servers.get(&t.cwd.to_string_lossy().to_string()))
                            {
                                let lbl = f_srv.port.map(|pt| format!("● F:{pt}")).unwrap_or_else(|| "● F".into());
                                rx -= 56.0;
                                self.pill_action_btn(
                                    rx,
                                    btn_y + 2.0,
                                    52.0,
                                    24.0,
                                    &lbl,
                                    GREEN,
                                    true,
                                    Hit::ProjServerRole(i, ServerRole::Front),
                                );
                            }
                        }
                    } else if let Some(srv) = self.servers.get(&p.path) {
                        let port_txt = srv
                            .port
                            .map(|pt| format!("● :{pt}"))
                            .unwrap_or_else(|| "● LIVE".into());
                        let pw = 58.0;
                        rx -= pw + 4.0;
                        let hit_id = srv
                            .port
                            .map(|pt| Hit::ProjPort(i, pt))
                            .unwrap_or(Hit::ProjServerTerm(i));
                        self.pill_action_btn(rx, btn_y + 2.0, pw, 24.0, &port_txt, GREEN, true, hit_id);
                    }
                }

                // --- TEXTO E INDICADORES MICRO-BADGES A LA IZQUIERDA ---
                let text_x = lx + 68.0;
                let max_text_w = (rx - text_x - 8.0).max(80.0);

                // Línea 1: Nombre del proyecto + Micro-cápsula verde independiente si está abierto en un IDE
                let name_w = g.measure(&p.name, &g.f.body).min(max_text_w - 10.0);
                g.text(&p.name, &g.f.body, text_x, ry + 5.0, name_w + 6.0, 20.0, WHITE, 1.0);

                let mut badge_l1_x = text_x + name_w + 8.0;
                if !open_progs.is_empty() && badge_l1_x + 56.0 < rx {
                    let prog_txt = open_progs[0];
                    let pw = (g.measure(prog_txt, &g.f.tiny) + 18.0).min(rx - badge_l1_x - 4.0);
                    g.rrect(badge_l1_x, ry + 6.5, pw, 17.0, 8.5, GREEN, 0.14);
                    g.rrect_stroke(badge_l1_x, ry + 6.5, pw, 17.0, 8.5, GREEN, 0.45, 1.0);
                    g.circle(badge_l1_x + 7.0, ry + 15.0, 2.5, GREEN, 1.0);
                    g.text(
                        prog_txt,
                        &g.f.tiny,
                        badge_l1_x + 12.5,
                        ry + 6.5,
                        pw - 15.0,
                        17.0,
                        GREEN,
                        1.0,
                    );
                    badge_l1_x += pw + 6.0;
                }
                let _ = badge_l1_x;

                // Línea 2: Stack en gris neutro + Micro-Badge de rama Git con icono vectorial + Micro-Badge Full-Stack + Nota rápida
                let sub_y = ry + 25.0;
                let stack_label = if p.stack == "Solo carpeta" {
                    "Carpeta"
                } else {
                    p.stack.as_str()
                };
                let st_w = g.measure(stack_label, &g.f.sub).min(max_text_w);
                g.text(stack_label, &g.f.sub, text_x, sub_y, st_w + 4.0, 18.0, GRAY, 0.95);
                let mut m_x = text_x + st_w + 8.0;

                // Micro-badge de rama Git con icono vectorial real
                if let Some(git) = git_info {
                    let br_w = (g.measure(&git.branch, &g.f.tiny) + 21.0).min(110.0);
                    if m_x + br_w < rx {
                        let br_col = if git.dirty { AMBER } else { GRAY };
                        g.rrect(
                            m_x,
                            sub_y + 1.0,
                            br_w,
                            16.0,
                            5.0,
                            if git.dirty { AMBER } else { WHITE },
                            if git.dirty { 0.14 } else { 0.06 },
                        );
                        g.draw_git_branch_icon(m_x + 4.5, sub_y + 3.5, 11.0, br_col, 0.95);
                        g.text(
                            &git.branch,
                            &g.f.tiny,
                            m_x + 16.5,
                            sub_y + 1.0,
                            br_w - 19.0,
                            16.0,
                            br_col,
                            0.95,
                        );
                        m_x += br_w + 6.0;
                    }
                }

                // Micro-badge Full-Stack si tiene Front + Back
                if is_fullstack && m_x + 56.0 < rx {
                    let fsw = 54.0;
                    g.rrect(m_x, sub_y + 1.0, fsw, 16.0, 5.0, CYAN, 0.12);
                    g.text("Full-Stack", &g.f.tiny_c, m_x, sub_y + 1.0, fsw, 16.0, CYAN, 0.95);
                    m_x += fsw + 6.0;
                }

                // Micro-badge de Nota / Recordatorio Rápido (Mini-TODO)
                if let Some(note) = p.note.as_deref() {
                    if !note.is_empty() && m_x + 45.0 < rx {
                        let rem_w = (rx - m_x - 4.0).max(40.0);
                        let note_lbl = format!("📝 {note}");
                        let nw = (g.measure(&note_lbl, &g.f.tiny) + 12.0).min(rem_w);
                        g.rrect(m_x, sub_y + 1.0, nw, 16.0, 5.0, AMBER, 0.13);
                        g.text(&note_lbl, &g.f.tiny, m_x + 5.0, sub_y + 1.0, nw - 8.0, 16.0, AMBER, 0.95);
                    }
                }

                ry += 54.0;
            }
        }

        // 5. Barra de Scroll minimalista (3 px, estilo macOS) al costado de la lista de proyectos
        if filtered.len() > MAX_ROWS {
            let track_x = lx + left_w - 5.0;
            let track_y = list_top_y + 4.0;
            let track_h = (MAX_ROWS as f32) * 54.0 - 14.0;
            g.rrect(track_x, track_y, 3.0, track_h, 1.5, WHITE, 0.07);
            let ratio = (MAX_ROWS as f32) / (filtered.len() as f32);
            let thumb_h = (track_h * ratio).max(24.0);
            let max_scroll = (filtered.len() - MAX_ROWS).max(1) as f32;
            let thumb_y = track_y + (track_h - thumb_h) * (sc as f32 / max_scroll);
            g.rrect(track_x, thumb_y, 3.0, thumb_h, 1.5, GRAY, 0.65);
        }

        // Botón inferior izquierdo: Crear nuevo proyecto
        ry += 4.0;
        let new_btn_h = 46.0;
        self.row_bg(lx + 16.0, ry, left_w - 24.0, new_btn_h, Hit::NewProject, false);
        g.rrect(lx + 24.0, ry + 7.0, 32.0, 32.0, 9.0, WHITE, 0.08);
        g.rrect_stroke(lx + 24.0, ry + 7.0, 32.0, 32.0, 9.0, BORDER, 0.9, 1.0);
        g.text(G_ADD, &g.f.icon, lx + 24.0, ry + 7.0, 32.0, 32.0, WHITE, 1.0);
        g.text(
            "Crear nuevo proyecto",
            &g.f.body,
            lx + 68.0,
            ry,
            left_w - 165.0,
            new_btn_h,
            WHITE,
            1.0,
        );
        g.rrect(lx + left_w - 82.0, ry + 12.0, 62.0, 22.0, 6.0, CARD_HOV, 1.0);
        g.rrect_stroke(lx + left_w - 82.0, ry + 12.0, 62.0, 22.0, 6.0, BORDER, 0.9, 1.0);
        g.text("Ctrl N", &g.f.badge, lx + left_w - 82.0, ry + 12.0, 62.0, 22.0, GRAY, 1.0);

        let left_bottom_y = ry + new_btn_h;

        // ====================================================================
        // 6. COLUMNA DERECHA BALANCEADA EN DOS TARJETAS (Cero hueco negro vacío)
        // ====================================================================
        let panel_w = right_w - 16.0;
        let total_right_h = (left_bottom_y - list_top_y).max(220.0);

        // Tarjeta Superior: ABIERTOS AHORA
        let max_open_rows = if self.open_windows.len() <= 2 { 2 } else { 3 };
        let open_rows_shown = self.open_windows.len().min(max_open_rows);
        let top_card_h = if self.open_windows.is_empty() {
            86.0
        } else {
            34.0 + (open_rows_shown as f32) * 46.0 + 6.0
        }
        .min(total_right_h - 115.0);

        g.rrect(right_x, list_top_y, panel_w, top_card_h, 14.0, CARD, 0.65);
        g.rrect_stroke(
            right_x + 0.5,
            list_top_y + 0.5,
            panel_w - 1.0,
            top_card_h - 1.0,
            14.0,
            BORDER,
            0.7,
            1.0,
        );

        g.circle(
            right_x + 14.0,
            list_top_y + 16.0,
            3.5,
            if self.open_windows.is_empty() { DIM } else { GREEN },
            1.0,
        );
        let open_hdr = format!("ABIERTOS AHORA ({})", self.open_windows.len());
        g.text(
            &open_hdr,
            &g.f.tiny,
            right_x + 24.0,
            list_top_y + 4.0,
            panel_w - 30.0,
            24.0,
            GRAY,
            1.0,
        );

        let mut oy = list_top_y + 30.0;
        if self.open_windows.is_empty() {
            g.text(
                "Ningún IDE abierto ahora",
                &g.f.sub_c,
                right_x + 10.0,
                oy + 4.0,
                panel_w - 20.0,
                20.0,
                GRAY,
                0.85,
            );
            g.text(
                "Al abrir un proyecto aparecerá aquí",
                &g.f.tiny_c,
                right_x + 10.0,
                oy + 24.0,
                panel_w - 20.0,
                16.0,
                DIM,
                0.9,
            );
        } else {
            for (idx, ow) in self.open_windows.iter().enumerate().take(open_rows_shown) {
                let card_x = right_x + 7.0;
                let card_w = panel_w - 14.0;
                let card_h = 42.0;
                let hov_focus = self.hov(Hit::OpenWinFocus(idx));
                let hov_close = self.hov(Hit::OpenWinClose(idx));
                g.rrect(
                    card_x,
                    oy,
                    card_w,
                    card_h,
                    10.0,
                    if hov_focus { CARD_HOV } else { BODY },
                    0.85,
                );
                g.rrect_stroke(
                    card_x + 0.5,
                    oy + 0.5,
                    card_w - 1.0,
                    card_h - 1.0,
                    10.0,
                    if hov_focus { GREEN } else { BORDER },
                    if hov_focus { 0.55 } else { 0.65 },
                    1.0,
                );
                g.draw_stack_badge(&ow.stack, card_x + 6.0, oy + 7.0, 28.0);
                g.text(
                    &ow.project_name,
                    &g.f.body,
                    card_x + 40.0,
                    oy + 3.0,
                    card_w - 68.0,
                    19.0,
                    WHITE,
                    1.0,
                );
                g.text(
                    &format!("● {}", ow.program_name),
                    &g.f.tiny,
                    card_x + 40.0,
                    oy + 21.0,
                    card_w - 68.0,
                    16.0,
                    GREEN,
                    0.95,
                );
                self.hit(card_x, oy, card_w - 28.0, card_h, Hit::OpenWinFocus(idx));

                g.rrect(
                    card_x + card_w - 25.0,
                    oy + 9.0,
                    20.0,
                    24.0,
                    6.0,
                    RED,
                    if hov_close { 0.22 } else { 0.0 },
                );
                g.text(
                    G_CLOSE,
                    &g.f.icon_sm,
                    card_x + card_w - 25.0,
                    oy + 9.0,
                    20.0,
                    24.0,
                    if hov_close { RED } else { GRAY },
                    0.9,
                );
                self.hit(card_x + card_w - 25.0, oy + 7.0, 22.0, 28.0, Hit::OpenWinClose(idx));
                oy += 46.0;
            }
        }

        // Tarjeta Inferior: SERVIDORES EN VIVO Y ACCESOS RÁPIDOS (completa exactamente la altura hasta `left_bottom_y`)
        let bot_card_y = list_top_y + top_card_h + 8.0;
        let bot_card_h = (left_bottom_y - bot_card_y).max(90.0);

        g.rrect(right_x, bot_card_y, panel_w, bot_card_h, 14.0, CARD, 0.65);
        g.rrect_stroke(
            right_x + 0.5,
            bot_card_y + 0.5,
            panel_w - 1.0,
            bot_card_h - 1.0,
            14.0,
            BORDER,
            0.7,
            1.0,
        );

        let has_srvs = !self.servers.is_empty();
        g.circle(
            right_x + 14.0,
            bot_card_y + 16.0,
            3.5,
            if has_srvs { GREEN } else { CYAN },
            1.0,
        );
        let bot_hdr = if has_srvs {
            format!("SERVIDORES EN VIVO ({})", self.servers.len())
        } else {
            "ACCESOS RÁPIDOS".to_string()
        };
        g.text(
            &bot_hdr,
            &g.f.tiny,
            right_x + 24.0,
            bot_card_y + 4.0,
            panel_w - 30.0,
            24.0,
            GRAY,
            1.0,
        );

        let mut by = bot_card_y + 30.0;
        let bot_limit_y = bot_card_y + bot_card_h - 8.0;

        // Mostrar servidores activos en la tarjeta inferior derecha
        for srv in self.servers.values().take(3) {
            if by + 38.0 > bot_limit_y {
                break;
            }
            let sx = right_x + 7.0;
            let sw = panel_w - 14.0;
            let sh = 36.0;
            g.rrect(sx, by, sw, sh, 9.0, BODY, 0.85);
            g.rrect_stroke(sx + 0.5, by + 0.5, sw - 1.0, sh - 1.0, 9.0, GREEN, 0.35, 1.0);
            g.circle(sx + 10.0, by + 18.0, 3.2, GREEN, 1.0);
            let title = if srv.label == "Main" {
                srv.project_name.clone()
            } else {
                format!("{} · {}", srv.project_name, srv.label)
            };
            g.text(&title, &g.f.badge, sx + 18.0, by + 2.0, sw - 74.0, 17.0, WHITE, 1.0);
            let port_str = srv
                .port
                .map(|pt| format!("localhost:{pt}"))
                .unwrap_or_else(|| "Iniciando…".into());
            g.text(&port_str, &g.f.tiny, sx + 18.0, by + 18.0, sw - 74.0, 15.0, GREEN, 0.95);
            if let Some(pt) = srv.port {
                self.hit(sx, by, sw - 48.0, sh, Hit::PortBadge(pt));
                self.mini_action_btn(sx + sw - 46.0, by + 6.0, 20.0, G_TERM, GREEN, false, Hit::PortTerm(pt));
                self.mini_action_btn(sx + sw - 23.0, by + 6.0, 20.0, G_STOP, RED, false, Hit::KillPort(pt));
            }
            by += 40.0;
        }

        // Acciones rápidas útiles para completar la tarjeta inferior derecha
        let mut quick_actions: Vec<(&str, String, String, u32, Hit)> = Vec::new();
        if let Some((repo_name, _)) = &self.clipboard_repo {
            quick_actions.push((
                G_DOWNLOAD,
                format!("Clonar {repo_name}"),
                "URL de GitHub en portapapeles".into(),
                GREEN,
                Hit::CloneClipboardRepo,
            ));
        }
        quick_actions.push((
            G_FOLDER,
            "Abrir carpeta raíz".into(),
            actions::short(&self.cfg.base_dir),
            WHITE,
            Hit::QuickOpenBaseDir,
        ));
        quick_actions.push((
            G_EDIT,
            "Librerías y Programas".into(),
            "Agregar IDEs, stacks o paquetes".into(),
            CYAN,
            Hit::OpenCustomEditor,
        ));
        quick_actions.push((
            G_SYNC,
            "Sincronizar carpetas".into(),
            "Actualizar proyectos del disco".into(),
            GRAY,
            Hit::QuickSyncProjects,
        ));

        for (icon, title, desc, col, hit_id) in quick_actions {
            if by + 38.0 > bot_limit_y {
                break;
            }
            let qx = right_x + 7.0;
            let qw = panel_w - 14.0;
            let qh = 38.0;
            let hov = self.hov(hit_id);
            g.rrect(
                qx,
                by,
                qw,
                qh,
                9.0,
                if hov { CARD_HOV } else { BODY },
                if hov { 0.95 } else { 0.72 },
            );
            g.rrect_stroke(
                qx + 0.5,
                by + 0.5,
                qw - 1.0,
                qh - 1.0,
                9.0,
                if hov { col } else { BORDER },
                if hov { 0.55 } else { 0.6 },
                1.0,
            );
            g.text(icon, &g.f.icon_sm, qx + 6.0, by, 24.0, qh, col, 0.95);
            g.text(&title, &g.f.badge, qx + 32.0, by + 3.0, qw - 38.0, 17.0, WHITE, 1.0);
            g.text(&desc, &g.f.tiny, qx + 32.0, by + 19.0, qw - 38.0, 15.0, DIM, 0.95);
            self.hit(qx, by, qw, qh, hit_id);
            by += 42.0;
        }
    }

    pub(crate) fn header(&self, lx: f32, y: f32, tw: f32, title: &str, sub: &str, back: bool) {
        let g = &self.gfx;
        if back {
            self.icon_btn(lx + 16.0, y + 15.0, G_BACK, Hit::Back);
        } else {
            g.draw_logo(lx + 16.0, y + 14.0, 32.0, 32.0, 1.0);
        }
        g.text(title, &g.f.title, lx + 56.0, y + 13.0, tw - 140.0, 20.0, WHITE, 1.0);
        g.text(sub, &g.f.sub, lx + 56.0, y + 32.0, tw - 140.0, 16.0, GRAY, 1.0);
        if !back {
            self.icon_btn(lx + tw - 80.0, y + 15.0, G_GEAR, Hit::Settings);
        }
        self.icon_btn(lx + tw - 44.0, y + 15.0, G_CLOSE, Hit::Close);
        g.line(lx + 16.0, y + 56.0, lx + tw - 16.0, y + 56.0, BORDER, 0.8, 1.0);
    }

    pub(crate) fn icon_btn(&self, x: f32, y: f32, glyph: &str, id: Hit) {
        let g = &self.gfx;
        let hov = self.hov(id);
        g.rrect(x, y, 28.0, 28.0, 9.0, WHITE, if hov { 0.14 } else { 0.0 });
        g.text(glyph, &g.f.icon, x, y, 28.0, 28.0, if hov { WHITE } else { GRAY }, 1.0);
        self.hit(x, y, 28.0, 28.0, id);
    }

    pub(crate) fn pill_action_btn(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        color: u32,
        active: bool,
        id: Hit,
    ) {
        let g = &self.gfx;
        let hov = self.hov(id);
        g.rrect(
            x,
            y,
            w,
            h,
            8.0,
            color,
            if hov {
                0.24
            } else if active {
                0.14
            } else {
                0.06
            },
        );
        g.rrect_stroke(
            x + 0.5,
            y + 0.5,
            w - 1.0,
            h - 1.0,
            8.0,
            if active || hov { color } else { BORDER },
            if active || hov { 0.65 } else { 0.8 },
            1.0,
        );
        g.text(
            label,
            &g.f.badge,
            x,
            y,
            w,
            h,
            if active || hov { color } else { GRAY },
            1.0,
        );
        self.hit(x, y, w, h, id);
    }

    pub(crate) fn mini_action_btn(
        &self,
        x: f32,
        y: f32,
        s: f32,
        glyph: &str,
        color: u32,
        active: bool,
        id: Hit,
    ) {
        let g = &self.gfx;
        let hov = self.hov(id);
        g.rrect(
            x,
            y,
            s,
            s,
            8.0,
            color,
            if hov {
                0.22
            } else if active {
                0.15
            } else {
                0.05
            },
        );
        g.rrect_stroke(
            x + 0.5,
            y + 0.5,
            s - 1.0,
            s - 1.0,
            8.0,
            if active || hov { color } else { BORDER },
            if active || hov { 0.65 } else { 0.8 },
            1.0,
        );
        g.text(
            glyph,
            &g.f.icon_sm,
            x,
            y,
            s,
            s,
            if active || hov { color } else { GRAY },
            1.0,
        );
        self.hit(x, y, s, s, id);
    }

    pub(crate) fn row_bg(&self, x: f32, y: f32, w: f32, h: f32, id: Hit, sel: bool) {
        let g = &self.gfx;
        let hov = self.hov(id);
        g.rrect(x, y, w, h, 13.0, if hov || sel { CARD_HOV } else { CARD }, 1.0);
        g.rrect_stroke(
            x + 0.5,
            y + 0.5,
            w - 1.0,
            h - 1.0,
            13.0,
            if sel { WHITE } else { BORDER },
            if sel { 0.85 } else { 0.7 },
            1.0,
        );
        self.hit(x, y, w, h, id);
    }

    pub(crate) fn toggle_pill(&self, x: f32, y: f32, on: bool) {
        let g = &self.gfx;
        g.rrect(
            x,
            y,
            38.0,
            20.0,
            10.0,
            if on { WHITE } else { 0x27272A },
            if on { 0.95 } else { 1.0 },
        );
        let kx = if on { x + 20.0 } else { x + 2.0 };
        g.circle(kx + 8.0, y + 10.0, 7.0, if on { 0x000000 } else { GRAY }, 1.0);
    }

    pub(crate) fn draw_tooltip(&self, win_x: f32, win_y: f32, win_w: f32, win_h: f32) {
        let Some(hov) = self.hover else { return };
        let label: Option<String> = match hov {
            Hit::Settings => Some("Configuración general".into()),
            Hit::Close => Some("Minimizar a la gota (Esc)".into()),
            Hit::Back => Some("Volver atrás (Esc)".into()),
            Hit::ClearSearch => Some("Limpiar búsqueda".into()),
            Hit::FilterChip(FilterMode::All) => Some("Mostrar todos los proyectos".into()),
            Hit::FilterChip(FilterMode::Pinned) => Some("Filtrar solo proyectos fijados".into()),
            Hit::FilterChip(FilterMode::GitDirty) => Some("Filtrar proyectos con cambios sin subir a Git".into()),
            Hit::FilterChip(FilterMode::RunningServer) => Some("Filtrar proyectos con servidor activo".into()),
            Hit::FilterChip(FilterMode::FullStack) => Some("Filtrar proyectos Full-Stack (Front + Back)".into()),
            Hit::CloneClipboardRepo => Some("Clonar repositorio de GitHub copiado en el portapapeles".into()),
            Hit::QuickOpenBaseDir => Some("Abrir la carpeta raíz donde están tus proyectos".into()),
            Hit::QuickSyncProjects => Some("Sincronizar carpetas creadas o borradas en el disco".into()),
            Hit::PortBadge(p) | Hit::ProjPort(_, p) => Some(format!("Abrir http://localhost:{p} en el navegador")),
            Hit::PortTerm(p) => Some(format!("Ver terminal en vivo del puerto :{p}")),
            Hit::KillPort(p) => Some(format!("Detener servidor en el puerto :{p}")),
            Hit::ProjServer(i) => {
                let running = self
                    .cfg
                    .projects
                    .get(i)
                    .map(|p| self.servers.contains_key(&p.path))
                    .unwrap_or(false);
                if running {
                    Some("Detener servidor local".into())
                } else {
                    Some("Iniciar servidor de desarrollo".into())
                }
            }
            Hit::ProjServerRole(_, ServerRole::Front) => Some("Iniciar / Detener solo el Frontend".into()),
            Hit::ProjServerRole(_, ServerRole::Back) => Some("Iniciar / Detener solo el Backend".into()),
            Hit::ProjServerRole(_, ServerRole::Both) => Some("Combo Full-Stack: Iniciar / Detener Front y Back a la vez".into()),
            Hit::ProjServerTerm(_) => Some("Ver terminal del servidor en vivo".into()),
            Hit::ProjGit(_) => Some("Git y GitHub (Commit, Push, Pull)".into()),
            Hit::ProjNote(_) => Some("Nota / Recordatorio rápido del proyecto (Mini-TODO)".into()),
            Hit::ProjTerm(_) => Some("Abrir en Windows Terminal / PowerShell".into()),
            Hit::ProjExplorer(_) => Some("Abrir carpeta en el Explorador de archivos".into()),
            Hit::ProjPin(i) => {
                let pinned = self.cfg.projects.get(i).map(|p| p.pinned).unwrap_or(false);
                if pinned {
                    Some("Desfijar proyecto".into())
                } else {
                    Some("Fijar proyecto arriba".into())
                }
            }
            Hit::OpenWinFocus(_) => Some("Saltar a la ventana ya abierta de este proyecto".into()),
            Hit::OpenWinClose(_) => Some("Cerrar la ventana de este proyecto".into()),
            _ => None,
        };
        let Some(txt) = label else { return };

        let rect = self
            .hits
            .borrow()
            .iter()
            .rev()
            .find(|h| h.4 == hov)
            .copied();
        let Some((hx, hy, hw, _hh, _)) = rect else { return };

        let g = &self.gfx;
        let tw = g.measure(&txt, &g.f.tiny) + 18.0;
        let th = 22.0;
        let mut tx = hx + hw * 0.5 - tw * 0.5;
        tx = tx.clamp(win_x + 10.0, win_x + win_w - tw - 10.0);
        let ty = if hy > win_y + 34.0 {
            hy - th - 5.0
        } else {
            (hy + 32.0).min(win_y + win_h - th - 6.0)
        };

        g.rrect(tx - 1.0, ty + 1.5, tw + 2.0, th + 1.0, 6.0, 0x000000, 0.7);
        g.rrect(tx, ty, tw, th, 6.0, 0x18181B, 0.98);
        g.rrect_stroke(tx + 0.5, ty + 0.5, tw - 1.0, th - 1.0, 6.0, WHITE, 0.28, 1.0);
        g.text(&txt, &g.f.tiny_c, tx + 6.0, ty, tw - 12.0, th, WHITE, 0.98);
    }
}

