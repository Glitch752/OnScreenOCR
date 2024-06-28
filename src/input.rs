use std::{sync::{Arc, Mutex}, thread};

use inputbot::KeybdKey::{self, LShiftKey, LAltKey, LControlKey, LSuper};
use winit::event_loop::EventLoop;

use crate::settings::Keybind;

#[derive(Clone, Debug)]
enum KeybindState {
    WaitingForKeybind,
    KeybindEntered(Keybind),
    None
}

pub struct InputHandler {
    keybind_state: Arc<Mutex<KeybindState>>,
}

impl InputHandler {
    pub fn new() -> Self {
        let keybind_state = Arc::new(Mutex::new(KeybindState::None));
        Self {
            keybind_state
        }
    }

    pub fn handle(
        &mut self,
        event_loop: &EventLoop<()>,
        keybind: Arc<Mutex<Keybind>>
    ) {
        let loop_proxy = event_loop.create_proxy();

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

        let keybind_state = self.keybind_state.clone();
        KeybdKey::bind_all_release(move |event| {
            match inputbot::from_keybd_key(event) {
                Some(key) => {
                    let shift = LShiftKey.is_pressed();
                    let alt = LAltKey.is_pressed();
                    let ctrl = LControlKey.is_pressed();
                    let meta = LSuper.is_pressed();
                    let keybind = Keybind {
                        key,
                        shift,
                        alt,
                        ctrl,
                        meta
                    };

                    *keybind_state.lock().expect("Unable to lock keybind callback") = KeybindState::KeybindEntered(keybind);
                }
                _ => {}
            }
        });

        thread::spawn(|| {
            inputbot::handle_input_events(false);
        });
    }

    pub fn detect_next_keybind(&mut self) {
        *self.keybind_state.lock().expect("Unable to lock keybind callback") = KeybindState::WaitingForKeybind;
    }

    pub fn stop_detecting_keybind(&mut self) {
        *self.keybind_state.lock().expect("Unable to lock keybind callback") = KeybindState::None;
    }

    pub fn check_for_detected_keybind(&mut self) -> Option<Keybind> {
        match self.keybind_state.lock().expect("Unable to lock keybind callback").to_owned() {
            KeybindState::KeybindEntered(keybind) => {
                *self.keybind_state.lock().expect("Unable to lock keybind callback") = KeybindState::None;
                Some(keybind)
            }
            _ => None
        }
    }
}