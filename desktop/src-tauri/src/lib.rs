mod artwork;
mod lyrics;
mod media;
mod settings;

use lyrics::LyricsPayload;
use media::{Command, Snapshot};
use serde::Serialize;
use settings::Settings;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};

const FLOATING: &str = "floating";
const FULLSCREEN: &str = "fullscreen";
const SETTINGS: &str = "settings";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtworkPayload {
    track_id: String,
    data_url: Option<String>,
}

#[derive(Default)]
struct Shared {
    now: Option<Snapshot>,
    artwork: Option<ArtworkPayload>,
    lyrics: Option<LyricsPayload>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatePayload {
    now_playing: Option<Snapshot>,
    artwork: Option<ArtworkPayload>,
    lyrics: Option<LyricsPayload>,
    settings: Settings,
    platform: &'static str,
}

struct TrayItems {
    now_playing: MenuItem<Wry>,
    floating: CheckMenuItem<Wry>,
    click_through: CheckMenuItem<Wry>,
}

struct AppState {
    shared: Arc<Mutex<Shared>>,
    commands: Mutex<mpsc::Sender<Command>>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    tray: Mutex<Option<TrayItems>>,
}

fn platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "macos"
    }
}

// MARK: - Commands

#[tauri::command]
fn get_state(state: tauri::State<AppState>) -> StatePayload {
    let shared = state.shared.lock().unwrap();
    StatePayload {
        now_playing: shared.now.clone(),
        artwork: shared.artwork.clone(),
        lyrics: shared.lyrics.clone(),
        settings: state.settings.lock().unwrap().clone(),
        platform: platform(),
    }
}

#[tauri::command]
fn update_settings(app: AppHandle, state: tauri::State<AppState>, patch: serde_json::Value) -> Settings {
    let updated = {
        let mut settings = state.settings.lock().unwrap();
        *settings = settings.merged(patch);
        settings.save(&state.settings_path);
        settings.clone()
    };
    apply_settings(&app, &updated);
    let _ = app.emit("settings-changed", &updated);
    updated
}

#[tauri::command]
fn media_command(state: tauri::State<AppState>, action: String, position: Option<f64>) {
    let command = match action.as_str() {
        "playPause" => Command::PlayPause,
        "next" => Command::Next,
        "previous" => Command::Previous,
        "seek" => Command::Seek(position.unwrap_or(0.0)),
        _ => return,
    };
    let _ = state.commands.lock().unwrap().send(command);
}

// Commands that create or destroy windows must be async. Synchronous commands
// run on the main thread inside WebView2's message callback, and building a
// webview there deadlocks on Windows, which froze the app on "Full Screen".

#[tauri::command]
async fn show_fullscreen(app: AppHandle) {
    open_fullscreen(&app);
}

#[tauri::command]
async fn hide_fullscreen(app: AppHandle) {
    close_fullscreen(&app);
}

#[tauri::command]
async fn set_floating_visible(app: AppHandle, visible: bool) {
    set_floating(&app, visible);
}

#[tauri::command]
async fn open_settings(app: AppHandle) {
    open_settings_window(&app);
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    let _ = if enabled { launcher.enable() } else { launcher.disable() };
    launcher.is_enabled().unwrap_or(false)
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

// MARK: - Windows

fn floating_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(window) = app.get_webview_window(FLOATING) {
        return Some(window);
    }
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    let (min_w, min_h) = settings.floating_min_size();
    let window = WebviewWindowBuilder::new(app, FLOATING, WebviewUrl::App("floating.html".into()))
        .title("Lyrical")
        .inner_size(640.0, min_h.max(176.0))
        .min_inner_size(min_w, min_h)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .resizable(true)
        .visible(false)
        .build()
        .ok()?;
    position_default(&window);
    apply_settings(app, &settings);
    Some(window)
}

/// Bottom-centre of the primary display, above the taskbar, on first launch.
/// The window-state plugin restores the user's position after that.
fn position_default(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else { return };
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let origin = monitor.position().to_logical::<f64>(scale);
    let Ok(outer) = window.outer_size() else { return };
    let outer = outer.to_logical::<f64>(scale);
    let x = origin.x + (size.width - outer.width) / 2.0;
    let y = origin.y + size.height - outer.height - 96.0;
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
}

fn set_floating(app: &AppHandle, visible: bool) {
    let state = app.state::<AppState>();
    {
        let mut settings = state.settings.lock().unwrap();
        settings.floating_visible = visible;
        settings.save(&state.settings_path);
    }
    if let Some(items) = state.tray.lock().unwrap().as_ref() {
        let _ = items.floating.set_checked(visible);
    }
    let fullscreen_open = app.get_webview_window(FULLSCREEN).is_some();
    if let Some(window) = floating_window(app) {
        let _ = if visible && !fullscreen_open { window.show() } else { window.hide() };
    }
    let settings = state.settings.lock().unwrap().clone();
    let _ = app.emit("settings-changed", &settings);
}

