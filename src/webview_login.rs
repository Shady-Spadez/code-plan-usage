use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};

use crate::debug_log;

pub struct BrowserCredentials {
    pub cookie: String,
    pub csrf_token: String,
}

const TARGET_URL: &str = "https://console.volcengine.com/ark/region:ark+cn-beijing/openManagement?LLM=%7B%7D&advancedActiveKey=subscribe";

const COOKIE_CHECK_SCRIPT: &str = r#"
(function() {
    var timer = setInterval(function() {
        if (document.cookie.indexOf('AccountID') !== -1) {
            clearInterval(timer);
            setTimeout(function() {
                window.chrome.webview.postMessage('LOGIN_DETECTED');
            }, 5000);
        }
    }, 2000);
})();
"#;

pub fn try_extract_credentials() -> mpsc::Receiver<Option<BrowserCredentials>> {
    debug_log!("WebView2: starting login window");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || run_login_window(tx));
    rx
}

pub fn clear_webview_cookies() {
    debug_log!("WebView2: clearing all cookies");
    std::thread::spawn(|| {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); }

        let class_name = windows::core::w!("CodingPlanClearCookie");
        let hinstance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap() };

        let wnd_class = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
            lpfnWndProc: Some(clear_cookie_wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        unsafe { windows::Win32::UI::WindowsAndMessaging::RegisterClassW(&wnd_class); }

        let hwnd = match unsafe {
            windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE::default(),
                class_name,
                windows::core::w!(""),
                windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
                0, 0, 1, 1,
                None, None, hinstance, None,
            )
        } {
            Ok(h) => h,
            Err(e) => { debug_log!("WebView2: clear_cookies window failed: {:?}", e); return; }
        };

        let env = match create_env() {
            Ok(e) => e,
            Err(e) => { debug_log!("WebView2: clear_cookies env failed: {:?}", e); return; }
        };

        let (_controller, webview) = match create_controller(hwnd, &env) {
            Ok(c) => c,
            Err(e) => { debug_log!("WebView2: clear_cookies controller failed: {:?}", e); return; }
        };

        if let Some(cm) = get_cookie_manager(&webview) {
            match unsafe { cm.DeleteAllCookies() } {
                Ok(()) => debug_log!("WebView2: all cookies cleared"),
                Err(e) => debug_log!("WebView2: DeleteAllCookies failed: {:?}", e),
            }
        }

        unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd).ok(); }
    });
}

unsafe extern "system" fn clear_cookie_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
    ICoreWebView2CookieManager, ICoreWebView2_2,
    CreateCoreWebView2EnvironmentWithOptions,
};
use webview2_com::{
    CreateCoreWebView2EnvironmentCompletedHandler,
    CreateCoreWebView2ControllerCompletedHandler,
    AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    WebMessageReceivedEventHandler,
    GetCookiesCompletedHandler,
    take_pwstr,
};
use windows::core::Interface;
use windows::Win32::System::WinRT::EventRegistrationToken;

struct WindowData {
    controller: ICoreWebView2Controller,
    fetching: bool,
}

fn run_login_window(tx: Sender<Option<BrowserCredentials>>) {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); }

    let class_name = windows::core::w!("CodingPlanLoginWindow");
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
            windows::core::w!("登录火山引擎控制台"),
            windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            1024, 768,
            None, None, hinstance, None,
        )
    } {
        Ok(h) => h,
        Err(e) => {
            debug_log!("WebView2: failed to create window: {:?}", e);
            let _ = tx.send(None);
            return;
        }
    };

    let env = match create_env() {
        Ok(e) => e,
        Err(e) => {
            debug_log!("WebView2: failed to create environment: {:?}", e);
            let _ = tx.send(None);
            return;
        }
    };

    let (controller, webview) = match create_controller(hwnd, &env) {
        Ok(c) => c,
        Err(e) => {
            debug_log!("WebView2: failed to create controller: {:?}", e);
            let _ = tx.send(None);
            return;
        }
    };

    debug_log!("WebView2: controller created");

    unsafe {
        controller.SetIsVisible(true).ok();
        let _ = controller.SetBounds(windows::Win32::Foundation::RECT {
            left: 0, top: 0, right: 1024, bottom: 768,
        });
    }

    let window_data = Box::into_raw(Box::new(WindowData {
        controller,
        fetching: false,
    }));
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            window_data as isize,
        );
    }

    inject_script(&webview);

    let tx_cell = Rc::new(RefCell::new(Some(tx)));
    setup_message_handler(&webview, tx_cell, hwnd);

    unsafe {
        let url = windows::core::HSTRING::from(TARGET_URL);
        let _ = webview.Navigate(&url);
    }

    debug_log!("WebView2: showing maximized");
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE,
        );
        let _ = windows::Win32::Graphics::Gdi::UpdateWindow(hwnd);
    }

    debug_log!("WebView2: entering message loop");
    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
    loop {
        use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, TranslateMessage, DispatchMessageW};
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 == 0 || ret.0 == -1 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    debug_log!("WebView2: message loop exited");
}

