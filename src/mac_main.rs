
use khana::app::App;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{MenuEvent, MenuItem};
use std::sync::Arc;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::http::{header, Request, Response};
use wry::{WebViewBuilder, WebViewBuilderExtDarwin};

const START_URL: &str = "khana://reader/";

const PDF_URL: &str = "khanapdf://pdf/pdfjs/frame.html";

const MENU_COMMANDS: &[&str] = &["speed-down", "speed-up", "speed-reset", "settings", "mp3", "pdf", "font-up", "font-down"];

fn item(id: &str, text: &str, mods: Modifiers, key: Code) -> MenuItem {
    MenuItem::with_id(id, text, true, Some(Accelerator::new(mods, key)))
}

fn menu() -> muda::Menu {
    use muda::{Menu, PredefinedMenuItem, Submenu};
    let cmd = Modifiers::META;
    let m = Menu::new();
    let app = Submenu::new("Khana Reader (Rust)", true);
    let _ = app.append_items(&[
        &item("settings", "تنظیمات…", cmd, Code::Comma),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::quit(None),
    ]);
    let file = Submenu::new("پرونده", true);
    let _ = file.append_items(&[
        &item("mp3", "ذخیره‌ی صدا…", cmd, Code::KeyS),
        &item("pdf", "ذخیره‌ی ترجمه‌ی فارسی به صورت PDF…", cmd | Modifiers::SHIFT, Code::KeyS),
    ]);
    let edit = Submenu::new("ویرایش", true);
    let _ = edit.append_items(&[
        &PredefinedMenuItem::undo(None),
        &PredefinedMenuItem::redo(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
    ]);
    let speed = Submenu::new("سرعت", true);
    let _ = speed.append_items(&[
        &item("speed-down", "آهسته‌تر", cmd, Code::BracketLeft),
        &item("speed-up", "تندتر", cmd, Code::BracketRight),
        &PredefinedMenuItem::separator(),
        &item("speed-reset", "سرعت عادی", cmd, Code::Digit0),
    ]);
    let view = Submenu::new("نما", true);
    let _ = view.append_items(&[
        &item("font-up", "قلم بزرگ‌تر", cmd, Code::Equal),
        &item("font-down", "قلم کوچک‌تر", cmd, Code::Minus),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::fullscreen(None),
    ]);
    let window = Submenu::new("پنجره", true);
    let _ = window.append_items(&[&PredefinedMenuItem::minimize(None), &PredefinedMenuItem::close_window(None)]);
    let _ = m.append_items(&[&app, &file, &edit, &speed, &view, &window]);
    m
}

enum UserEvent {
    Menu(MenuEvent),
    PageGone,
    PdfGone,
}

pub fn run() {
    khana::ffi::lower_priority();
    std::thread::spawn(|| loop {
        khana::tts::set_heat(khana::mac::heat_level());
        if std::env::var("KHANA_SELFTEST").is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(15));
            continue;
        }
        if let Some((percent, charging)) = khana::mac::battery() {
            khana::tts::set_power(percent, charging);
        }
        std::thread::sleep(std::time::Duration::from_secs(15));
    });
    let app = Arc::new(App::new(khana::engines::engines()));
    let web = khana::vocab::web_dir();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let m = menu();
    m.init_for_nsapp();
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = proxy.send_event(UserEvent::Menu(e));
    }));
    let window = WindowBuilder::new()
        .with_title("خوانا (Rust)")
        .with_inner_size(tao::dpi::LogicalSize::new(1100.0, 820.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(420.0, 560.0))
        .build(&event_loop)
        .expect("window");

    let bridge = Arc::new(khana::pdfview::Bridge::default());
    let (a, b, web2) = (app.clone(), bridge.clone(), web.clone());
    let pdf_handler = move |_id: wry::WebViewId, req: Request<Vec<u8>>, responder: wry::RequestAsyncResponder| {
        let (a, b, web) = (a.clone(), b.clone(), web2.clone());
        std::thread::spawn(move || {
            let (method, path) = (req.method().as_str().to_string(), req.uri().path().to_string());
            let q = khana::app::query(req.uri().query().unwrap_or(""));
            let (status, ctype, headers, body) = if path.starts_with("/view/") {
                b.view(&method, &path, &q, req.body())
            } else if method == "GET" && path.starts_with("/pdfjs/") {
                let r = a.handle("GET", &path, "", None, &[], &web);
                let headers = r.headers.into_iter().filter(|(k, _)| k != "Access-Control-Allow-Origin")
                    .map(|(k, v)| if k == "Content-Security-Policy" { (k, v.replace("khana:", "khanapdf:").replace("frame-ancestors khanapdf:", "frame-ancestors 'none'")) } else { (k, v) }).collect();
                (r.status, r.ctype, headers, r.body)
            } else {
                (404, "text/plain", Vec::new(), Vec::new())
            };
            let mut rb = Response::builder().status(status).header(header::CONTENT_TYPE, ctype).header(header::CACHE_CONTROL, "no-store")
                .header("X-Content-Type-Options", "nosniff").header("Referrer-Policy", "no-referrer");
            for (k, v) in &headers {
                rb = rb.header(k.as_str(), v.as_str());
            }
            responder.respond(rb.body(body).unwrap_or_else(|_| Response::builder().status(500).body(Vec::new()).unwrap_or_default()));
        });
    };
    let a = app.clone();
    let b = bridge.clone();
    let handler = move |_id: wry::WebViewId, req: Request<Vec<u8>>, responder: wry::RequestAsyncResponder| {
        let a = a.clone();
        let b = b.clone();
        let web = web.clone();
        std::thread::spawn(move || {
            let method = req.method().as_str().to_string();
            let path = req.uri().path().to_string();
            let qs = req.uri().query().unwrap_or("").to_string();
            let token = req.headers().get("X-Khana-Token").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
            let r = if method == "POST" && path == "/api/pdfview" && token.as_deref() == Some(a.token.as_str()) {
                let (status, ctype, headers, body) = b.call(&khana::app::query(&qs), req.body());
                khana::app::Reply { status, ctype, body, headers }
            } else {
                a.handle(&method, &path, &qs, token.as_deref(), req.body(), &web)
            };
            if std::env::var("KHANA_SELFTEST").is_ok() {
                eprintln!("request {method} {path} body {} -> {} {}", req.body().len(), r.status, if r.status >= 400 { String::from_utf8_lossy(&r.body).into_owned() } else { String::new() });
            }
            let mut b = Response::builder()
                .status(r.status)
                .header(header::CONTENT_TYPE, r.ctype)
                .header(header::CACHE_CONTROL, "no-store")
                .header("X-Content-Type-Options", "nosniff")
                .header("Referrer-Policy", "no-referrer");
            for (k, v) in &r.headers {
                b = b.header(k.as_str(), v.as_str());
            }
            let resp = b.body(r.body).unwrap_or_else(|_| Response::builder().status(500).body(Vec::new()).unwrap_or_default());
            responder.respond(resp);
        });
    };
    let selftest = if std::env::var("KHANA_SELFTEST").is_ok() { include_str!("selftest.js") } else { "" };
    let selftest = format!("window.KHANA_PDF_VIEW = true;\n{selftest}");
    let gone = event_loop.create_proxy();
    let webview = WebViewBuilder::new()
        .with_initialization_script(selftest)
        .with_asynchronous_custom_protocol("khana".into(), handler)
        .with_url(START_URL)
        .with_incognito(true)
        .with_navigation_handler(|url| url.starts_with("khana://"))
        .with_new_window_req_handler(|_url, _features| wry::NewWindowResponse::Deny)
        .with_download_started_handler(|_url, _path| false)
        .with_drag_drop_handler(|_e| false)
        .with_permission_handler(|kind| match kind {
            wry::PermissionKind::Microphone => wry::PermissionResponse::Allow,
            _ => wry::PermissionResponse::Deny,
        })
        .with_devtools(cfg!(debug_assertions))
        .with_on_web_content_process_terminate_handler(move || {
            let _ = gone.send_event(UserEvent::PageGone);
        })
        .build(&window)
        .expect("webview");

    let pdf_gone = event_loop.create_proxy();
    let pdfview = WebViewBuilder::new()
        .with_asynchronous_custom_protocol("khanapdf".into(), pdf_handler)
        .with_url(PDF_URL)
        .with_incognito(true)
        .with_navigation_handler(|url| url.starts_with("khanapdf://pdf/"))
        .with_new_window_req_handler(|_url, _features| wry::NewWindowResponse::Deny)
        .with_download_started_handler(|_url, _path| false)
        .with_drag_drop_handler(|_e| true)
        .with_permission_handler(|_kind| wry::PermissionResponse::Deny)
        .with_devtools(false)
        .with_focused(false)
        .with_bounds(wry::Rect { position: tao::dpi::LogicalPosition::new(-1.0, -1.0).into(), size: tao::dpi::LogicalSize::new(1.0, 1.0).into() })
        .with_on_web_content_process_terminate_handler(move || {
            let _ = pdf_gone.send_event(UserEvent::PdfGone);
        })
        .build_as_child(&window)
        .expect("pdf view");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Menu(e)) => {
                let id = e.id.0.as_str();
                if MENU_COMMANDS.contains(&id) {
                    let _ = webview.evaluate_script(&format!("window.khanaMenu && window.khanaMenu({id:?})"));
                }
            }
            Event::UserEvent(UserEvent::PageGone) => {
                eprintln!("the page's process ended; loading the page again");
                let _ = webview.load_url(START_URL);
            }
            Event::UserEvent(UserEvent::PdfGone) => {
                eprintln!("pdf.js's process ended; starting it again");
                bridge.reset();
                let _ = pdfview.load_url(PDF_URL);
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                app.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