fn apply_settings(app: &AppHandle, settings: &Settings) {
    if let Some(items) = app.state::<AppState>().tray.lock().unwrap().as_ref() {
        let _ = items.click_through.set_checked(settings.click_through);
        let _ = items.floating.set_checked(settings.floating_visible);
    }
    let Some(window) = app.get_webview_window(FLOATING) else { return };
    let _ = window.set_ignore_cursor_events(settings.click_through);
    let (min_w, min_h) = settings.floating_min_size();
    let _ = window.set_min_size(Some(LogicalSize::new(min_w, min_h)));
    if let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) {
        let size = size.to_logical::<f64>(scale);
        if size.width < min_w || size.height < min_h {
            let _ = window.set_size(LogicalSize::new(size.width.max(min_w), size.height.max(min_h)));
        }
    }
    apply_glass(&window, &settings.glass);
}

#[cfg(windows)]
fn apply_glass(window: &WebviewWindow, glass: &str) {
    use window_vibrancy::{apply_acrylic, clear_acrylic};
    if glass == "acrylic" {
        // A light, slightly cool tint so the desktop shows through, like Liquid Glass.
        let _ = apply_acrylic(window, Some((24, 24, 32, 64)));
        let _ = window.set_shadow(true);
        round_corners(window);
    } else {
        let _ = clear_acrylic(window);
        let _ = window.set_shadow(false);
    }
}

#[cfg(not(windows))]
fn apply_glass(_window: &WebviewWindow, _glass: &str) {}

/// Asks Windows 11 to round the borderless window's corners like other windows.
/// (A custom SetWindowRgn clip would allow a larger radius, but Windows then
/// stops compositing the frame and paints a classic title bar over the box.)
#[cfg(windows)]
fn round_corners(window: &WebviewWindow) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    /// DWMWA_COLOR_NONE: no accent-coloured border; the page draws a glass rim instead.
    const COLOR_NONE: u32 = 0xFFFF_FFFE;
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else { return };
    let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
    let preference = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&preference) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &COLOR_NONE as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&COLOR_NONE) as u32,
        );
    }
}

fn open_fullscreen(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(FULLSCREEN) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let monitor = app
        .get_webview_window(FLOATING)
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let mut builder = WebviewWindowBuilder::new(app, FULLSCREEN, WebviewUrl::App("fullscreen.html".into()))
        .title("Lyrical")
        .decorations(false)
        .background_color(tauri::window::Color(0, 0, 0, 255))
        .focused(true)
        .visible(false);
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let position = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        builder = builder.position(position.x, position.y).inner_size(size.width, size.height);
    }
    if let Ok(window) = builder.build() {
        let _ = window.set_fullscreen(true);
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Some(floating) = app.get_webview_window(FLOATING) {
        let _ = floating.hide();
    }
}

fn close_fullscreen(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(FULLSCREEN) {
        let _ = window.destroy();
    }
    let visible = app.state::<AppState>().settings.lock().unwrap().floating_visible;
    if visible {
        if let Some(floating) = floating_window(app) {
            let _ = floating.show();
        }
    }
}

fn open_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("Lyrical Settings")
        .inner_size(560.0, 720.0)
        .min_inner_size(480.0, 520.0)
        .background_color(tauri::window::Color(22, 22, 28, 255))
        .center()
        .build();
}

// MARK: - Tray

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    let now_playing = MenuItem::with_id(app, "now", "Nothing playing", false, None::<&str>)?;
    let floating = CheckMenuItem::with_id(app, "floating", "Floating Lyrics", true, settings.floating_visible, None::<&str>)?;
    let fullscreen = MenuItem::with_id(app, "fullscreen", "Full Screen Lyrics", true, None::<&str>)?;
    let click_through =
        CheckMenuItem::with_id(app, "click-through", "Click-Through Floating Lyrics", true, settings.click_through, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Lyrical", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &now_playing,
            &PredefinedMenuItem::separator(app)?,
            &floating,
            &fullscreen,
            &click_through,
            &PredefinedMenuItem::separator(app)?,
            &settings_item,
            &quit,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id("lyrical")
        .tooltip("Lyrical")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "floating" => {
                let visible = !app.state::<AppState>().settings.lock().unwrap().floating_visible;
                set_floating(app, visible);
            }
            "fullscreen" => open_fullscreen(app),
            "click-through" => {
                let state = app.state::<AppState>();
                let enabled = !state.settings.lock().unwrap().click_through;
                update_settings(app.clone(), state, serde_json::json!({ "clickThrough": enabled }));
            }
            "settings" => open_settings_window(app),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;

    *app.state::<AppState>().tray.lock().unwrap() = Some(TrayItems { now_playing, floating, click_through });
    Ok(())
}

