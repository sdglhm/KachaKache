mod app_state;
mod commands;
mod services;
mod types;

use app_state::AppState;
#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use tauri::menu::{
    Menu, MenuItem, PredefinedMenuItem, Submenu,
};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Listener, LogicalPosition, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use types::SettingsPatch;

const OVERLAY_WIDTH: f64 = 188.0;
const OVERLAY_HEIGHT: f64 = 54.0;
const OVERLAY_MARGIN: i32 = 12;
const ABOUT_WIDTH: f64 = 500.0;
const ABOUT_HEIGHT: f64 = 580.0;
const SETUP_WIDTH: f64 = 960.0;
const SETUP_HEIGHT: f64 = 820.0;
const MENU_ID_ABOUT: &str = "about";
const MENU_ID_SETTINGS: &str = "settings";
const MENU_ID_TOGGLE: &str = "toggle";
const MENU_ID_NEW_DICTATION: &str = "new-dictation";
const MENU_ID_QUIT: &str = "quit";

fn toggle_menu_label(is_listening: bool) -> &'static str {
    if is_listening {
        "Stop Narrating"
    } else {
        "Start Narrating"
    }
}

fn tray_toggle_menu_label(is_listening: bool, shortcut: &str) -> String {
    if is_listening {
        "Stop Narrating".to_string()
    } else {
        format!("Start Narrating\t{}", shortcut.replace("Cmd", "⌘"))
    }
}

#[derive(Clone)]
struct ToggleMenuItems {
    app_menu: MenuItem<tauri::Wry>,
    tray_menu: MenuItem<tauri::Wry>,
}

fn set_toggle_menu_labels(toggle_items: &ToggleMenuItems, is_listening: bool, shortcut: &str) {
    let _ = toggle_items.app_menu.set_text(toggle_menu_label(is_listening));
    let _ = toggle_items
        .tray_menu
        .set_text(tray_toggle_menu_label(is_listening, shortcut));
}

fn setup_app_menu(app: &tauri::App) -> tauri::Result<MenuItem<tauri::Wry>> {
    let app_name = app.package_info().name.clone();

    let about_item =
        MenuItem::with_id(app, MENU_ID_ABOUT, format!("About {app_name}"), true, None::<&str>)?;
    let services_item = PredefinedMenuItem::services(app, None)?;
    let hide_item = PredefinedMenuItem::hide(app, None)?;
    let hide_others_item = PredefinedMenuItem::hide_others(app, None)?;
    let show_all_item = PredefinedMenuItem::show_all(app, None)?;
    let quit_item = PredefinedMenuItem::quit(app, None)?;
    let toggle_item = MenuItem::with_id(app, MENU_ID_TOGGLE, toggle_menu_label(false), true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, MENU_ID_SETTINGS, "Settings...", true, Some("Cmd+,"))?;
    let help_settings_item =
        MenuItem::with_id(app, MENU_ID_SETTINGS, "KachaKache Settings", true, None::<&str>)?;

    let app_menu = Submenu::with_items(
        app,
        &app_name,
        true,
        &[
            &about_item,
            &PredefinedMenuItem::separator(app)?,
            &toggle_item,
            &settings_item,
            &PredefinedMenuItem::separator(app)?,
            &services_item,
            &PredefinedMenuItem::separator(app)?,
            &hide_item,
            &hide_others_item,
            &show_all_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &MenuItem::with_id(app, MENU_ID_NEW_DICTATION, "New Dictation", true, Some("Cmd+N"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app, None)?],
    )?;

    let window_menu = Submenu::with_id_and_items(
        app,
        tauri::menu::WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &show_all_item,
        ],
    )?;

    let help_menu = Submenu::with_id_and_items(
        app,
        tauri::menu::HELP_SUBMENU_ID,
        "Help",
        true,
        &[&help_settings_item],
    )?;

    let _ = window_menu.set_as_windows_menu_for_nsapp();
    let _ = help_menu.set_as_help_menu_for_nsapp();

    let menu = Menu::with_items(
        app,
        &[&app_menu, &file_menu, &edit_menu, &view_menu, &window_menu, &help_menu],
    )?;
    let _ = app.set_menu(menu)?;

    Ok(toggle_item)
}

