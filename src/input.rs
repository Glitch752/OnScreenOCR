use std::{sync::{Arc, Mutex}, thread};

use inputbot::{get_keybd_key, KeybdKey::{self, LAltKey, LControlKey, LShiftKey, LSuper}};
use tray_item::{IconSource, TrayItem};
use winit::{event::KeyEvent, event_loop::EventLoop, platform::modifier_supplement::KeyEventExtModifierSupplement};

use crate::settings::Keybind;

#[derive(Clone, Debug)]
enum KeybindState {
    WaitingForKeybind,
    KeybindEntered(Keybind),
    None
}

impl PartialEq for KeybindState {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (KeybindState::WaitingForKeybind, KeybindState::WaitingForKeybind) => true,
            (KeybindState::KeybindEntered(_), KeybindState::KeybindEntered(_)) => true,
            (KeybindState::None, KeybindState::None) => true,
            _ => false
        }
    }

}

pub struct InputHandler {
    keybind_state: KeybindState,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            keybind_state: KeybindState::None
        }
    }

    pub fn handle(
        &mut self,
        event_loop: &EventLoop<()>,
        current_keybind: Arc<Mutex<Keybind>>
    ) {
        let loop_proxy = event_loop.create_proxy();
        let loop_proxy_2 = event_loop.create_proxy();

        println!("Listening with initial keybind {}", current_keybind.lock().expect("Unable to lock keybind").to_owned().to_string());

        KeybdKey::bind_all(move |event| {
            match inputbot::from_keybd_key(event) {
                Some(key) => {
                    let current_keybind = current_keybind.lock().expect("Unable to lock keybind").to_owned();
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

        let mut tray = TrayItem::new(
            "OnScreenOCR",
            IconSource::Resource("tray-default"),
        ).unwrap();

        tray.add_menu_item("Open overlay", move || {
            loop_proxy_2.send_event(()).expect("Unable to send event");
        }).unwrap();

        tray.add_menu_item("Quit", || {
            std::process::exit(0);
        }).unwrap();

        thread::spawn(|| {
            inputbot::handle_input_events(false);
        });
    }

    pub fn detect_next_keybind(&mut self) {
        self.keybind_state = KeybindState::WaitingForKeybind;
    }

    pub fn stop_detecting_keybind(&mut self) {
        self.keybind_state = KeybindState::None;
    }

    pub fn check_for_detected_keybind(&mut self) -> Option<Keybind> {
        match self.keybind_state {
            KeybindState::KeybindEntered(keybind) => {
                self.keybind_state = KeybindState::None;
                Some(keybind)
            }
            _ => None
        }
    }

    pub fn keyboard_event(&mut self, event: &KeyEvent) -> bool {
        if self.keybind_state == KeybindState::WaitingForKeybind {
            let without_modifiers = event.key_without_modifiers();

            let shift = LShiftKey.is_pressed();
            let alt = LAltKey.is_pressed();
            let ctrl = LControlKey.is_pressed();
            let meta = LSuper.is_pressed();

            if !shift && !alt && !ctrl && !meta {
                return false;
            }

            let keybind = Keybind {
                key: without_modifiers.to_text().unwrap_or_default().chars().next().unwrap_or_default(),
                shift,
                alt,
                ctrl,
                meta
            };

            if !get_keybd_key(keybind.key).is_some() {
                return false;
            }

            self.keybind_state = KeybindState::KeybindEntered(keybind);

            return true;
        }

        false
    }
}