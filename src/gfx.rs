use std::cell::Cell;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub static LOGO_PNG: &[u8] = include_bytes!("../logo/logo.png");

#[allow(dead_code)]
pub struct Fonts {
    pub title: IDWriteTextFormat,
    pub title_c: IDWriteTextFormat,
    pub body: IDWriteTextFormat,
    pub sub: IDWriteTextFormat,
    pub sub_c: IDWriteTextFormat,
    pub sub_r: IDWriteTextFormat,
    pub badge: IDWriteTextFormat,
    pub tiny: IDWriteTextFormat,
    pub tiny_c: IDWriteTextFormat,
    pub small_c: IDWriteTextFormat,
    pub btn: IDWriteTextFormat,
    pub input: IDWriteTextFormat,
    pub icon: IDWriteTextFormat,
    pub icon_sm: IDWriteTextFormat,
}

pub struct Gfx {
    rt: ID2D1DCRenderTarget,
    factory: ID2D1Factory,
    dw: IDWriteFactory,
    brush: ID2D1SolidColorBrush,
    pub f: Fonts,
    _icon_family: HSTRING,
    memdc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    pub wpx: i32,
    pub hpx: i32,
    pub scale: f32,
    pub alpha: Cell<f32>,
    pub logo: Option<ID2D1Bitmap>,
}

fn rt_props() -> D2D1_RENDER_TARGET_PROPERTIES {
    D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 0.0,
        dpiY: 0.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    }
}

const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