unsafe extern "system" fn wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, WM_CLOSE, WM_DESTROY, WM_SIZE, GWLP_USERDATA,
    };

    match msg {
        WM_SIZE => {
            let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if ptr != 0 {
                let data = &mut *(ptr as *mut WindowData);
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;
                unsafe {
                    let _ = data.controller.SetBounds(windows::Win32::Foundation::RECT {
                        left: 0, top: 0, right: width, bottom: height,
                    });
                }
            }
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_CLOSE => {
            let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if ptr != 0 {
                let data = &*(ptr as *mut WindowData);
                if data.fetching {
                    return windows::Win32::Foundation::LRESULT(0);
                }
            }
            unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) }.ok();
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if ptr != 0 {
                let _ = unsafe { Box::from_raw(ptr as *mut WindowData) };
            }
            unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0) };
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn get_cookie_manager(webview: &ICoreWebView2) -> Option<ICoreWebView2CookieManager> {
    let webview2: ICoreWebView2_2 = match webview.cast() {
        Ok(wv) => wv,
        Err(e) => {
            debug_log!("WebView2: cast to ICoreWebView2_2 failed: {:?}", e);
            return None;
        }
    };
    match unsafe { webview2.CookieManager() } {
        Ok(cm) => Some(cm),
        Err(e) => {
            debug_log!("WebView2: CookieManager failed: {:?}", e);
            None
        }
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
            tx.send(
                environment
                    .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER)),
            )
            .expect("send");
            Ok(())
        }),
    )?;
    rx.recv()
        .map_err(|_| webview2_com::Error::SendError)?
        .map_err(webview2_com::Error::WindowsError)
}

fn create_controller(
    hwnd: windows::Win32::Foundation::HWND,
    env: &ICoreWebView2Environment,
) -> Result<(ICoreWebView2Controller, ICoreWebView2), webview2_com::Error> {
    let (tx, rx) = mpsc::channel();
    let env = env.clone();
    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            env.CreateCoreWebView2Controller(hwnd, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, controller| {
            error_code?;
            tx.send(
                controller
                    .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER)),
            )
            .expect("send");
            Ok(())
        }),
    )?;
    let controller = rx
        .recv()
        .map_err(|_| webview2_com::Error::SendError)?
        .map_err(webview2_com::Error::WindowsError)?;
    let webview = unsafe { controller.CoreWebView2() }.map_err(webview2_com::Error::WindowsError)?;
    Ok((controller, webview))
}

fn inject_script(webview: &ICoreWebView2) {
    let (tx, rx) = mpsc::channel();
    let wv = Rc::new(webview.clone());
    let result = AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let js = windows::core::HSTRING::from(COOKIE_CHECK_SCRIPT);
            wv.AddScriptToExecuteOnDocumentCreated(&js, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, _id| {
            tx.send(error_code).expect("send");
            Ok(())
        }),
    );
    match result {
        Ok(()) => match rx.recv() {
            Ok(Ok(())) => debug_log!("WebView2: script injected"),
            Ok(Err(e)) => debug_log!("WebView2: script injection failed: {:?}", e),
            Err(_) => debug_log!("WebView2: script injection channel error"),
        },
        Err(e) => debug_log!("WebView2: script injection wait failed: {:?}", e),
    }
}

