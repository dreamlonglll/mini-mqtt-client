//! Mini MQTT Client — a lightweight MQTT debugging client built with Rust and
//! GPUI Kit.

// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// Localised text (see `locales/app.yml`) as a `SharedString`. Takes the
/// arguments of `rust_i18n::t!`: the key, then `name = value` pairs that fill
/// `%{name}`.
macro_rules! tr {
    ($($args:tt)*) => {
        gpui_kit::SharedString::from(rust_i18n::t!($($args)*))
    };
}

mod assets;
mod backend;
mod i18n;
mod state;
mod ui;

// Texts of the app (`locales/app.yml`), looked up with `rust_i18n::t!`.
rust_i18n::i18n!("locales", fallback = "en");

use gpui_kit::*;
use state::AppState;
use ui::app_view::AppView;

actions!(mini_mqtt, [Quit]);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,mini_mqtt_client=info")).init();
    install_panic_logger();

    gpui_kit::application().with_assets(assets::AppAssets).run(|cx| {
        gpui_kit::init(cx);
        // `MINI_MQTT_REDUCE_MOTION=1` (set by the E2E runs) turns transitions off,
        // so dialogs and drawers are where the accessibility tree says from the
        // first frame instead of sliding in under the test's clicks.
        if std::env::var("MINI_MQTT_REDUCE_MOTION").is_ok_and(|v| !v.is_empty() && v != "0") {
            cx.set_reduce_motion(true);
        }

        #[cfg(target_os = "macos")]
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        let state = cx.new(AppState::new);
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(960.), px(640.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Mini MQTT Client".into()),
                ..Default::default()
            }),
            app_id: Some(backend::config::APP_IDENTIFIER.into()),
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            window.set_window_title("Mini MQTT Client");
            cx.new(|cx| AppView::new(state.clone(), window, cx))
        })
        .expect("failed to open window");

        // Closing the main window quits the app (also on macOS).
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}

/// Write panics to the error log as well, so crashes of the windowed (no
/// console) release build leave a trace.
fn install_panic_logger() {
    let app_dir = backend::config::app_data_dir();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let logs = backend::logger::LogManager::new(&app_dir);
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        logs.write(
            backend::logger::LogKind::App,
            &format!("panic: {info}"),
            Some(&backtrace),
        );
        default_hook(info);
    }));
}
