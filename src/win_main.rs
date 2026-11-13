
use khana::app::App;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{MenuEvent, MenuItem};
use std::sync::Arc;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::http::{header, Request, Response};
use tao::platform::windows::WindowExtWindows;
use wry::{WebContext, WebViewBuilder, WebViewBuilderExtWindows};

const START_URL: &str = "http://khana.localhost/";

const MENU_COMMANDS: &[&str] = &["speed-down", "speed-up", "speed-reset", "settings", "mp3", "pdf", "font-up", "font-down"];

fn item(id: &str, text: &str, mods: Modifiers, key: Code) -> MenuItem {
    MenuItem::with_id(id, text, true, Some(Accelerator::new(mods, key)))
}

fn menu() -> muda::Menu {
    use muda::{Menu, Submenu};
    let ctrl = Modifiers::CONTROL;
    let m = Menu::new();
    let file = Submenu::new("پرونده", true);
    let _ = file.append_items(&[
        &item("settings", "تنظیمات…", ctrl, Code::Comma),
        &item("mp3", "ذخیره‌ی صدا…", ctrl, Code::KeyS),
    ]);
    let speed = Submenu::new("سرعت", true);
    let _ = speed.append_items(&[
        &item("speed-down", "آهسته‌تر", ctrl, Code::BracketLeft),
        &item("speed-up", "تندتر", ctrl, Code::BracketRight),
        &item("speed-reset", "سرعت عادی", ctrl, Code::Digit0),
    ]);
    let view = Submenu::new("نما", true);
    let _ = view.append_items(&[
        &item("font-up", "قلم بزرگ‌تر", ctrl, Code::Equal),
        &item("font-down", "قلم کوچک‌تر", ctrl, Code::Minus),
    ]);
    let _ = m.append_items(&[&file, &speed, &view]);
    m
}

enum UserEvent {
    Menu(MenuEvent),
}

pub fn run() {
    khana::ffi::lower_priority();
    let app = Arc::new(App::new(khana::engines::engines()));
    let web = khana::vocab::web_dir();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let m = menu();
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
    khana::ffi::winmenu::attach(&m, window.hwnd() as isize);

    let a = app.clone();
    let handler = move |_id: wry::WebViewId, req: Request<Vec<u8>>, responder: wry::RequestAsyncResponder| {
        let a = a.clone();
        let web = web.clone();
        std::thread::spawn(move || {
            let method = req.method().as_str().to_string();
            let path = req.uri().path().to_string();
            let qs = req.uri().query().unwrap_or("").to_string();
            let token = req.headers().get("X-Khana-Token").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
            let r = a.handle(&method, &path, &qs, token.as_deref(), req.body(), &web);
            if path == "/api/init" && std::env::var("KHANA_SMOKE").is_ok() {
                eprintln!("SMOKE page loaded {}", r.status);
            }
            if khana::app::selftest_on() {
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
    let selftest = if khana::app::selftest_on() { "" } else { "" };
    let mut web_context = WebContext::new(Some(khana::app::data_dir().join("webview-offline")));
    let webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_additional_browser_args(concat!(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection ",
            "--host-resolver-rules=\"MAP * ~NOTFOUND\" ",
            "--proxy-server=\"http://0.0.0.0:1\" --proxy-bypass-list=\"<-loopback>\" ",
            "--disable-background-networking --disable-component-update --disable-domain-reliability --no-pings"
        ))
        .with_initialization_script(selftest)
        .with_asynchronous_custom_protocol("khana".into(), handler)
        .with_url(START_URL)
        .with_incognito(true)
        .with_navigation_handler(|url| url == "http://khana.localhost" || url.starts_with(START_URL))
        .with_new_window_req_handler(|_url, _features| wry::NewWindowResponse::Deny)
        .with_download_started_handler(|_url, _path| false)
        .with_drag_drop_handler(|_e| false)
        .with_permission_handler(|kind| match kind {
            wry::PermissionKind::Microphone => wry::PermissionResponse::Allow,
            _ => wry::PermissionResponse::Deny,
        })
        .with_devtools(cfg!(debug_assertions))
        .build(&window)
        .expect("webview");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Menu(e)) => {
                let id = e.id.0.as_str();
                if MENU_COMMANDS.contains(&id) {
                    let _ = webview.evaluate_script(&format!("window.khanaMenu && window.khanaMenu({id:?})"));
                }
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                app.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