fn setup_message_handler(
    webview: &ICoreWebView2,
    tx_cell: Rc<RefCell<Option<Sender<Option<BrowserCredentials>>>>>,
    hwnd: windows::Win32::Foundation::HWND,
) {
    let cookie_manager = match get_cookie_manager(webview) {
        Some(cm) => Rc::new(cm),
        None => return,
    };

    let handler = WebMessageReceivedEventHandler::create(Box::new(
        move |_webview, args| {
            if let Some(args) = args {
                let mut message = windows::core::PWSTR(std::ptr::null_mut());
                if unsafe { args.TryGetWebMessageAsString(&mut message) }.is_ok() {
                    let message = take_pwstr(message);
                    debug_log!("WebView2: WebMessage received: {}", message);

                    if message == "LOGIN_DETECTED" {
                        debug_log!("WebView2: login detected, fetching cookies");
                        set_fetching(hwnd, true);

                        let cm = cookie_manager.clone();
                        let tx_cell = tx_cell.clone();

                        let uri = windows::core::HSTRING::from("https://console.volcengine.com");
                        let callback = GetCookiesCompletedHandler::create(Box::new(
                            move |result, cookie_list| -> windows::core::Result<()> {
                                if let Ok(()) = result {
                                    if let Some(list) = cookie_list {
                                        if let Some(creds) = parse_cookies_from_list(&list) {
                                            debug_log!("WebView2: extracted {} cookies", creds.cookie.split(';').count());
                                            if let Some(tx) = tx_cell.borrow_mut().take() {
                                                let _ = tx.send(Some(creds));
                                            }
                                        } else {
                                            debug_log!("WebView2: parse_cookies_from_list returned None");
                                        }
                                    } else {
                                        debug_log!("WebView2: GetCookies returned no cookie list");
                                    }
                                } else {
                                    debug_log!("WebView2: GetCookies failed: {:?}", result.err());
                                }
                                set_fetching(hwnd, false);
                                Ok(())
                            }
                        ));

                        unsafe {
                            if let Err(e) = cm.GetCookies(&uri, &callback) {
                                debug_log!("WebView2: GetCookies call failed: {:?}", e);
                            }
                        }
                    }
                }
            }
            Ok(())
        },
    ));

    unsafe {
        let mut token = EventRegistrationToken::default();
        if let Err(e) = webview.add_WebMessageReceived(&handler, &mut token) {
            debug_log!("WebView2: add_WebMessageReceived failed: {:?}", e);
        }
    }
}

fn set_fetching(hwnd: windows::Win32::Foundation::HWND, value: bool) {
    let ptr = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
        )
    };
    if ptr != 0 {
        let data = unsafe { &mut *(ptr as *mut WindowData) };
        data.fetching = value;
    }
}

fn parse_cookies_from_list(
    list: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2CookieList,
) -> Option<BrowserCredentials> {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Cookie;

    let mut count: u32 = 0;
    if unsafe { list.Count(&mut count) }.is_err() {
        return None;
    }

    let mut cookie_pairs: Vec<String> = Vec::new();
    let mut csrf_token = String::new();

    for i in 0..count {
        let cookie: ICoreWebView2Cookie = match unsafe { list.GetValueAtIndex(i) } {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut name_pwstr = windows::core::PWSTR::null();
        let mut value_pwstr = windows::core::PWSTR::null();

        if unsafe { cookie.Name(&mut name_pwstr) }.is_err() {
            continue;
        }
        if unsafe { cookie.Value(&mut value_pwstr) }.is_err() {
            continue;
        }

        let name = take_pwstr(name_pwstr);
        let value = take_pwstr(value_pwstr);

        if name == "csrfToken" && csrf_token.is_empty() {
            csrf_token = value.clone();
        }

        cookie_pairs.push(format!("{}={}", name, value));
    }

    if cookie_pairs.is_empty() || csrf_token.is_empty() {
        return None;
    }

    Some(BrowserCredentials {
        cookie: cookie_pairs.join("; "),
        csrf_token,
    })
}
