//! WebView2-based login for dash.coconut.is to extract JWT Authorization token.
//!
//! The Coconut platform uses a JWT Bearer token (not cookies) for API auth.
//! This module opens a WebView2 window to `https://dash.coconut.is/account/settings/`,
//! detects login via DOM inspection, then extracts the token from localStorage.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};

use coding_plan_widget_shared::debug_log;

const TARGET_URL: &str = "https://dash.coconut.is/account/settings/";

const LOGIN_CHECK_SCRIPT: &str = r#"
(function() {
    var checks = 0;
    var maxChecks = 60;
    var timer = setInterval(function() {
        checks++;
        var token = localStorage.getItem('token') || localStorage.getItem('authToken')
            || localStorage.getItem('accessToken') || localStorage.getItem('jwt');
        if (token) {
            clearInterval(timer);
            window.chrome.webview.postMessage('TOKEN:' + token);
            return;
        }
        if (document.querySelector('[data-testid="user-menu"]') ||
            document.querySelector('.user-avatar') ||
            document.querySelector('.account-info')) {
            clearInterval(timer);
            var state = window.__NEXT_DATA__ || window.__INITIAL_STATE__;
            if (state && state.token) {
                window.chrome.webview.postMessage('TOKEN:' + state.token);
            } else {
                var cookies = document.cookie.split(';');
                for (var i = 0; i < cookies.length; i++) {
                    var c = cookies[i].trim();
                    if (c.startsWith('token=') || c.startsWith('jwt=')) {
                        window.chrome.webview.postMessage('TOKEN:' + c.split('=')[1]);
                        return;
                    }
                }
                window.chrome.webview.postMessage('LOGIN_DETECTED_NO_TOKEN');
            }
        } else if (checks >= maxChecks) {
            clearInterval(timer);
            window.chrome.webview.postMessage('NO_LOGIN');
        }
    }, 1000);
})();
"#;

const SILENT_LOGIN_CHECK_SCRIPT: &str = r#"
(function() {
    var checks = 0;
    var maxChecks = 20;
    var timer = setInterval(function() {
        checks++;
        var token = localStorage.getItem('token') || localStorage.getItem('authToken')
            || localStorage.getItem('accessToken') || localStorage.getItem('jwt');
        if (token) {
            clearInterval(timer);
            window.chrome.webview.postMessage('TOKEN:' + token);
            return;
        }
        if (checks >= maxChecks) {
            clearInterval(timer);
            window.chrome.webview.postMessage('NO_LOGIN');
        }
    }, 500);
})();
"#;

pub fn try_extract_token() -> mpsc::Receiver<Option<String>> {
    debug_log!("Coconut WebView2: starting login window");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || run_login_window(tx));
    rx
}

pub fn try_silent_extract_token() -> mpsc::Receiver<Option<String>> {
    debug_log!("Coconut WebView2: starting silent token extraction");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || run_silent_extraction(tx));
    rx
}

use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
    CreateCoreWebView2EnvironmentWithOptions,
};
use webview2_com::{
    CreateCoreWebView2EnvironmentCompletedHandler,
    CreateCoreWebView2ControllerCompletedHandler,
    AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    WebMessageReceivedEventHandler,
};
use windows::Win32::System::WinRT::EventRegistrationToken;

struct WindowData {
    controller: ICoreWebView2Controller,
    fetching: bool,
}

struct SilentExtractionData {
    #[allow(dead_code)]
    controller: ICoreWebView2Controller,
    tx_cell: Rc<RefCell<Option<Sender<Option<String>>>>>,
}

