use std::sync::Arc;

use tauri::{
    AppHandle, Emitter, Manager,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{
    i18n::{NativeText, text},
    runtime::{AppRuntime, EVENT_NAVIGATE},
};

pub fn install(app: &AppHandle, runtime: Arc<AppRuntime>) -> tauri::Result<()> {
    let menu = build_menu(app, runtime.settings().language)?;

    TrayIconBuilder::with_id("main")
        .icon(tray_image())
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("SyncHalo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
                && is_activation_click(button, button_state)
            {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "pause" => {
                let _ = runtime.pause_sync(!runtime.is_paused());
            }
            "send-file" => {
                show_main_window(app);
                let _ = app.emit(EVENT_NAVIGATE, "files");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn refresh(app: &AppHandle, runtime: &AppRuntime) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(build_menu(app, runtime.settings().language)?))?;
    }
    Ok(())
}

fn build_menu(
    app: &AppHandle,
    language: synchalo_core::LanguagePreference,
) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(
        app,
        "open",
        text(language, NativeText::TrayOpen),
        true,
        None::<&str>,
    )?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        text(language, NativeText::TrayPause),
        true,
        None::<&str>,
    )?;
    let send_file = MenuItem::with_id(
        app,
        "send-file",
        text(language, NativeText::TraySendFile),
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        text(language, NativeText::TrayQuit),
        true,
        None::<&str>,
    )?;
    Menu::with_items(app, &[&open, &pause, &send_file, &separator, &quit])
}

fn show_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.show();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn is_activation_click(button: MouseButton, button_state: MouseButtonState) -> bool {
    button == MouseButton::Left && button_state == MouseButtonState::Up
}

fn tray_image() -> Image<'static> {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let center = (SIZE as f32 - 1.0) / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let ring = (distance > 8.0 && distance < 12.8) || distance < 3.4;
            let alpha = if ring { 255 } else { 0 };
            rgba.extend_from_slice(&[35, 95, 220, alpha]);
        }
    }
    Image::new_owned(rgba, SIZE, SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_completed_left_click_activates_the_window() {
        assert!(is_activation_click(MouseButton::Left, MouseButtonState::Up));
        assert!(!is_activation_click(
            MouseButton::Left,
            MouseButtonState::Down
        ));
        assert!(!is_activation_click(
            MouseButton::Right,
            MouseButtonState::Up
        ));
    }
}
