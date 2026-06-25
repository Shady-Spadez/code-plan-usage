use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};
    use tray_icon::{
        menu::{Menu, MenuEvent, MenuItem},
        Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsWindowVisible, PostMessageW, ShowWindow, SW_HIDE, SW_SHOW, WM_USER,
    };
    use windows::Win32::Graphics::Gdi::InvalidateRect;
    use windows::Win32::Foundation::{WPARAM, LPARAM};

    use crate::debug_log;

    pub enum TrayCommand {
        Refresh,
    }

    /// Thread-safe queue for tray commands pushed by event handlers.
    static EVENT_QUEUE: std::sync::LazyLock<Mutex<VecDeque<TrayCommand>>> =
        std::sync::LazyLock::new(|| Mutex::new(VecDeque::new()));

    // Stores the tray icon so it can be explicitly dropped to remove it
    // from the system tray before the process exits.
    thread_local! {
        static TRAY_ICON: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
    }

    /// Flag set by the Settings menu callback; checked in the eframe update loop.
    pub static SETTINGS_REQUESTED: AtomicBool = AtomicBool::new(false);

    /// Window title set by `init_tray`. Used to find the widget window.
    static WIDGET_TITLE: OnceLock<String> = OnceLock::new();

    fn push_command(cmd: TrayCommand) {
        if let Ok(mut queue) = EVENT_QUEUE.lock() {
            queue.push_back(cmd);
        }
    }

    /// Find the main widget window by title and toggle its visibility.
    fn toggle_window_visibility() {
        let title = WIDGET_TITLE.get().map(|s| s.as_str()).unwrap_or("");
        let htitle = HSTRING::from(title);
        unsafe {
            if let Ok(hwnd) = FindWindowW(PCWSTR::null(), &htitle) {
                let visible = IsWindowVisible(hwnd).as_bool();
                debug_log!(
                    "Tray: toggle visibility (was visible={}) -> {}",
                    visible,
                    !visible
                );
                let _ = ShowWindow(hwnd, if visible { SW_HIDE } else { SW_SHOW });
                if !visible {
                    // Force a repaint and wake up the eframe event loop.
                    let _ = InvalidateRect(hwnd, None, true);
                    let _ = PostMessageW(hwnd, WM_USER, WPARAM(0), LPARAM(0));
                }
            } else {
                debug_log!("Tray: toggle visibility failed, window not found");
            }
        }
    }

    /// Ensure the main widget window is visible (used before opening settings).
    fn show_window() {
        let title = WIDGET_TITLE.get().map(|s| s.as_str()).unwrap_or("");
        let htitle = HSTRING::from(title);
        unsafe {
            if let Ok(hwnd) = FindWindowW(PCWSTR::null(), &htitle) {
                if !IsWindowVisible(hwnd).as_bool() {
                    debug_log!("Tray: showing window for settings");
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    // Force a repaint and wake up the eframe event loop.
                    let _ = InvalidateRect(hwnd, None, true);
                    let _ = PostMessageW(hwnd, WM_USER, WPARAM(0), LPARAM(0));
                }
            }
        }
    }

    /// Clean up the tray icon and exit the process.
    fn cleanup_and_exit() {
        debug_log!("Tray: exit requested, cleaning up and exiting");
        std::process::exit(0);
    }

    /// Initialize the system tray with the given window title and tray tooltip.
    /// The icon is a simple green circle; override `widget_title` to match the
    /// eframe window title so that show/hide can find the correct HWND.
    pub fn init_tray(widget_title: &str, tooltip: &str) {
        let _ = WIDGET_TITLE.set(widget_title.to_string());

        let icon = create_icon();

        let show_hide = MenuItem::new("显示/隐藏", true, None);
        let refresh = MenuItem::new("刷新", true, None);
        let settings = MenuItem::new("设置", true, None);
        let exit = MenuItem::new("退出", true, None);

        let show_hide_id = show_hide.id().clone();
        let refresh_id = refresh.id().clone();
        let settings_id = settings.id().clone();
        let exit_id = exit.id().clone();

        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == exit_id {
                debug_log!("Tray menu: exit clicked");
                cleanup_and_exit();
                return;
            }
            if event.id == show_hide_id {
                debug_log!("Tray menu: show/hide clicked");
                toggle_window_visibility();
                return;
            }
            if event.id == settings_id {
                debug_log!("Tray menu: settings clicked");
                SETTINGS_REQUESTED.store(true, Ordering::SeqCst);
                show_window();
                return;
            }
            if event.id == refresh_id {
                debug_log!("Tray menu: refresh clicked");
                push_command(TrayCommand::Refresh);
            }
        }));

        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click { .. } = event {
                debug_log!("Tray icon: left-click (toggle visibility)");
                toggle_window_visibility()
            }
        }));

        let menu = Menu::new();
        let _ = menu.append(&show_hide);
        let _ = menu.append(&refresh);
        let _ = menu.append(&settings);
        let _ = menu.append(&exit);

        let ttip = tooltip.to_string();
        match TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_tooltip(ttip)
            .build()
        {
            Ok(tray) => {
                TRAY_ICON.with(|cell| {
                    *cell.borrow_mut() = Some(tray);
                });
            }
            Err(e) => {
                debug_log!("Failed to create tray icon: {:?}", e);
            }
        }
    }

    pub fn check_events() -> Option<TrayCommand> {
        if let Ok(mut queue) = EVENT_QUEUE.lock() {
            queue.pop_front()
        } else {
            None
        }
    }

    fn create_icon() -> Icon {
        let size = 32u32;
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);
        let center = size as f32 / 2.0;
        let radius = size as f32 / 2.0 - 2.0;

        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist <= radius {
                    rgba.push(76);
                    rgba.push(175);
                    rgba.push(80);
                    rgba.push(255);
                } else {
                    rgba.push(0);
                    rgba.push(0);
                    rgba.push(0);
                    rgba.push(0);
                }
            }
        }

        Icon::from_rgba(rgba, size, size).expect("Failed to create tray icon from RGBA data")
    }