fn run_login_window(tx: Sender<Option<String>>) {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); }

    let class_name = windows::core::w!("CoconutLoginWindow");
    let hinstance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap() };

    let wnd_class = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        style: windows::Win32::UI::WindowsAndMessaging::CS_HREDRAW
            | windows::Win32::UI::WindowsAndMessaging::CS_VREDRAW,
        ..Default::default()
    };
    unsafe { windows::Win32::UI::WindowsAndMessaging::RegisterClassW(&wnd_class); }

    let hwnd = match unsafe {
        windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE::default(),
            class_name,
            windows::core::w!("登录 Coconut"),
            windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            1024, 768, None, None, hinstance, None,
        )
    } {
        Ok(h) => h,
        Err(e) => { debug_log!("Coconut WebView2: window failed: {:?}", e); let _ = tx.send(None); return; }
    };

    let env = match create_env() {
        Ok(e) => e,
        Err(e) => { debug_log!("Coconut WebView2: env failed: {:?}", e); let _ = tx.send(None); return; }
    };

    let (controller, webview) = match create_controller(hwnd, &env) {
        Ok(c) => c,
        Err(e) => { debug_log!("Coconut WebView2: controller failed: {:?}", e); let _ = tx.send(None); return; }
    };

    unsafe {
        controller.SetIsVisible(true).ok();
        let _ = controller.SetBounds(windows::Win32::Foundation::RECT { left: 0, top: 0, right: 1024, bottom: 768 });
    }

    let window_data = Box::into_raw(Box::new(WindowData { controller, fetching: false }));
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA, window_data as isize);
    }

    inject_script(&webview, LOGIN_CHECK_SCRIPT);
    let tx_cell = Rc::new(RefCell::new(Some(tx)));
    setup_message_handler(&webview, tx_cell, hwnd);

    unsafe {
        let url = windows::core::HSTRING::from(TARGET_URL);
        let _ = webview.Navigate(&url);
    }

    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE);
        let _ = windows::Win32::Graphics::Gdi::UpdateWindow(hwnd);
    }

    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
    loop {
        let ret = unsafe { windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 == 0 || ret.0 == -1 { break; }
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wndproc(
    hwnd: windows::Win32::Foundation::HWND, msg: u32,
    wparam: windows::Win32::Foundation::WPARAM, lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcW, WM_CLOSE, WM_DESTROY, WM_SIZE, GWLP_USERDATA};

    match msg {
        WM_SIZE => {
            let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if ptr != 0 {
                let data = &mut *(ptr as *mut WindowData);
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;
                unsafe { let _ = data.controller.SetBounds(windows::Win32::Foundation::RECT { left: 0, top: 0, right: width, bottom: height }); }
            }
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_CLOSE => {
            let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if ptr != 0 {
                let data = &*(ptr as *mut WindowData);
                if data.fetching { return windows::Win32::Foundation::LRESULT(0); }
            }
            unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) }.ok();
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if ptr != 0 { let _ = unsafe { Box::from_raw(ptr as *mut WindowData) }; }
            unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0) };
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

const SILENT_TIMEOUT_TIMER_ID: usize = 100;
const SILENT_TIMEOUT_MS: u32 = 20000;

fn run_silent_extraction(tx: Sender<Option<String>>) {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); }

    let class_name = windows::core::w!("CoconutSilentWindow");
    let hinstance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap() };

    let wnd_class = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
        lpfnWndProc: Some(silent_wndproc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe { windows::Win32::UI::WindowsAndMessaging::RegisterClassW(&wnd_class); }

    let hwnd = match unsafe {
        windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE::default(),
            class_name, windows::core::w!(""),
            windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
            0, 0, 1, 1, None, None, hinstance, None,
        )
    } {
        Ok(h) => h,
        Err(e) => { debug_log!("Coconut silent: window failed: {:?}", e); let _ = tx.send(None); return; }
    };

    let env = match create_env() {
        Ok(e) => e,
        Err(e) => { debug_log!("Coconut silent: env failed: {:?}", e); let _ = tx.send(None); return; }
    };

    let (controller, webview) = match create_controller(hwnd, &env) {
        Ok(c) => c,
        Err(e) => { debug_log!("Coconut silent: controller failed: {:?}", e); let _ = tx.send(None); return; }
    };

    unsafe {
        controller.SetIsVisible(false).ok();
        let _ = controller.SetBounds(windows::Win32::Foundation::RECT { left: 0, top: 0, right: 1, bottom: 1 });
    }

    let tx_cell = Rc::new(RefCell::new(Some(tx)));
    let window_data = Box::into_raw(Box::new(SilentExtractionData { controller, tx_cell: tx_cell.clone() }));
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA, window_data as isize);
    }

    inject_script(&webview, SILENT_LOGIN_CHECK_SCRIPT);
    setup_message_handler(&webview, tx_cell, hwnd);

    unsafe {
        let url = windows::core::HSTRING::from(TARGET_URL);
        let _ = webview.Navigate(&url);
    }

    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetTimer(hwnd, SILENT_TIMEOUT_TIMER_ID, SILENT_TIMEOUT_MS, None);
    }

    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
    loop {
        let ret = unsafe { windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 == 0 || ret.0 == -1 { break; }
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            let _ = windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }
    }

    let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA) };
    if ptr != 0 {
        let data = unsafe { &mut *(ptr as *mut SilentExtractionData) };
        if let Some(tx) = data.tx_cell.borrow_mut().take() { let _ = tx.send(None); }
    }
}

