use std::thread;

use inputbot::KeybdKey::{self, LShiftKey, LAltKey, LControlKey, LSuper};
use winit::event_loop::EventLoop;

use crate::App;

pub fn handle(event_loop: &EventLoop<()>, app: &mut App) {
    let loop_proxy = event_loop.create_proxy();
        
    let keybind = app.icon_context.settings.open_keybind.clone();
    KeybdKey::bind_all(move |event| {
        match inputbot::from_keybd_key(event) {
            Some(key) => {
                let current_keybind = keybind.lock().expect("Unable to lock keybind").to_owned();
                let shift_matches = LShiftKey.is_pressed() == current_keybind.shift;
                let alt_matches = LAltKey.is_pressed() == current_keybind.alt;
                let control_matches = LControlKey.is_pressed() == current_keybind.ctrl;
                let meta_matches = LSuper.is_pressed() == current_keybind.meta;
                let key_matches = key == current_keybind.key;
                if shift_matches && alt_matches && control_matches && meta_matches && key_matches {
                    // We need to open the window on the main thread
                    loop_proxy.send_event(()).expect("Unable to send event");
                }
            }
            _ => {}
        }
    });

    thread::spawn(|| {
        inputbot::handle_input_events();
    });
}