fn fmt(
    dw: &IDWriteFactory,
    fam: &HSTRING,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    align: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat> {
    unsafe {
        let f = dw.CreateTextFormat(
            fam,
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("es-es"),
        )?;
        f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        f.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        f.SetTextAlignment(align)?;
        let sign = dw.CreateEllipsisTrimmingSign(&f)?;
        let trim = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        f.SetTrimming(&trim, &sign)?;
        Ok(f)
    }
}

fn family_exists(dw: &IDWriteFactory, name: &HSTRING) -> bool {
    unsafe {
        let mut coll = None;
        if dw.GetSystemFontCollection(&mut coll, false).is_err() {
            return false;
        }
        let Some(coll) = coll else { return false };
        let mut idx = 0u32;
        let mut exists = BOOL(0);
        coll.FindFamilyName(name, &mut idx, &mut exists).is_ok() && exists.as_bool()
    }
}

impl Fonts {
    fn new(dw: &IDWriteFactory, icon_family: &HSTRING) -> Result<Self> {
        let ui = if family_exists(dw, &HSTRING::from("Segoe UI Variable Text")) {
            HSTRING::from("Segoe UI Variable Text")
        } else {
            HSTRING::from("Segoe UI")
        };
        let l = DWRITE_TEXT_ALIGNMENT_LEADING;
        let c = DWRITE_TEXT_ALIGNMENT_CENTER;
        let r = DWRITE_TEXT_ALIGNMENT_TRAILING;
        Ok(Self {
            title: fmt(dw, &ui, 14.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, l)?,
            title_c: fmt(dw, &ui, 15.0, DWRITE_FONT_WEIGHT_BOLD, c)?,
            body: fmt(dw, &ui, 13.5, DWRITE_FONT_WEIGHT_NORMAL, l)?,
            sub: fmt(dw, &ui, 12.0, DWRITE_FONT_WEIGHT_NORMAL, l)?,
            sub_c: fmt(dw, &ui, 12.0, DWRITE_FONT_WEIGHT_NORMAL, c)?,
            sub_r: fmt(dw, &ui, 12.0, DWRITE_FONT_WEIGHT_NORMAL, r)?,
            badge: fmt(dw, &ui, 11.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, c)?,
            tiny: fmt(dw, &ui, 11.0, DWRITE_FONT_WEIGHT_MEDIUM, l)?,
            tiny_c: fmt(dw, &ui, 11.0, DWRITE_FONT_WEIGHT_MEDIUM, c)?,
            small_c: fmt(dw, &ui, 10.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, c)?,
            btn: fmt(dw, &ui, 13.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, c)?,
            input: fmt(dw, &ui, 15.0, DWRITE_FONT_WEIGHT_NORMAL, l)?,
            icon: fmt(dw, icon_family, 16.0, DWRITE_FONT_WEIGHT_NORMAL, c)?,
            icon_sm: fmt(dw, icon_family, 12.0, DWRITE_FONT_WEIGHT_NORMAL, c)?,
        })
    }
}

impl Gfx {
    pub fn new(scale: f32, w: f32, h: f32) -> Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let rt = factory.CreateDCRenderTarget(&rt_props())?;
            let dw: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let brush = rt.CreateSolidColorBrush(&CLEAR, None)?;
            let icon_family = if family_exists(&dw, &HSTRING::from("Segoe Fluent Icons")) {
                HSTRING::from("Segoe Fluent Icons")
            } else {
                HSTRING::from("Segoe MDL2 Assets")
            };
            let f = Fonts::new(&dw, &icon_family)?;
            let memdc = CreateCompatibleDC(None);
            let logo = Self::load_bitmap_from_bytes(&rt, LOGO_PNG);
            let mut g = Gfx {
                rt,
                factory,
                dw,
                brush,
                f,
                _icon_family: icon_family,
                memdc,
                bmp: HBITMAP::default(),
                old: HGDIOBJ::default(),
                wpx: 0,
                hpx: 0,
                scale,
                alpha: Cell::new(1.0),
                logo,
            };
            g.resize(scale, w, h)?;
            Ok(g)
        }
    }

    fn load_bitmap_from_bytes(rt: &ID2D1DCRenderTarget, bytes: &[u8]) -> Option<ID2D1Bitmap> {
        unsafe {
            let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
            let stream = wic.CreateStream().ok()?;
            stream.InitializeFromMemory(bytes).ok()?;
            let decoder = wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand).ok()?;
            let frame = decoder.GetFrame(0).ok()?;
            let converter = wic.CreateFormatConverter().ok()?;
            converter.Initialize(&frame, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom).ok()?;
            rt.CreateBitmapFromWicBitmap(&converter, None).ok()
        }
    }

    fn dib(dc: HDC, w: i32, h: i32) -> Result<HBITMAP> {
        unsafe {
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, None, 0)
        }
    }

    pub fn resize(&mut self, scale: f32, w: f32, h: f32) -> Result<()> {
        unsafe {
            self.scale = scale;
            self.wpx = (w * scale).ceil() as i32;
            self.hpx = (h * scale).ceil() as i32;
            if !self.bmp.is_invalid() {
                SelectObject(self.memdc, self.old);
                let _ = DeleteObject(self.bmp);
            }
            self.bmp = Self::dib(self.memdc, self.wpx, self.hpx)?;
            self.old = SelectObject(self.memdc, self.bmp);
            Ok(())
        }
    }

    pub fn begin(&self) -> Result<()> {
        unsafe {
            let rc = RECT { left: 0, top: 0, right: self.wpx, bottom: self.hpx };
            self.rt.BindDC(self.memdc, &rc)?;
            self.rt.SetDpi(96.0 * self.scale, 96.0 * self.scale);
            self.rt.BeginDraw();
            self.rt.Clear(Some(&CLEAR));
            Ok(())
        }
    }

    pub fn present(&mut self, hwnd: HWND, x: i32, y: i32) {
        unsafe {
            if let Err(e) = self.rt.EndDraw(None, None) {
                if e.code() == D2DERR_RECREATE_TARGET {
                    if let Ok(rt) = self.factory.CreateDCRenderTarget(&rt_props()) {
                        if let Ok(b) = rt.CreateSolidColorBrush(&CLEAR, None) {
                            self.rt = rt;
                            self.brush = b;
                        }
                    }
                }
                return;
            }
            let src = POINT { x: 0, y: 0 };
            let dst = POINT { x, y };
            let size = SIZE { cx: self.wpx, cy: self.hpx };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                hwnd,
                None,
                Some(&dst),
                Some(&size),
                self.memdc,
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
        }
    }

    fn col(&self, c: u32, a: f32) {
        let r = ((c >> 16) & 255) as f32 / 255.0;
        let g = ((c >> 8) & 255) as f32 / 255.0;
        let b = (c & 255) as f32 / 255.0;
        unsafe {
            self.brush.SetColor(&D2D1_COLOR_F { r, g, b, a: (a * self.alpha.get()).clamp(0.0, 1.0) });
        }
    }

    fn rr(x: f32, y: f32, w: f32, h: f32, r: f32) -> D2D1_ROUNDED_RECT {
        D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F { left: x, top: y, right: x + w, bottom: y + h },
            radiusX: r,
            radiusY: r,
        }
    }

    pub fn rrect(&self, x: f32, y: f32, w: f32, h: f32, r: f32, c: u32, a: f32) {
        self.col(c, a);
        unsafe { self.rt.FillRoundedRectangle(&Self::rr(x, y, w, h, r), &self.brush) }
    }

    pub fn rrect_stroke(&self, x: f32, y: f32, w: f32, h: f32, r: f32, c: u32, a: f32, sw: f32) {
        self.col(c, a);
        unsafe { self.rt.DrawRoundedRectangle(&Self::rr(x, y, w, h, r), &self.brush, sw, None) }
    }

    pub fn waterdrop_path(&self, cx: f32, w: f32, h: f32) -> Result<ID2D1PathGeometry> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            let p0 = D2D_POINT_2F { x: cx - w * 0.5, y: 0.0 };
            sink.BeginFigure(p0, D2D1_FIGURE_BEGIN_FILLED);

            let r_cup = (h * 0.46).clamp(8.0, 15.0);
            let cy = (h - r_cup).max(4.0);
            let fillet_x = (w * 0.5 - r_cup) * 0.55;
            let fillet_y = cy * 0.55;

            // S-curve izquierda: transición suave desde el marco superior hacia la copa
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: cx - w * 0.5 + fillet_x, y: 0.0 },
                point2: D2D_POINT_2F { x: cx - r_cup, y: cy - fillet_y },
                point3: D2D_POINT_2F { x: cx - r_cup, y: cy },
            });
            // Fondo redondeado de la gota (copa semicircular que alberga la cámara)
            let k = r_cup * 0.5522847;
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: cx - r_cup, y: cy + k },
                point2: D2D_POINT_2F { x: cx - k, y: cy + r_cup },
                point3: D2D_POINT_2F { x: cx, y: cy + r_cup },
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: cx + k, y: cy + r_cup },
                point2: D2D_POINT_2F { x: cx + r_cup, y: cy + k },
                point3: D2D_POINT_2F { x: cx + r_cup, y: cy },
            });
            // S-curve derecha: asciende y se abre suavemente al marco superior
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: cx + r_cup, y: cy - fillet_y },
                point2: D2D_POINT_2F { x: cx + w * 0.5 - fillet_x, y: 0.0 },
                point3: D2D_POINT_2F { x: cx + w * 0.5, y: 0.0 },
            });
            sink.AddLine(p0);
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            Ok(path)
        }
    }

    /// Traza solo el contorno de la gota hacia abajo (sin trazar la línea horizontal superior y=0)
    pub fn waterdrop_curve_path(&self, cx: f32, w: f32, h: f32) -> Result<ID2D1PathGeometry> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            let p0 = D2D_POINT_2F { x: cx - w * 0.5, y: 0.0 };
            sink.BeginFigure(p0, D2D1_FIGURE_BEGIN_HOLLOW);

            let r_cup = (h * 0.46).clamp(8.0, 15.0);
            let cy = (h - r_cup).max(4.0);
            let fillet_x = (w * 0.5 - r_cup) * 0.55;
            let fillet_y = cy * 0.55;

            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: cx - w * 0.5 + fillet_x, y: 0.0 },
                point2: D2D_POINT_2F { x: cx - r_cup, y: cy - fillet_y },
                point3: D2D_POINT_2F { x: cx - r_cup, y: cy },
            });
            let k = r_cup * 0.5522847;
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: cx - r_cup, y: cy + k },
                point2: D2D_POINT_2F { x: cx - k, y: cy + r_cup },
                point3: D2D_POINT_2F { x: cx, y: cy + r_cup },
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: cx + k, y: cy + r_cup },
                point2: D2D_POINT_2F { x: cx + r_cup, y: cy + k },
                point3: D2D_POINT_2F { x: cx + r_cup, y: cy },
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: cx + r_cup, y: cy - fillet_y },
                point2: D2D_POINT_2F { x: cx + w * 0.5 - fillet_x, y: 0.0 },
                point3: D2D_POINT_2F { x: cx + w * 0.5, y: 0.0 },
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            Ok(path)
        }
    }

    pub fn top_panel_path(&self, x: f32, w: f32, h: f32, r: f32) -> Result<ID2D1PathGeometry> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            let r = r.min(w / 2.0).min(h);
            sink.BeginFigure(D2D_POINT_2F { x, y: 0.0 }, D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(D2D_POINT_2F { x, y: (h - r).max(0.0) });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x, y: h - r * 0.45 },
                point2: D2D_POINT_2F { x: x + r * 0.45, y: h },
                point3: D2D_POINT_2F { x: x + r, y: h },
            });
            sink.AddLine(D2D_POINT_2F { x: x + w - r, y: h });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: x + w - r * 0.45, y: h },
                point2: D2D_POINT_2F { x: x + w, y: h - r * 0.45 },
                point3: D2D_POINT_2F { x: x + w, y: (h - r).max(0.0) },
            });
            sink.AddLine(D2D_POINT_2F { x: x + w, y: 0.0 });
            sink.AddLine(D2D_POINT_2F { x, y: 0.0 });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            Ok(path)
        }
    }

    /// Contorno del panel desplegado (solo laterales y esquinas inferiores, sin línea en y=0)
    pub fn top_panel_curve_path(&self, x: f32, w: f32, h: f32, r: f32) -> Result<ID2D1PathGeometry> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            let r = r.min(w / 2.0).min(h);
            sink.BeginFigure(D2D_POINT_2F { x, y: 0.0 }, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddLine(D2D_POINT_2F { x, y: (h - r).max(0.0) });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x, y: h - r * 0.45 },
                point2: D2D_POINT_2F { x: x + r * 0.45, y: h },
                point3: D2D_POINT_2F { x: x + r, y: h },
            });
            sink.AddLine(D2D_POINT_2F { x: x + w - r, y: h });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F { x: x + w - r * 0.45, y: h },
                point2: D2D_POINT_2F { x: x + w, y: h - r * 0.45 },
                point3: D2D_POINT_2F { x: x + w, y: (h - r).max(0.0) },
            });
            sink.AddLine(D2D_POINT_2F { x: x + w, y: 0.0 });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            Ok(path)
        }
    }

    pub fn fill_path(&self, path: &ID2D1PathGeometry, c: u32, a: f32) {
        self.col(c, a);
        unsafe { self.rt.FillGeometry(path, &self.brush, None); }
    }

    pub fn stroke_path(&self, path: &ID2D1PathGeometry, c: u32, a: f32, sw: f32) {
        self.col(c, a);
        unsafe { self.rt.DrawGeometry(path, &self.brush, sw, None); }
    }

    pub fn circle(&self, cx: f32, cy: f32, r: f32, c: u32, a: f32) {
        self.col(c, a);
        let e = D2D1_ELLIPSE { point: D2D_POINT_2F { x: cx, y: cy }, radiusX: r, radiusY: r };
        unsafe { self.rt.FillEllipse(&e, &self.brush) }
    }

    pub fn ring(&self, cx: f32, cy: f32, r: f32, c: u32, a: f32, sw: f32) {
        self.col(c, a);
        let e = D2D1_ELLIPSE { point: D2D_POINT_2F { x: cx, y: cy }, radiusX: r, radiusY: r };
        unsafe { self.rt.DrawEllipse(&e, &self.brush, sw, None) }
    }

    pub fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, c: u32, a: f32, sw: f32) {
        self.col(c, a);
        unsafe {
            self.rt.DrawLine(D2D_POINT_2F { x: x1, y: y1 }, D2D_POINT_2F { x: x2, y: y2 }, &self.brush, sw, None)
        }
    }

    pub fn text(&self, s: &str, f: &IDWriteTextFormat, x: f32, y: f32, w: f32, h: f32, c: u32, a: f32) {
        self.col(c, a);
        let t: Vec<u16> = s.encode_utf16().collect();
        let rc = D2D_RECT_F { left: x, top: y, right: x + w, bottom: y + h };
        unsafe {
            self.rt.DrawText(&t, f, &rc, &self.brush, D2D1_DRAW_TEXT_OPTIONS_CLIP, DWRITE_MEASURING_MODE_NATURAL)
        }
    }

    pub fn measure(&self, s: &str, f: &IDWriteTextFormat) -> f32 {
        let t: Vec<u16> = s.encode_utf16().collect();
        unsafe {
            if let Ok(l) = self.dw.CreateTextLayout(&t, f, 10000.0, 100.0) {
                let mut m = DWRITE_TEXT_METRICS::default();
                if l.GetMetrics(&mut m).is_ok() {
                    return m.widthIncludingTrailingWhitespace;
                }
            }
        }
        0.0
    }

    pub fn clip(&self, x: f32, y: f32, w: f32, h: f32) {
        let rc = D2D_RECT_F { left: x, top: y, right: x + w, bottom: y + h };
        unsafe { self.rt.PushAxisAlignedClip(&rc, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) }
    }

    pub fn unclip(&self) {
        unsafe { self.rt.PopAxisAlignedClip() }
    }

    pub fn draw_logo(&self, x: f32, y: f32, w: f32, h: f32, a: f32) {
        if let Some(bmp) = &self.logo {
            let dst = D2D_RECT_F { left: x, top: y, right: x + w, bottom: y + h };
            unsafe {
                self.rt.DrawBitmap(
                    bmp,
                    Some(&dst),
                    (a * self.alpha.get()).clamp(0.0, 1.0),
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
        } else {
            self.rrect(x, y, w, h, 6.0, 0xFFFFFF, 0.2 * a);
        }
    }

    /// Animación del aro de luz exterior que rodea el notch en hover
    pub fn draw_glow_ring(&self, cx: f32, w: f32, h: f32, pulse: f32) {
        if pulse <= 0.005 {
            return;
        }
        if let Ok(curve) = self.waterdrop_curve_path(cx, w, h) {
            // Halo difuso exterior amplio
            self.stroke_path(&curve, 0xFFFFFF, 0.12 * pulse, 6.0);
            // Halo intermedio
            self.stroke_path(&curve, 0xFFFFFF, 0.28 * pulse, 3.2);
            // Aro luminoso nítido
            self.stroke_path(&curve, 0xFFFFFF, 0.90 * pulse, 1.4);
        }
    }

    /// Barra de progreso luminosa que recorre el borde inferior curvo de la gota mientras una tarea corre en 2do plano.
    pub fn draw_waterdrop_progress(&self, cx: f32, w: f32, h: f32, frac: f32, color: u32) {
        let f = frac.clamp(0.04, 1.0);
        if let Ok(curve) = self.waterdrop_curve_path(cx, w, h) {
            self.stroke_path(&curve, 0xFFFFFF, 0.14, 1.8);
            let left = cx - w * 0.5 - 4.0;
            let fill_w = (w + 8.0) * f;
            self.clip(left, 0.0, fill_w, h + 10.0);
            self.stroke_path(&curve, color, 0.30, 5.0);
            self.stroke_path(&curve, color, 0.98, 2.2);
            self.unclip();
        }
    }

    /// Dibuja el icono vectorial oficial de rama Git (2 nodos principales + 1 nodo bifurcado con curva suave).
    pub fn draw_git_branch_icon(&self, x: f32, y: f32, s: f32, color: u32, a: f32) {
        let cx = x + s * 0.5;
        let cy = y + s * 0.5;
        let dx = s * 0.19;
        let dy = s * 0.25;
        let r = (s * 0.085).clamp(1.5, 2.4);
        let sw = (s * 0.075).clamp(1.2, 1.7);

        let x_left = cx - dx;
        let y_top = cy - dy;
        let y_bot = cy + dy;
        let x_right = cx + dx;
        let y_branch = cy - dy * 0.65;

        // Tronco vertical izquierdo
        self.line(x_left, y_top + r, x_left, y_bot - r, color, a, sw);

        // Curva en S desde el tronco hacia el nodo superior derecho
        unsafe {
            if let Ok(path) = self.factory.CreatePathGeometry() {
                if let Ok(sink) = path.Open() {
                    sink.BeginFigure(
                        D2D_POINT_2F {
                            x: x_left,
                            y: y_bot - r * 1.2,
                        },
                        D2D1_FIGURE_BEGIN_HOLLOW,
                    );
                    sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                        point1: D2D_POINT_2F {
                            x: x_left,
                            y: cy,
                        },
                        point2: D2D_POINT_2F {
                            x: x_right,
                            y: cy + dy * 0.15,
                        },
                        point3: D2D_POINT_2F {
                            x: x_right,
                            y: y_branch + r,
                        },
                    });
                    sink.EndFigure(D2D1_FIGURE_END_OPEN);
                    let _ = sink.Close();
                    self.stroke_path(&path, color, a, sw);
                }
            }
        }

        // Los 3 nodos circulares de la rama
        self.ring(x_left, y_top, r, color, a, sw);
        self.ring(x_left, y_bot, r, color, a, sw);
        self.ring(x_right, y_branch, r, color, a, sw);
    }

    /// Dibuja la insignia con el icono vectorial oficial de la tecnología en estilo minimalista
    pub fn draw_stack_badge(&self, stack: &str, x: f32, y: f32, s: f32) {
        let r = s * 0.28;
        self.rrect(x, y, s, s, r, 0x141416, 1.0);
        self.rrect_stroke(x, y, s, s, r, 0x27272A, 1.0, 1.0);
        self.draw_stack_icon_only(stack, x, y, s);
    }

    /// Dibuja únicamente el glifo vectorial de la tecnología sin caja exterior (útil en la píldora compacta y badges).
    pub fn draw_stack_icon_only(&self, stack: &str, x: f32, y: f32, s: f32) {
        let cx = x + s * 0.5;
        let cy = y + s * 0.5;
        let st = stack.to_lowercase();

        if st.contains("react") {
            // React: núcleo y órbitas atómicas
            let orbit_w = s * 0.38;
            let orbit_h = s * 0.14;
            self.circle(cx, cy, s * 0.08, 0xFFFFFF, 1.0);
            let e1 = D2D1_ELLIPSE { point: D2D_POINT_2F { x: cx, y: cy }, radiusX: orbit_w, radiusY: orbit_h };
            unsafe {
                self.rt.DrawEllipse(&e1, &self.brush, 1.2, None);
                let mut m_orig = windows::Foundation::Numerics::Matrix3x2::default();
                self.rt.GetTransform(&mut m_orig);
                let m60 = windows::Foundation::Numerics::Matrix3x2::rotation(60.0, cx, cy);
                self.rt.SetTransform(&(m_orig * m60));
                self.rt.DrawEllipse(&e1, &self.brush, 1.2, None);
                let mm60 = windows::Foundation::Numerics::Matrix3x2::rotation(-60.0, cx, cy);
                self.rt.SetTransform(&(m_orig * mm60));
                self.rt.DrawEllipse(&e1, &self.brush, 1.2, None);
                self.rt.SetTransform(&m_orig);
            }
        } else if st.contains("vite") {
            let hw = s * 0.32;
            let hh = s * 0.32;
            self.line(cx - hw, cy - hh, cx, cy + hh, 0xFFFFFF, 0.90, 1.8);
            self.line(cx + hw, cy - hh, cx, cy + hh, 0xFFFFFF, 0.90, 1.8);
            self.line(cx - 2.0, cy - hh * 0.8, cx + 3.0, cy - 1.0, 0xFFFFFF, 1.0, 1.8);
            self.line(cx + 3.0, cy - 1.0, cx - 1.0, cy + 1.0, 0xFFFFFF, 1.0, 1.8);
            self.line(cx - 1.0, cy + 1.0, cx + 2.0, cy + hh * 0.8, 0xFFFFFF, 1.0, 1.8);
        } else if st.contains("vue") {
            let w_out = s * 0.36;
            let h_out = s * 0.32;
            let w_in = s * 0.22;
            let h_in = s * 0.18;
            self.line(cx - w_out, cy - h_out, cx, cy + h_out, 0xFFFFFF, 0.95, 2.0);
            self.line(cx + w_out, cy - h_out, cx, cy + h_out, 0xFFFFFF, 0.95, 2.0);
            self.line(cx - w_in, cy - h_out, cx, cy - h_out + h_in, 0xFFFFFF, 0.50, 1.6);
            self.line(cx + w_in, cy - h_out, cx, cy - h_out + h_in, 0xFFFFFF, 0.50, 1.6);
        } else if st.contains("astro") {
            let r_top = s * 0.32;
            self.line(cx, cy - r_top, cx - s * 0.22, cy + r_top * 0.6, 0xFFFFFF, 0.95, 2.0);
            self.line(cx, cy - r_top, cx + s * 0.22, cy + r_top * 0.6, 0xFFFFFF, 0.95, 2.0);
            self.line(cx - s * 0.14, cy + r_top * 0.1, cx + s * 0.14, cy + r_top * 0.1, 0xFFFFFF, 0.70, 1.5);
            self.circle(cx, cy + r_top * 0.8, 2.0, 0xFFFFFF, 0.9);
        } else if st.contains("next") {
            let nh = s * 0.30;
            let nw = s * 0.26;
            self.line(cx - nw, cy - nh, cx - nw, cy + nh, 0xFFFFFF, 0.95, 2.0);
            self.line(cx - nw, cy - nh, cx + nw, cy + nh, 0xFFFFFF, 0.95, 2.0);
            self.line(cx + nw, cy - nh, cx + nw, cy + nh * 0.3, 0xFFFFFF, 0.95, 2.0);
        } else if st.contains("node") {
            let hr = s * 0.30;
            for i in 0..6 {
                let a1 = i as f32 * std::f32::consts::TAU / 6.0;
                let a2 = (i + 1) as f32 * std::f32::consts::TAU / 6.0;
                self.line(cx + a1.cos() * hr, cy + a1.sin() * hr, cx + a2.cos() * hr, cy + a2.sin() * hr, 0xFFFFFF, 0.90, 1.5);
            }
            self.circle(cx, cy, 2.5, 0xFFFFFF, 0.9);
        } else if st.contains("python") {
            let pr = s * 0.22;
            self.rrect(cx - pr, cy - pr, pr * 1.5, pr * 0.9, 3.0, 0xFFFFFF, 0.9);
            self.circle(cx - pr * 0.4, cy - pr * 0.55, 1.5, 0x000000, 1.0);
            self.rrect(cx - pr * 0.5, cy + pr * 0.1, pr * 1.5, pr * 0.9, 3.0, 0xFFFFFF, 0.6);
            self.circle(cx + pr * 0.4, cy + pr * 0.55, 1.5, 0x000000, 1.0);
        } else if st.contains("rust") {
            let rr = s * 0.28;
            self.ring(cx, cy, rr, 0xFFFFFF, 0.9, 1.6);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                self.line(cx + a.cos() * rr, cy + a.sin() * rr, cx + a.cos() * (rr + 2.5), cy + a.sin() * (rr + 2.5), 0xFFFFFF, 0.9, 1.8);
            }
            self.circle(cx, cy, 2.0, 0xFFFFFF, 1.0);
        } else if st.contains("angular") {
            // Angular: escudo con A central
            let sw = s * 0.32;
            let sh = s * 0.34;
            self.line(cx, cy - sh, cx - sw, cy - sh * 0.4, 0xFFFFFF, 0.9, 1.5);
            self.line(cx, cy - sh, cx + sw, cy - sh * 0.4, 0xFFFFFF, 0.9, 1.5);
            self.line(cx - sw, cy - sh * 0.4, cx - sw * 0.7, cy + sh * 0.5, 0xFFFFFF, 0.9, 1.5);
            self.line(cx + sw, cy - sh * 0.4, cx + sw * 0.7, cy + sh * 0.5, 0xFFFFFF, 0.9, 1.5);
            self.line(cx - sw * 0.7, cy + sh * 0.5, cx, cy + sh, 0xFFFFFF, 0.9, 1.5);
            self.line(cx + sw * 0.7, cy + sh * 0.5, cx, cy + sh, 0xFFFFFF, 0.9, 1.5);
            self.line(cx, cy - sh * 0.45, cx - sw * 0.45, cy + sh * 0.35, 0xFFFFFF, 1.0, 1.7);
            self.line(cx, cy - sh * 0.45, cx + sw * 0.45, cy + sh * 0.35, 0xFFFFFF, 1.0, 1.7);
            self.line(cx - sw * 0.25, cy + sh * 0.05, cx + sw * 0.25, cy + sh * 0.05, 0xFFFFFF, 1.0, 1.5);
        } else if st.contains("svelte") {
            // Svelte: doble S estilizada
            let sw = s * 0.24;
            let sh = s * 0.28;
            self.rrect_stroke(cx - sw, cy - sh, sw * 2.0, sh * 1.1, 4.5, 0xFFFFFF, 0.95, 1.7);
            self.rrect_stroke(cx - sw, cy - sh * 0.1, sw * 2.0, sh * 1.1, 4.5, 0xFFFFFF, 0.75, 1.7);
        } else if st.contains("typescript") {
            let bw = s * 0.56;
            self.rrect_stroke(cx - bw * 0.5, cy - bw * 0.5, bw, bw, 4.0, 0xFFFFFF, 0.9, 1.4);
            self.text("TS", &self.f.small_c, x, y, s, s, 0xFFFFFF, 0.95);
        } else {
            let fw = s * 0.36;
            let fh = s * 0.26;
            self.rrect(cx - fw * 0.5, cy - fh * 0.3, fw, fh, 3.0, 0xFFFFFF, 0.15);
            self.rrect_stroke(cx - fw * 0.5, cy - fh * 0.3, fw, fh, 3.0, 0xFFFFFF, 0.85, 1.4);
            self.line(cx - fw * 0.5 + 2.0, cy - fh * 0.3 - 2.5, cx - fw * 0.1, cy - fh * 0.3 - 2.5, 0xFFFFFF, 0.85, 1.4);
        }
    }

    /// Crea el icono oficial de la app desde el PNG para la bandeja del sistema.
    pub fn make_app_icon(&self, size: i32) -> Result<HICON> {
        unsafe {
            if let Ok(wic) = CoCreateInstance::<_, IWICImagingFactory>(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) {
                if let Ok(stream) = wic.CreateStream() {
                    if stream.InitializeFromMemory(LOGO_PNG).is_ok() {
                        if let Ok(decoder) = wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand) {
                            if let Ok(frame) = decoder.GetFrame(0) {
                                if let Ok(converter) = wic.CreateFormatConverter() {
                                    if converter.Initialize(&frame, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom).is_ok() {
                                        if let Ok(scaler) = wic.CreateBitmapScaler() {
                                            if scaler.Initialize(&converter, size as u32, size as u32, WICBitmapInterpolationModeHighQualityCubic).is_ok() {
                                                let dc = CreateCompatibleDC(None);
                                                let bi = BITMAPINFO {
                                                    bmiHeader: BITMAPINFOHEADER {
                                                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                                                        biWidth: size,
                                                        biHeight: -size,
                                                        biPlanes: 1,
                                                        biBitCount: 32,
                                                        biCompression: BI_RGB.0,
                                                        ..Default::default()
                                                    },
                                                    ..Default::default()
                                                };
                                                let mut bits: *mut u32 = std::ptr::null_mut();
                                                let bmp = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits as *mut _ as *mut _, None, 0)?;
                                                if !bits.is_null() {
                                                    let stride = (size * 4) as u32;
                                                    let buf_size = stride * size as u32;
                                                    let slice = std::slice::from_raw_parts_mut(bits as *mut u8, buf_size as usize);
                                                    if scaler.CopyPixels(std::ptr::null(), stride, slice).is_ok() {
                                                        let mask = CreateBitmap(size, size, 1, 1, None);
                                                        let ii = ICONINFO { fIcon: TRUE, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: bmp };
                                                        let icon = CreateIconIndirect(&ii);
                                                        let _ = DeleteObject(mask);
                                                        let _ = DeleteObject(bmp);
                                                        let _ = DeleteDC(dc);
                                                        if let Ok(ic) = icon {
                                                            return Ok(ic);
                                                        }
                                                    }
                                                    let _ = DeleteObject(bmp);
                                                }
                                                let _ = DeleteDC(dc);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            self.make_icon(size, 0xFFFFFF)
        }
    }

    /// Dibuja el icono de la app (para la bandeja del sistema).
    pub fn make_icon(&self, size: i32, accent: u32) -> Result<HICON> {
        unsafe {
            let dc = CreateCompatibleDC(None);
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size,
                    biHeight: -size,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut u32 = std::ptr::null_mut();
            let bmp = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits as *mut _ as *mut _, None, 0)?;
            if bits.is_null() {
                let _ = DeleteDC(dc);
                return LoadIconW(None, IDI_APPLICATION);
            }
            let r = ((accent >> 16) & 255) as u8;
            let g = ((accent >> 8) & 255) as u8;
            let b = (accent & 255) as u8;
            let radius = size as f32 / 2.0;
            let slice = std::slice::from_raw_parts_mut(bits, (size * size) as usize);
            for y in 0..size {
                for x in 0..size {
                    let dx = x as f32 + 0.5 - radius;
                    let dy = y as f32 + 0.5 - radius;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist <= radius - 1.0 {
                        slice[(y * size + x) as usize] = (255u32 << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                    } else if dist <= radius {
                        let a = (radius - dist).clamp(0.0, 1.0);
                        let alpha = (a * 255.0) as u32;
                        let pr = (r as f32 * a) as u32;
                        let pg = (g as f32 * a) as u32;
                        let pb = (b as f32 * a) as u32;
                        slice[(y * size + x) as usize] = (alpha << 24) | (pr << 16) | (pg << 8) | pb;
                    } else {
                        slice[(y * size + x) as usize] = 0;
                    }
                }
            }
            let mask = CreateBitmap(size, size, 1, 1, None);
            let ii = ICONINFO { fIcon: TRUE, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: bmp };
            let icon = CreateIconIndirect(&ii);
            let _ = DeleteObject(mask);
            let _ = DeleteObject(bmp);
            let _ = DeleteDC(dc);
            icon
        }
    }
}