fn setup_tray(app: &tauri::App, shortcut: &str) -> tauri::Result<MenuItem<tauri::Wry>> {
    let settings = MenuItem::with_id(app, MENU_ID_SETTINGS, "Settings...", true, None::<&str>)?;
    let toggle = MenuItem::with_id(
        app,
        MENU_ID_TOGGLE,
        tray_toggle_menu_label(false, shortcut),
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, MENU_ID_QUIT, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &settings, &quit])?;

    let mut tray_builder = TrayIconBuilder::with_id("kachakache-tray")
        .menu(&menu)
        .tooltip("KachaKache")
        .icon_as_template(false)
        .show_menu_on_left_click(true);

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    let _ = tray_builder.build(app)?;

    Ok(toggle)
}

fn create_overlay_window(app: &tauri::App) -> tauri::Result<()> {
    if app.get_webview_window("overlay").is_some() {
        return Ok(());
    }

    let overlay = WebviewWindowBuilder::new(
        app,
        "overlay",
        WebviewUrl::App("index.html?overlay=1".into()),
    )
    .title("KachaKache Overlay")
    .decorations(false)
    .transparent(true)
    .visible(false)
    .resizable(false)
    .focused(false)
    .focusable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
    .build()?;

    let _ = overlay.set_always_on_top(true);
    let _ = overlay.set_focusable(false);
    let _ = overlay.set_ignore_cursor_events(true);
    let _ = overlay.set_shadow(false);
    position_overlay_bottom_right(&overlay);
    Ok(())
}

fn show_about_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("about") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let hide_dock_icon = tauri::async_runtime::block_on(app.state::<AppState>().current_settings())
            .hide_dock_icon;
        let _ = sync_dock_icon_visibility(app, hide_dock_icon);
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, "about", WebviewUrl::App("index.html?about=1".into()))
        .title("About KachaKache")
        .inner_size(ABOUT_WIDTH, ABOUT_HEIGHT)
        .minimizable(false)
        .maximizable(false)
        .resizable(false)
        .center()
        .visible(true)
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .traffic_light_position(LogicalPosition::new(18.0, 24.0))
        .build()?;

    let _ = window.set_focus();
    let hide_dock_icon = tauri::async_runtime::block_on(app.state::<AppState>().current_settings())
        .hide_dock_icon;
    let _ = sync_dock_icon_visibility(app, hide_dock_icon);
    Ok(())
}

pub fn show_setup_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("setup") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let hide_dock_icon = tauri::async_runtime::block_on(app.state::<AppState>().current_settings())
            .hide_dock_icon;
        let _ = sync_dock_icon_visibility(app, hide_dock_icon);
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, "setup", WebviewUrl::App("index.html?setup=1".into()))
        .title("Set up KachaKache")
        .inner_size(SETUP_WIDTH, SETUP_HEIGHT)
        .minimizable(false)
        .maximizable(false)
        .resizable(false)
        .center()
        .visible(true)
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .traffic_light_position(LogicalPosition::new(22.0, 28.0))
        .build()?;

    let _ = window.set_focus();
    let hide_dock_icon = tauri::async_runtime::block_on(app.state::<AppState>().current_settings())
        .hide_dock_icon;
    let _ = sync_dock_icon_visibility(app, hide_dock_icon);
    Ok(())
}

pub fn close_setup_window(app: &tauri::AppHandle, focus_main: bool) {
    if let Some(window) = app.get_webview_window("setup") {
        let _ = window.close();
    }

    if focus_main {
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.show();
            let _ = main.unminimize();
            let _ = main.set_focus();
        }
    }

    let hide_dock_icon = tauri::async_runtime::block_on(app.state::<AppState>().current_settings())
        .hide_dock_icon;
    let _ = sync_dock_icon_visibility(app, hide_dock_icon);
}

#[cfg(target_os = "macos")]
fn any_primary_window_visible(app: &AppHandle) -> bool {
    ["main", "setup", "about"]
        .iter()
        .filter_map(|label| app.get_webview_window(label))
        .any(|window| window.is_visible().unwrap_or(false))
}

