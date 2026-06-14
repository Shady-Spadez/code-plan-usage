#[cfg(windows)]
pub mod tray {
    use std::cell::UnsafeCell;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use tray_icon::{
        menu::{Menu, MenuEvent, MenuItem},
        Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOW,
    };

    use crate::debug_log;

    pub enum TrayCommand {
        Refresh,
    }

    /// Thread-safe queue for tray commands pushed by event handlers.
    static EVENT_QUEUE: std::sync::LazyLock<Mutex<VecDeque<TrayCommand>>> =
        std::sync::LazyLock::new(|| Mutex::new(VecDeque::new()));

    /// Wrapper to allow storing a !Sync type (TrayIcon) in a static.
    ///
    /// SAFETY: All access happens on the main thread — the menu event
    /// callback runs on the same thread that created the tray icon window.
    struct MainThreadCell<T>(UnsafeCell<T>);
    unsafe impl<T> Sync for MainThreadCell<T> {}

    /// Stores the tray icon so it can be explicitly dropped to remove it
    /// from the system tray before the process exits.
    static TRAY_ICON: MainThreadCell<Option<TrayIcon>> = MainThreadCell(UnsafeCell::new(None));

    /// Flag set by the Settings menu callback; checked in the eframe update loop.
    pub static SETTINGS_REQUESTED: AtomicBool = AtomicBool::new(false);

    fn push_command(cmd: TrayCommand) {
        if let Ok(mut queue) = EVENT_QUEUE.lock() {
            queue.push_back(cmd);
        }
    }

    /// Find the main widget window by title and toggle its visibility.
    fn toggle_window_visibility() {
        let title = windows::core::w!("Coding Plan Widget");
        unsafe {
            if let Ok(hwnd) = FindWindowW(PCWSTR::null(), title) {
                let visible = IsWindowVisible(hwnd).as_bool();
                debug_log!(
                    "Tray: toggle visibility (was visible={}) -> {}",
                    visible,
                    !visible
                );
                let _ = ShowWindow(hwnd, if visible { SW_HIDE } else { SW_SHOW });
            } else {
                debug_log!("Tray: toggle visibility failed, window not found");
            }
        }
    }

    /// Ensure the main widget window is visible (used before opening settings).
    fn show_window() {
        let title = windows::core::w!("Coding Plan Widget");
        unsafe {
            if let Ok(hwnd) = FindWindowW(PCWSTR::null(), title) {
                if !IsWindowVisible(hwnd).as_bool() {
                    debug_log!("Tray: showing window for settings");
                    let _ = ShowWindow(hwnd, SW_SHOW);
                }
            }
        }
    }

    /// Clean up the tray icon and exit the process.
    /// Called directly from the menu event handler so it works even when
    /// the main window is hidden (and the eframe update loop is paused).
    fn cleanup_and_exit() {
        debug_log!("Tray: exit requested, cleaning up and exiting");
        // SAFETY: Only accessed from the main thread (the menu callback
        // runs on the same thread that created the tray icon).
        let tray_icon = unsafe { &mut *TRAY_ICON.0.get() };
        // Dropping the TrayIcon sends NIM_DELETE to remove it from the
        // system tray on Windows.
        tray_icon.take();
        std::process::exit(0);
    }

    pub fn init_tray() {
        let icon = create_icon();

        let show_hide = MenuItem::new("显示/隐藏", true, None);
        let refresh = MenuItem::new("刷新", true, None);
        let settings = MenuItem::new("设置", true, None);
        let exit = MenuItem::new("退出", true, None);

        let show_hide_id = show_hide.id().clone();
        let refresh_id = refresh.id().clone();
        let settings_id = settings.id().clone();
        let exit_id = exit.id().clone();

        // Handle all menu commands directly in the callback so they work
        // even when the main window is hidden and the eframe update loop
        // is not running.
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
                show_window(); // ensure window is visible so update() runs
                return;
            }
            if event.id == refresh_id {
                debug_log!("Tray menu: refresh clicked");
                push_command(TrayCommand::Refresh);
            }
        }));

        // Left-click on tray icon toggles window visibility.
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
            TrayIconEvent::Click { .. } => {
                debug_log!("Tray icon: left-click (toggle visibility)");
                toggle_window_visibility()
            }
            _ => {}
        }));

        let menu = Menu::new();
        let _ = menu.append(&show_hide);
        let _ = menu.append(&refresh);
        let _ = menu.append(&settings);
        let _ = menu.append(&exit);

        match TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_tooltip("Coding Plan Widget")
            .build()
        {
            Ok(tray) => {
                // SAFETY: init_tray is called from main before the event
                // loop starts, so no concurrent access is possible.
                let tray_icon = unsafe { &mut *TRAY_ICON.0.get() };
                *tray_icon = Some(tray);
            }
            Err(e) => {
                debug_log!("Failed to create tray icon: {:?}", e);
            }
        }
    }

    pub fn check_events() -> Option<TrayCommand> {
        // Poll from the event queue populated by the set_event_handler callbacks.
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
                    // Green circle
                    rgba.push(76);
                    rgba.push(175);
                    rgba.push(80);
                    rgba.push(255);
                } else {
                    // Transparent
                    rgba.push(0);
                    rgba.push(0);
                    rgba.push(0);
                    rgba.push(0);
                }
            }
        }

        Icon::from_rgba(rgba, size, size).expect("Failed to create tray icon from RGBA data")
    }
}
