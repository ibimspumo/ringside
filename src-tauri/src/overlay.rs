//! One transparent overlay window per poker table window, glued to the table's client area.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Interactive area inside an overlay, in physical pixels relative to the overlay window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

#[derive(Debug, Clone, Default)]
pub struct OverlayInfo {
    pub title: String,
    pub table_key: Option<String>,
    pub hit_regions: Vec<Rect>,
}

/// A visible window of the poker client, as seen by the last scan.
#[derive(Debug, Clone, Serialize)]
pub struct ClientWindow {
    pub title: String,
    pub class: String,
    pub table_key: Option<String>,
}

#[derive(Default)]
pub struct Overlays {
    pub by_label: Mutex<HashMap<String, OverlayInfo>>,
    pub client_windows: Mutex<Vec<ClientWindow>>,
}

#[cfg(windows)]
pub use win::spawn;

#[cfg(not(windows))]
pub fn spawn(_app: tauri::AppHandle) {}

#[cfg(windows)]
mod win {
    use std::collections::HashMap;
    use std::time::Duration;

    use ringside_core::table::key_from_title;
    use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
    use windows::Win32::Foundation::{CloseHandle, HWND, POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, GWL_EXSTYLE, GWLP_HWNDPARENT, GetClassNameW, GetClientRect, GetCursorPos, GetWindowLongPtrW,
        GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SWP_NOACTIVATE, SWP_NOZORDER,
        SetWindowLongPtrW, SetWindowPos, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    use windows::core::{PCWSTR, PWSTR};

    use super::{ClientWindow, OverlayInfo};
    use crate::AppState;

    struct Table {
        hwnd: isize,
        title: String,
        rect: (i32, i32, i32, i32),
        minimized: bool,
    }

    struct Tracked {
        window: WebviewWindow,
        hwnd: isize,
        rect: (i32, i32, i32, i32),
        title: String,
        click_through: bool,
    }

    fn hwnd(v: isize) -> HWND {
        HWND(v as *mut _)
    }

    /// Re-scanning all top-level windows is slower than following known tables, so it runs
    /// less often than the position updates.
    const SCAN_EVERY: u32 = 16;

    fn is_poker_client(pid: u32) -> bool {
        unsafe {
            let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(process);
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            let exe = path.rsplit('\\').next().unwrap_or_default().to_lowercase();
            ok && exe.starts_with("pokerstars")
        }
    }

    fn text(buf: &[u16], len: i32) -> String {
        String::from_utf16_lossy(&buf[..len.clamp(0, buf.len() as i32) as usize])
    }

    /// Visible windows of the poker client, and the ones among them that are tables.
    fn scan() -> (Vec<isize>, Vec<ClientWindow>) {
        let mut clients: HashMap<u32, bool> = HashMap::new();
        let mut tables = Vec::new();
        let mut seen = Vec::new();
        let mut after: Option<HWND> = None;
        loop {
            let Ok(h) = (unsafe { FindWindowExW(None, after, PCWSTR::null(), PCWSTR::null()) }) else { break };
            if h.0.is_null() {
                break;
            }
            after = Some(h);
            if !unsafe { IsWindowVisible(h) }.as_bool() {
                continue;
            }
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
            if !*clients.entry(pid).or_insert_with(|| is_poker_client(pid)) {
                continue;
            }
            let mut buf = [0u16; 512];
            let title = text(&buf, unsafe { GetWindowTextW(h, &mut buf) });
            let mut cbuf = [0u16; 256];
            let class = text(&cbuf, unsafe { GetClassNameW(h, &mut cbuf) });
            let table_key = key_from_title(&title);
            if table_key.is_some() {
                tables.push(h.0 as isize);
            }
            seen.push(ClientWindow { title, class, table_key });
        }
        (tables, seen)
    }

    fn describe(h: HWND) -> Option<Table> {
        unsafe {
            if !IsWindowVisible(h).as_bool() {
                return None;
            }
            let mut buf = [0u16; 512];
            let title = text(&buf, GetWindowTextW(h, &mut buf));
            let mut rect = RECT::default();
            GetClientRect(h, &mut rect).ok()?;
            let mut origin = POINT { x: 0, y: 0 };
            let _ = ClientToScreen(h, &mut origin);
            Some(Table {
                hwnd: h.0 as isize,
                title,
                rect: (origin.x, origin.y, rect.right - rect.left, rect.bottom - rect.top),
                minimized: IsIconic(h).as_bool(),
            })
        }
    }

    fn create_overlay(app: &AppHandle, label: &str, table: &Table) -> Option<Tracked> {
        let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html?view=overlay".into()))
            .title("Ringside HUD")
            .transparent(true)
            .decorations(false)
            .shadow(false)
            .resizable(false)
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .build()
            .ok()?;
        let overlay = hwnd(window.hwnd().ok()?.0 as isize);
        unsafe {
            // Owned by the table: stays directly above it, hides with it, never floats over other apps.
            SetWindowLongPtrW(overlay, GWLP_HWNDPARENT, table.hwnd);
            let ex = GetWindowLongPtrW(overlay, GWL_EXSTYLE);
            SetWindowLongPtrW(overlay, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize);
        }
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
        Some(Tracked { window, hwnd: overlay.0 as isize, rect: (0, 0, 0, 0), title: String::new(), click_through: true })
    }

    pub fn spawn(app: AppHandle) {
        std::thread::spawn(move || {
            let mut tracked: HashMap<String, Tracked> = HashMap::new();
            let mut last_count = usize::MAX;
            let mut table_hwnds: Vec<isize> = Vec::new();
            let mut tick = 0u32;
            loop {
                let state = app.state::<AppState>();
                if tick % SCAN_EVERY == 0 {
                    let (found, seen) = scan();
                    table_hwnds = found;
                    *state.overlays.client_windows.lock().unwrap() = seen;
                }
                tick = tick.wrapping_add(1);
                let tables: Vec<Table> = table_hwnds.iter().filter_map(|&h| describe(hwnd(h))).collect();
                let mut cursor = POINT::default();
                let cursor_ok = unsafe { GetCursorPos(&mut cursor) }.is_ok();

                let mut alive = Vec::new();
                for table in &tables {
                    let label = format!("ov-{:x}", table.hwnd);
                    alive.push(label.clone());
                    if !tracked.contains_key(&label) {
                        match create_overlay(&app, &label, table) {
                            Some(t) => {
                                tracked.insert(label.clone(), t);
                            }
                            None => continue,
                        }
                    }
                    let t = tracked.get_mut(&label).expect("inserted above");

                    if t.title != table.title {
                        t.title = table.title.clone();
                        let key = key_from_title(&table.title);
                        {
                            let mut overlays = state.overlays.by_label.lock().unwrap();
                            let info = overlays.entry(label.clone()).or_insert_with(OverlayInfo::default);
                            info.title = table.title.clone();
                            info.table_key = key.clone();
                        }
                        let _ = app.emit_to(label.as_str(), "overlay-table", key);
                    }

                    if !table.minimized && t.rect != table.rect {
                        t.rect = table.rect;
                        let (x, y, w, h) = table.rect;
                        unsafe {
                            let _ = SetWindowPos(hwnd(t.hwnd), None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER);
                        }
                    }

                    let over_panel = cursor_ok && {
                        let (x, y, w, h) = t.rect;
                        let (cx, cy) = (cursor.x - x, cursor.y - y);
                        cx >= 0 && cy >= 0 && cx < w && cy < h && {
                            let overlays = state.overlays.by_label.lock().unwrap();
                            overlays
                                .get(&label)
                                .is_some_and(|o| o.hit_regions.iter().any(|r| r.contains(cx as f64, cy as f64)))
                        }
                    };
                    if over_panel == t.click_through {
                        t.click_through = !over_panel;
                        let _ = t.window.set_ignore_cursor_events(t.click_through);
                    }
                }

                tracked.retain(|label, t| {
                    let keep = alive.contains(label);
                    if !keep {
                        let _ = t.window.destroy();
                        state.overlays.by_label.lock().unwrap().remove(label);
                    }
                    keep
                });
                if tracked.len() != last_count {
                    last_count = tracked.len();
                    let _ = app.emit("tables-tracked", last_count);
                }

                std::thread::sleep(Duration::from_millis(30));
            }
        });
    }
}