pub fn sync_dock_icon_visibility(app: &AppHandle, hide_dock_icon: bool) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let should_hide = hide_dock_icon && !any_primary_window_visible(app);
        app.run_on_main_thread(move || {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let app = NSApplication::sharedApplication(mtm);
            let policy = if should_hide {
                NSApplicationActivationPolicy::Accessory
            } else {
                NSApplicationActivationPolicy::Regular
            };
            let _ = app.setActivationPolicy(policy);
        })?;
    }

    Ok(())
}

fn position_overlay_bottom_right(overlay: &WebviewWindow) {
    let monitor = overlay
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| overlay.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };

    let Ok(size) = overlay.outer_size() else {
        return;
    };
    let work_area = monitor.work_area();
    let margin = OVERLAY_MARGIN;

    let x = work_area.position.x + work_area.size.width as i32 - size.width as i32 - margin;
    let y = work_area.position.y + work_area.size.height as i32 - size.height as i32 - margin;
    let _ = overlay.set_position(PhysicalPosition::new(x, y));
}

fn register_menu_handler(app: &tauri::App, toggle_items: ToggleMenuItems) {
    app.on_menu_event(move |app, event| match event.id().as_ref() {
        MENU_ID_ABOUT => {
            let _ = show_about_window(&app.clone());
        }
        MENU_ID_SETTINGS => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            let hide_dock_icon =
                tauri::async_runtime::block_on(app.state::<AppState>().current_settings())
                    .hide_dock_icon;
            let _ = sync_dock_icon_visibility(app, hide_dock_icon);
        }
        MENU_ID_TOGGLE => {
            let state = app.state::<AppState>().inner().clone();
            let app_handle = app.clone();
            let toggle_items = toggle_items.clone();
            tauri::async_runtime::spawn(async move {
                if state.dictation.is_listening() {
                    let _ = state.stop_dictation(&app_handle).await;
                } else {
                    let _ = state.start_dictation(&app_handle).await;
                }
                let shortcut = state.current_settings().await.shortcut;
                set_toggle_menu_labels(&toggle_items, state.dictation.is_listening(), &shortcut);
            });
        }
        MENU_ID_NEW_DICTATION => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            let hide_dock_icon =
                tauri::async_runtime::block_on(app.state::<AppState>().current_settings())
                    .hide_dock_icon;
            let _ = sync_dock_icon_visibility(app, hide_dock_icon);
            let state = app.state::<AppState>().inner().clone();
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if !state.dictation.is_listening() {
                    let _ = state.start_dictation(&app_handle).await;
                }
            });
        }
        MENU_ID_QUIT => {
            app.exit(0);
        }
        _ => {}
    });
}

fn setup_tray_menu_state_listener(app: &tauri::AppHandle, toggle_items: ToggleMenuItems) {
    let app_handle = app.clone();
    let state_toggle_items = toggle_items.clone();
    app.listen("dictation://state-changed", move |event| {
        let payload = event.payload().to_string();
        let is_listening = payload.contains("listening");
        let app_handle = app_handle.clone();
        let toggle_items = state_toggle_items.clone();
        tauri::async_runtime::spawn(async move {
            let shortcut = app_handle.state::<AppState>().current_settings().await.shortcut;
            set_toggle_menu_labels(&toggle_items, is_listening, &shortcut);
        });
    });

    let app_handle = app.clone();
    let shortcut_toggle_items = toggle_items.clone();
    app.listen("settings://shortcut-changed", move |event| {
        let shortcut = event.payload().trim_matches('"').to_string();
        let app_handle = app_handle.clone();
        let toggle_items = shortcut_toggle_items.clone();
        tauri::async_runtime::spawn(async move {
            let is_listening = app_handle.state::<AppState>().dictation.is_listening();
            set_toggle_menu_labels(&toggle_items, is_listening, &shortcut);
        });
    });
}