// MARK: - Polling

fn spawn_poller(app: AppHandle, shared: Arc<Mutex<Shared>>, commands: mpsc::Receiver<Command>) {
    std::thread::spawn(move || {
        media::init_thread();
        let mut backend = media::backend();
        let mut last_id: Option<String> = None;
        loop {
            match commands.recv_timeout(Duration::from_millis(500)) {
                Ok(command) => {
                    backend.send(command);
                    // Give the player a moment to apply it before re-reading.
                    std::thread::sleep(Duration::from_millis(120));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            let snapshot = backend.snapshot();
            let id = snapshot.as_ref().map(|s| s.track_id.clone());
            // Record the new track before starting its lookups: they check it to
            // drop stale results, and a cache hit can finish before we'd get here.
            shared.lock().unwrap().now = snapshot.clone();
            if id != last_id {
                last_id = id.clone();
                on_track_change(&app, &shared, snapshot.clone(), backend.artwork());
            }
            let _ = app.emit("now-playing", &snapshot);
        }
    });
}

fn on_track_change(app: &AppHandle, shared: &Arc<Mutex<Shared>>, snapshot: Option<Snapshot>, art: Option<media::Artwork>) {
    let label = snapshot
        .as_ref()
        .map(|s| if s.artist.is_empty() { s.title.clone() } else { format!("{} — {}", s.title, s.artist) })
        .unwrap_or_else(|| "Nothing playing".into());
    if let Some(items) = app.state::<AppState>().tray.lock().unwrap().as_ref() {
        let _ = items.now_playing.set_text(truncate(&label, 48));
    }

    let Some(track) = snapshot else {
        let mut state = shared.lock().unwrap();
        state.artwork = None;
        state.lyrics = None;
        let _ = app.emit("artwork", None::<ArtworkPayload>);
        let _ = app.emit("lyrics", None::<LyricsPayload>);
        return;
    };

    {
        let mut state = shared.lock().unwrap();
        state.artwork = Some(ArtworkPayload { track_id: track.track_id.clone(), data_url: None });
        state.lyrics = Some(LyricsPayload::loading(&track.track_id));
        let _ = app.emit("artwork", &state.artwork);
        let _ = app.emit("lyrics", &state.lyrics);
    }

    let is_current = |shared: &Arc<Mutex<Shared>>, id: &str| {
        shared.lock().unwrap().now.as_ref().map(|n| n.track_id.as_str()) == Some(id)
    };

    let (app_art, shared_art, track_art) = (app.clone(), shared.clone(), track.clone());
    tauri::async_runtime::spawn(async move {
        let data_url = artwork::resolve(art, &track_art.title, &track_art.artist).await;
        if !is_current(&shared_art, &track_art.track_id) {
            return;
        }
        let payload = ArtworkPayload { track_id: track_art.track_id.clone(), data_url };
        shared_art.lock().unwrap().artwork = Some(payload.clone());
        let _ = app_art.emit("artwork", &payload);
    });

    let (app_lyrics, shared_lyrics) = (app.clone(), shared.clone());
    tauri::async_runtime::spawn(async move {
        let payload = lyrics::fetch(&track).await;
        if !is_current(&shared_lyrics, &track.track_id) {
            return;
        }
        shared_lyrics.lock().unwrap().lyrics = Some(payload.clone());
        let _ = app_lyrics.emit("lyrics", &payload);
    });
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max - 1).collect::<String>() + "…"
    }
}

// MARK: - Entry point

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_window_state::StateFlags;

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| open_settings_window(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION | StateFlags::SIZE)
                .with_denylist(&[FULLSCREEN, SETTINGS])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_state,
            update_settings,
            media_command,
            show_fullscreen,
            hide_fullscreen,
            set_floating_visible,
            open_settings,
            set_autostart,
            get_autostart,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&settings_path);
            let (sender, receiver) = mpsc::channel();
            let shared = Arc::new(Mutex::new(Shared::default()));
            app.manage(AppState {
                shared: shared.clone(),
                commands: Mutex::new(sender),
                settings: Mutex::new(settings.clone()),
                settings_path,
                tray: Mutex::new(None),
            });

            let handle = app.handle().clone();
            build_tray(&handle)?;
            if let Some(window) = floating_window(&handle) {
                if settings.floating_visible {
                    let _ = window.show();
                }
            }
            spawn_poller(handle, shared, receiver);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Lyrical")
        .run(|_app, event| {
            // Closing windows shouldn't quit a tray app.
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}
