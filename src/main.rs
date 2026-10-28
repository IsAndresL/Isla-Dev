#![windows_subsystem = "windows"]

mod actions;
mod app;
mod config;
mod gfx;

use std::cell::RefCell;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use app::App;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // try_borrow_mut: durante menús/diálogos modales los mensajes anidados se delegan a DefWindowProc.
    let r = APP.with(|cell| match cell.try_borrow_mut() {
        Ok(mut a) => a.as_mut().and_then(|a| a.handle(msg, wp, lp)),
        Err(_) => None,
    });
    match r {
        Some(v) => LRESULT(v),
        None => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|info| {
        let path = config::app_dir().join("crash.log");
        let _ = std::fs::write(path, format!("{info}"));
    }));
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let _mutex = CreateMutexW(None, true, w!("IslaDev.SingleInstance"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return Ok(());
        }
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        let hinst = GetModuleHandleW(None)?;
        let class = w!("IslaDevWnd");
        let wc = WNDCLASSW {
            hInstance: hinst.into(),
            lpszClassName: class,
            lpfnWndProc: Some(wndproc),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_ACCEPTFILES,
            class,
            w!("Isla Dev"),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            10,
            10,
            None,
            None,
            hinst,
            None,
        )?;

        let app = App::new(hwnd)?;
        APP.with(|a| *a.borrow_mut() = Some(app));
        APP.with(|a| {
            if let Some(a) = a.borrow_mut().as_mut() {
                a.init()
            }
        });

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Ok(())
    }
}