unsafe extern "system" fn silent_wndproc(
    hwnd: windows::Win32::Foundation::HWND, msg: u32,
    wparam: windows::Win32::Foundation::WPARAM, lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcW, KillTimer, WM_CLOSE, WM_DESTROY, WM_TIMER, GWLP_USERDATA};

    match msg {
        WM_TIMER => {
            if wparam.0 == SILENT_TIMEOUT_TIMER_ID {
                let _ = unsafe { KillTimer(hwnd, SILENT_TIMEOUT_TIMER_ID) };
                let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
                if ptr != 0 {
                    let data = unsafe { &mut *(ptr as *mut SilentExtractionData) };
                    if let Some(tx) = data.tx_cell.borrow_mut().take() { let _ = tx.send(None); }
                }
                unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) }.ok();
            }
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_CLOSE => { unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) }.ok(); windows::Win32::Foundation::LRESULT(0) }
        WM_DESTROY => {
            let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if ptr != 0 {
                unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); }
                let _ = unsafe { Box::from_raw(ptr as *mut SilentExtractionData) };
            }
            unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0) };
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn create_env() -> Result<ICoreWebView2Environment, webview2_com::Error> {
    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(|handler| unsafe {
            CreateCoreWebView2EnvironmentWithOptions(None, None, None, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, environment| {
            error_code?;
            tx.send(environment.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER))).expect("send");
            Ok(())
        }),
    )?;
    rx.recv().map_err(|_| webview2_com::Error::SendError)?.map_err(webview2_com::Error::WindowsError)
}

fn create_controller(
    hwnd: windows::Win32::Foundation::HWND, env: &ICoreWebView2Environment,
) -> Result<(ICoreWebView2Controller, ICoreWebView2), webview2_com::Error> {
    let (tx, rx) = mpsc::channel();
    let env = env.clone();
    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe { env.CreateCoreWebView2Controller(hwnd, &handler).map_err(webview2_com::Error::WindowsError) }),
        Box::new(move |error_code, controller| {
            error_code?;
            tx.send(controller.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER))).expect("send");
            Ok(())
        }),
    )?;
    let controller = rx.recv().map_err(|_| webview2_com::Error::SendError)?.map_err(webview2_com::Error::WindowsError)?;
    let webview = unsafe { controller.CoreWebView2() }.map_err(webview2_com::Error::WindowsError)?;
    Ok((controller, webview))
}

fn inject_script(webview: &ICoreWebView2, script: &'static str) {
    let (tx, rx) = mpsc::channel();
    let wv = Rc::new(webview.clone());
    let result = AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let js = windows::core::HSTRING::from(script);
            wv.AddScriptToExecuteOnDocumentCreated(&js, &handler).map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, _id| { tx.send(error_code).expect("send"); Ok(()) }),
    );
    match result {
        Ok(()) => match rx.recv() {
            Ok(Ok(())) => debug_log!("Coconut WebView2: script injected"),
            Ok(Err(e)) => debug_log!("Coconut WebView2: script injection failed: {:?}", e),
            Err(_) => debug_log!("Coconut WebView2: script injection channel error"),
        },
        Err(e) => debug_log!("Coconut WebView2: script injection wait failed: {:?}", e),
    }
}

fn setup_message_handler(
    webview: &ICoreWebView2,
    tx_cell: Rc<RefCell<Option<Sender<Option<String>>>>>,
    hwnd: windows::Win32::Foundation::HWND,
) {
    let handler = WebMessageReceivedEventHandler::create(Box::new(
        move |_webview, args| {
            if let Some(args) = args {
                let mut message = windows::core::PWSTR(std::ptr::null_mut());
                if unsafe { args.TryGetWebMessageAsString(&mut message) }.is_ok() {
                    let message = webview2_com::take_pwstr(message);
                    debug_log!("Coconut WebView2: message received: {}", message);

                    if let Some(token) = message.strip_prefix("TOKEN:") {
                        debug_log!("Coconut WebView2: token extracted");
                        if let Some(tx) = tx_cell.borrow_mut().take() {
                            let _ = tx.send(Some(token.to_string()));
                        }
                        unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) }.ok();
                    } else if message == "LOGIN_DETECTED_NO_TOKEN" {
                        debug_log!("Coconut WebView2: login detected but no token found");
                        if let Some(tx) = tx_cell.borrow_mut().take() { let _ = tx.send(None); }
                    } else if message == "NO_LOGIN" {
                        debug_log!("Coconut WebView2: no login detected");
                        if let Some(tx) = tx_cell.borrow_mut().take() { let _ = tx.send(None); }
                        unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) }.ok();
                    }
                }
            }
            Ok(())
        },
    ));

    unsafe {
        let mut token = EventRegistrationToken::default();
        if let Err(e) = webview.add_WebMessageReceived(&handler, &mut token) {
            debug_log!("Coconut WebView2: add_WebMessageReceived failed: {:?}", e);
        }
    }
}