fn setup_overlay_visibility_listener(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    app.listen("dictation://state-changed", move |event| {
        let payload = event.payload().to_string();
        let handle = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let state = handle.state::<AppState>().inner().clone();
            let overlay_enabled = state.current_settings().await.overlay_enabled;
            if let Some(overlay) = handle.get_webview_window("overlay") {
                if !overlay_enabled {
                    let _ = overlay.set_ignore_cursor_events(true);
                    let _ = overlay.hide();
                    return;
                }

                let is_listening = payload.contains("listening");
                let should_show = is_listening
                    || payload.contains("processing")
                    || payload.contains("done");
                if should_show {
                    let _ = overlay.set_ignore_cursor_events(!is_listening);
                    position_overlay_bottom_right(&overlay);
                    let _ = overlay.show();
                } else {
                    let _ = overlay.set_ignore_cursor_events(true);
                    let _ = overlay.hide();
                }
            }
        });
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            create_overlay_window(app)?;
            let app_toggle_item = setup_app_menu(app)?;
            let app_handle: AppHandle = app.handle().clone();
            let state = tauri::async_runtime::block_on(AppState::new(&app_handle))?;
            let shortcut = tauri::async_runtime::block_on(state.current_settings()).shortcut;
            app.manage(state.clone());

            let tray_toggle_item = setup_tray(app, &shortcut)?;
            let toggle_items = ToggleMenuItems {
                app_menu: app_toggle_item,
                tray_menu: tray_toggle_item,
            };
            register_menu_handler(app, toggle_items.clone());

            if let Err(err) = tauri::async_runtime::block_on(
                state.register_shortcut(&app_handle, shortcut.clone()),
            ) {
                eprintln!("failed to register saved shortcut `{shortcut}`: {err:#}");
                let fallback = crate::types::Settings::default().shortcut;
                tauri::async_runtime::block_on(
                    state.register_shortcut(&app_handle, fallback.clone()),
                )?;
                let _ = tauri::async_runtime::block_on(state.update_settings(SettingsPatch {
                    shortcut: Some(fallback),
                    ..Default::default()
                }));
            }
            let current_shortcut = tauri::async_runtime::block_on(state.current_settings()).shortcut;
            set_toggle_menu_labels(&toggle_items, state.dictation.is_listening(), &current_shortcut);
            setup_tray_menu_state_listener(&app_handle, toggle_items);
            setup_overlay_visibility_listener(&app_handle);

            let initial_settings = tauri::async_runtime::block_on(state.current_settings());
            let onboarding_completed = initial_settings.onboarding_completed;
            let hide_dock_icon = initial_settings.hide_dock_icon;
            let _ = sync_dock_icon_visibility(&app_handle, hide_dock_icon);
            if !onboarding_completed {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.hide();
                }
                let _ = show_setup_window(&app_handle);
                let _ = sync_dock_icon_visibility(&app_handle, hide_dock_icon);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    let hide_dock_icon = tauri::async_runtime::block_on(
                        window.app_handle().state::<AppState>().current_settings(),
                    )
                    .hide_dock_icon;
                    let _ = sync_dock_icon_visibility(&window.app_handle(), hide_dock_icon);
                }
            } else if window.label() == "setup" {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    if let Some(main) = window.app_handle().get_webview_window("main") {
                        let _ = main.show();
                        let _ = main.unminimize();
                    }
                    let hide_dock_icon = tauri::async_runtime::block_on(
                        window.app_handle().state::<AppState>().current_settings(),
                    )
                    .hide_dock_icon;
                    let _ = sync_dock_icon_visibility(&window.app_handle(), hide_dock_icon);
                }
            } else if window.label() == "about" {
                if let tauri::WindowEvent::Destroyed = event {
                    let hide_dock_icon = tauri::async_runtime::block_on(
                        window.app_handle().state::<AppState>().current_settings(),
                    )
                    .hide_dock_icon;
                    let _ = sync_dock_icon_visibility(&window.app_handle(), hide_dock_icon);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap_state,
            commands::list_microphones,
            commands::start_dictation,
            commands::stop_dictation,
            commands::get_settings,
            commands::update_settings,
            commands::list_recommended_models,
            commands::list_installed_models,
            commands::download_model,
            commands::cancel_model_download,
            commands::set_active_model,
            commands::delete_model,
            commands::get_permissions_status,
            commands::request_permission,
            commands::open_permission_settings,
            commands::get_history,
            commands::clear_history,
            commands::delete_history_entry,
            commands::copy_history_entry,
            commands::open_setup_window,
            commands::complete_setup_flow,
            commands::dismiss_setup_flow,
            commands::is_debug_build
        ])
        .run(tauri::generate_context!())
        .expect("error while running KachaKache");
}
