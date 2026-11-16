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

#[derive(Default)]
pub struct Overlays(pub Mutex<HashMap<String, OverlayInfo>>);

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
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, GWL_EXSTYLE, GWLP_HWNDPARENT, GetClientRect, GetCursorPos, GetWindowLongPtrW, GetWindowTextW,
        IsIconic, IsWindowVisible, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
    };
    use windows::core::{HSTRING, PCWSTR};

    use super::OverlayInfo;
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

    fn find_tables(classes: &[String]) -> Vec<Table> {
        let mut out = Vec::new();
        for class in classes {
            let class = HSTRING::from(class.as_str());
            let mut after: Option<HWND> = None;
            loop {
                let Ok(found) = (unsafe { FindWindowExW(None, after, &class, PCWSTR::null()) }) else { break };
                if found.0.is_null() {
                    break;
                }
                after = Some(found);
                if let Some(t) = describe(found) {
                    out.push(t);
                }
            }
        }
        out
    }

    fn describe(h: HWND) -> Option<Table> {
        unsafe {
            if !IsWindowVisible(h).as_bool() {
                return None;
            }
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(h, &mut buf);
            let title = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
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
            loop {
                let state = app.state::<AppState>();
                let classes = state.settings.lock().unwrap().table_window_classes.clone();
                let tables = find_tables(&classes);
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
                            let mut overlays = state.overlays.0.lock().unwrap();
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
                            let overlays = state.overlays.0.lock().unwrap();
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
                        state.overlays.0.lock().unwrap().remove(label);
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
