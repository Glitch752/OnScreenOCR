#![windows_subsystem = "windows"]

#![feature(duration_millis_float)]
#![feature(fs_try_exists)]

use clipboard::{ClipboardContext, ClipboardProvider};
use clipboard_image::copy_image_to_clipboard;
use input::InputHandler;
use inputbot::MouseCursor;
use ocr_handler::{FormatOptions, OCRHandler, get_screenshot_path};
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use screenshot::{crop_screenshot_to_bounds, crop_screenshot_to_polygon, screenshot_from_handle, Screenshot};
use selection::{Selection, SelectionInputResult};
use undo_stack::UndoStack;
use windows_sys::Win32::Foundation::HWND;
use std::sync::{mpsc, Arc, LazyLock, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Cursor, CursorIcon, Fullscreen, Window, WindowId, WindowLevel};
use renderer::{IconContext, IconEvent};

mod ocr_handler;
mod renderer;
mod screenshot;
mod selection;
mod wgpu_text;
mod settings;
mod clipboard_image;
mod undo_stack;
mod input;

pub static INITIALIZATION_ERRORS: LazyLock<Arc<Mutex<Vec<String>>>> = LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

fn main() {
    // Only run event loop on user interaction
    let event_loop = EventLoop::new().expect("Unable to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    
    let loop_proxy: winit::event_loop::EventLoopProxy<()> = event_loop.create_proxy();
    let mut tray = tray_item::TrayItem::new(
        "OnScreenOCR",
        tray_item::IconSource::Resource("tray-default"),
    ).unwrap();
    tray.add_menu_item("Open overlay", move || {
        loop_proxy.send_event(()).expect("Unable to send event");
    }).unwrap();
    tray.inner_mut().add_separator().unwrap();
    tray.add_menu_item("Quit", || {
        std::process::exit(0);
    }).unwrap();

    let mut app = App::default();
    let keybind = app.icon_context.settings.open_keybind.clone();
    app.input_handler.handle(&event_loop, keybind);
    event_loop.run_app(&mut app).expect("Unable to run event loop");
}

struct WindowState {
    window: Window,
    pixels: Pixels,
    shader_renderer: renderer::Renderer,
    
    #[cfg(windows)]
    focus_before_overlay: HWND
}

struct App {
    window_state: Option<WindowState>,
    size: (u32, u32),
    selection: Selection,
    ocr_handler: OCRHandler,
    relative_mouse_pos: (i32, i32),

    icon_context: IconContext,
    icon_event_receiver: mpsc::Receiver<IconEvent>,

    input_handler: InputHandler,

    undo_stack: UndoStack,
    attempt_copy_on_next_ocr: bool,

    user_feedback_queue: Vec<(String, [f32; 3])>
}

impl Default for App {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();

        let icon_context = IconContext::new(tx);
        let icon_event_receiver = rx;

        App {
            window_state: None,
            size: (0, 0),
            selection: Selection::default(),
            ocr_handler: OCRHandler::new(FormatOptions::from_settings(&icon_context.settings)),
            relative_mouse_pos: (0, 0),
            
            icon_context: icon_context,
            icon_event_receiver,

            input_handler: InputHandler::new(),

            undo_stack: UndoStack::new(),
            attempt_copy_on_next_ocr: false,

            user_feedback_queue: INITIALIZATION_ERRORS.lock().unwrap().iter().map(|err| (err.clone(), [0.8, 0.3, 0.4])).collect()
        }
    }

}

impl App {
    fn set_mouse_cursor(&self) {
        let window = &self.window_state.as_ref().unwrap().window;
        let cursor = match (self.selection.shift_held, self.selection.mouse_down) {
            (true, true) => CursorIcon::Grabbing,
            (true, false) => CursorIcon::Grab,
            (false, true) => CursorIcon::Crosshair,
            (false, false) => CursorIcon::Default,
        };
        window.set_cursor(Cursor::from(cursor));
    }

    fn redraw(&mut self) {
        self.process_icon_events();

        self.set_mouse_cursor();

        let updated = self.ocr_handler.update_ocr_preview_text();
        if updated && self.attempt_copy_on_next_ocr {
            self.attempt_copy_on_next_ocr = false;
            self.attempt_copy();
        }

        let state = self.window_state.as_mut().unwrap();

        let pixels = &state.pixels;
        let shader_renderer = &mut state.shader_renderer;

        if self.user_feedback_queue.len() > 0 {
            let (message, color) = self.user_feedback_queue.remove(0);
            shader_renderer.show_user_feedback(message, color);
        }

        self.icon_context.has_selection = self.selection.bounds.width != 0 && self.selection.bounds.height != 0;

        match self.input_handler.check_for_detected_keybind() {
            Some(keybind) => {
                *self.icon_context.settings.open_keybind.lock().expect("Unable to lock keybind") = keybind;
                self.icon_context.settings.open_keybind_string = keybind.to_string();
                self.icon_context.settings.save();
            }
            None => ()
        }

        let render_result = pixels.render_with(|encoder, render_target, context| {
            shader_renderer.update(
                context,
                self.size,
                &mut self.selection,
                self.ocr_handler.ocr_preview_text.clone(),
                self.relative_mouse_pos,
                &mut self.icon_context
            );

            shader_renderer.render(
                encoder,
                render_target,
                context.scaling_renderer.clip_rect(),
            );

            Ok(())
        });

        if render_result.is_err() {
            println!("Error rendering: {:?}", render_result);
        }
    }

    fn process_icon_events(&mut self) {
        let rx = &self.icon_event_receiver;

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        for event in events {
            match event {
                IconEvent::Close => {
                    self.hide_window();
                }
                IconEvent::Copy => {
                    self.attempt_copy();
                }
                IconEvent::Screenshot => {
                    self.attempt_screenshot();
                }
                IconEvent::ActiveOCRLeft => {
                    self.icon_context.settings.tesseract_settings.ocr_language_decrement();
                    self.ocr_handler.update_ocr_settings(self.icon_context.settings.tesseract_settings.clone());
                }
                IconEvent::ActiveOCRRight => {
                    self.icon_context.settings.tesseract_settings.ocr_language_increment();
                    self.ocr_handler.update_ocr_settings(self.icon_context.settings.tesseract_settings.clone());
                }
                IconEvent::UpdateOCRFormatOption => {
                    self.ocr_handler.format_option_changed(FormatOptions::from_settings(&self.icon_context.settings));
                }
                IconEvent::OpenOCRConfiguration => {
                    #[cfg(windows)]
                    {
                        let _ = std::process::Command::new("notepad")
                            .arg(self.icon_context.settings.tesseract_settings.absolute_path())
                            .spawn();
                    }
                    
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(self.icon_context.settings.tesseract_settings.absolute_path())
                            .spawn();
                    }

                    #[cfg(not(any(windows, target_os = "linux")))]
                    {
                        eprintln!("Opening the OCR configuration is not supported on this platform");
                    }

                    self.hide_window();
                }
                IconEvent::RefreshOCRConfiguration => {
                    INITIALIZATION_ERRORS.lock().unwrap().clear();
                    self.icon_context.settings.tesseract_settings.reload();
                    for error in INITIALIZATION_ERRORS.lock().unwrap().iter() {
                        self.show_negative_feedback(&error);
                    }

                    self.ocr_handler.update_ocr_settings(self.icon_context.settings.tesseract_settings.clone());
                }
                IconEvent::ChangeUsePolygon => {
                    self.selection.change_use_polygon(self.icon_context.settings.use_polygon);
                    self.ocr_handler.selection_changed(&self.selection);
                }
                IconEvent::ChangeKeybind => {
                    self.icon_context.settings.open_keybind_string = "Press a key combination".to_string();
                    self.input_handler.detect_next_keybind();
                }
            }
        }
    }

    fn auto_copy(&mut self) {
        if self.icon_context.settings.auto_copy {
            self.attempt_copy_on_next_ocr = true;
            self.ocr_handler.selection_changed(&self.selection);
        }
    }

    fn attempt_copy(&mut self) {
        if self.ocr_handler.ocr_preview_text.is_none() {
            return;
        }

        // Copy the OCR text to the clipboard
        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        ctx.set_contents(self.ocr_handler.ocr_preview_text.clone().unwrap()).expect("Unable to set clipboard contents");
        
        if self.icon_context.settings.close_on_copy {
            self.hide_window();
        }

        self.show_positive_feedback("Copied to clipboard");
    }

    fn attempt_screenshot(&mut self) {
        if self.selection.bounds.width == 0 || self.selection.bounds.height == 0 {
            return;
        }
        let screenshot_path = get_screenshot_path();
        if !std::fs::try_exists(&screenshot_path).unwrap_or(false) {
            return;
        }

        let pos_bounds = self.selection.bounds.to_positive_size();
        if pos_bounds.width < 5 || pos_bounds.height < 5 {
            return;
        }
        
        // Load from screenshot_path and crop
        let img = image::open(screenshot_path);
        if img.is_err() {
            eprintln!("Error loading image: {:?}", img);
            return;
        }
        let screenshot = Screenshot::from(img.unwrap());
        
        let cropped_screenshot = crop_screenshot_to_bounds(pos_bounds, &screenshot);
        let cropped_screenshot = crop_screenshot_to_polygon(
            &self.selection.polygon.vertices.iter().map(|v| (v.x as i32 - self.selection.bounds.x, v.y as i32 - self.selection.bounds.y)).collect(),
            &cropped_screenshot
        );

        copy_image_to_clipboard(&cropped_screenshot.into());
        
        if self.icon_context.settings.close_on_copy {
            self.hide_window();
        }

        self.show_positive_feedback("Screenshot copied to clipboard");
    }

    fn hide_window(&mut self) {
        if !self.window_state.as_ref().unwrap().window.is_visible().unwrap_or(false) {
            return;
        }

        self.input_handler.stop_detecting_keybind();
        self.window_state.as_ref().unwrap().window.set_visible(false);
        self.icon_context.settings.save();

        #[cfg(windows)] {
            unsafe {
                let hwnd = self.window_state.as_ref().unwrap().focus_before_overlay;

                let length = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW(hwnd) + 1;
                let mut title: Vec<u16> = vec![0; length as usize];
                let textw = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, title.as_mut_ptr() as windows_sys::core::PWSTR, length);
                if textw != 0 {
                    if let Ok(title) = String::from_utf16(title[0..(textw as usize)].as_ref()) {
                        println!("Refocused '{}'", title);
                    }
                }

                windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
            }
        }
    }

    fn undo(&mut self) {
        if self.undo_stack.undo(&mut self.selection).is_ok() {
            self.ocr_handler.ocr_preview_text = None;
            self.ocr_handler.selection_changed(&self.selection);
        }
    }
    fn redo(&mut self) {
        if self.undo_stack.redo(&mut self.selection).is_ok() {
            self.ocr_handler.ocr_preview_text = None;
            self.ocr_handler.selection_changed(&self.selection);
        }
    }
    
    fn show_positive_feedback(&mut self, message: &str) {
        if let Some(state) = &mut self.window_state {
            state.shader_renderer.show_user_feedback(message.to_string(), [0.3, 0.8, 0.4]);
        }
    }
    fn show_negative_feedback(&mut self, message: &str) {
        if let Some(state) = &mut self.window_state {
            state.shader_renderer.show_user_feedback(message.to_string(), [0.8, 0.3, 0.4]);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        if self.window_state.is_none() {
            let global_mouse_position = MouseCursor::pos();
            let monitor = event_loop.available_monitors().find(|monitor| {
                monitor.position().x <= global_mouse_position.0
                    && monitor.position().x + monitor.size().width as i32 >= global_mouse_position.0
                    && monitor.position().y <= global_mouse_position.1
                    && monitor.position().y + monitor.size().height as i32 >= global_mouse_position.1
            });
            
            // Need to screenshot before the window is visible
            let screenshot = screenshot_from_handle(
                monitor.clone().unwrap_or(event_loop.primary_monitor().unwrap_or(event_loop.available_monitors().next().expect("No monitors found")))
            );

            let current_focus = {
                #[cfg(windows)] unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }
                #[cfg(not(windows))] { 0 as HWND }
            };

            // Create the window
            let window = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("OCR Overlay")
                        .with_skip_taskbar(true)
                        .with_decorations(false)
                        .with_fullscreen(Some(Fullscreen::Borderless(monitor)))
                        .with_resizable(false)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_visible(false),
                )
                .unwrap();

            let (width, height) = {
                let window_size = window.inner_size();
                (window_size.width, window_size.height)
            };
            self.size = (width, height);

            let surface_texture = SurfaceTexture::new(width, height, &window);

            let builder = PixelsBuilder::new(width, height, surface_texture);
            let builder = builder.clear_color(pixels::wgpu::Color::WHITE);
            let pixels = builder.build().expect("Unable to create pixels");

            let shader_renderer = renderer::Renderer::new(&pixels, width, height, screenshot.bytes.as_slice())
                .expect("Unable to create shader renderer");
            
            self.ocr_handler.set_screenshot(screenshot);
            
            self.undo_stack.take_snapshot(&self.selection);

            self.window_state = Some(WindowState {
                window,
                pixels,
                shader_renderer,

                #[cfg(windows)]
                focus_before_overlay: current_focus
            });
            
            let window = &self.window_state.as_ref().unwrap().window;
            window.set_visible(true);
            window.focus_window();
            self.redraw();
        } else {
            // Move the window to the monitor with the mouse
            let global_mouse_position = MouseCursor::pos();
            let monitor = event_loop.available_monitors().find(|monitor| {
                monitor.position().x <= global_mouse_position.0
                    && monitor.position().x + monitor.size().width as i32 >= global_mouse_position.0
                    && monitor.position().y <= global_mouse_position.1
                    && monitor.position().y + monitor.size().height as i32 >= global_mouse_position.1
            });

            let window_state = self.window_state.as_mut().unwrap();
            let window = &window_state.window;

            #[cfg(windows)] unsafe {
                window_state.focus_before_overlay = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
            }

            // If the window is already open and on the same monitor, just hide it
            if window.is_visible() == Some(true) && window.current_monitor() == monitor {
                self.hide_window();
                return;
            }
            
            window.set_fullscreen(Some(Fullscreen::Borderless(monitor)));
            window.set_visible(false);

            let new_size = window.inner_size();
            if self.size != (new_size.width, new_size.height) {
                self.size = (new_size.width, new_size.height);
                let pixels = &mut window_state.pixels;
                let shader_renderer = &mut window_state.shader_renderer;

                pixels.resize_surface(new_size.width, new_size.height).expect("Unable to resize pixels surface");
                pixels.resize_buffer(new_size.width, new_size.height).expect("Unable to resize pixels buffer");

                let screenshot = screenshot_from_handle(
                    window.current_monitor().unwrap_or(event_loop.primary_monitor().unwrap_or(event_loop.available_monitors().next().expect("No monitors found")))
                );
                shader_renderer.resize(pixels, new_size.width, new_size.height, screenshot.bytes.as_slice()).expect("Unable to resize shader renderer");
            }

            let pixels = &window_state.pixels;
            let shader_renderer = &mut window_state.shader_renderer;

            shader_renderer.before_reopen_window();
            self.ocr_handler.reset_state();
            
            let window = &window_state.window;
            let screenshot = screenshot_from_handle(
                window.current_monitor().unwrap_or(event_loop.primary_monitor().unwrap_or(event_loop.available_monitors().next().expect("No monitors found")))
            );

            let result = shader_renderer.write_screenshot_to_texture(pixels, &screenshot);
            if result.is_err() {
                println!("Error writing screenshot to texture: {:?}", result);
            }
            self.ocr_handler.set_screenshot(screenshot);

            self.selection.reset();
            self.icon_context.reset();

            self.undo_stack.reset();
            self.undo_stack.take_snapshot(&self.selection);

            let window_state = self.window_state.as_mut().unwrap();
            let window = &window_state.window;
            window.set_visible(true);
            window.focus_window();
            self.redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if self.window_state.is_none() {
                    return; // Shouldn't happen, but just in case
                }
        
                let window = &self.window_state.as_mut().unwrap().window;
                if !window.is_visible().unwrap_or(false) {
                    return;
                }

                self.redraw();

                let window = &self.window_state.as_mut().unwrap().window;
                window.request_redraw();
            }
            #[allow(unused)]
            WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => {
                if self.window_state.is_none() {
                    return; // Probably shouldn't happen; just in case
                }

                let window = &self.window_state.as_ref().unwrap().window;

                let mut move_dist = 10.;
                if self.selection.shift_held {
                    move_dist /= 10.;
                } else if self.selection.ctrl_held {
                    move_dist *= 5.;
                }

                if self.input_handler.keyboard_event(&event) {
                    return;
                }

                let result = self.selection.keyboard_event(&event);
                match result {
                    SelectionInputResult::Changed => {
                        self.ocr_handler.selection_changed(&self.selection);
                        self.undo_stack.take_snapshot(&self.selection);
                        return;
                    }
                    SelectionInputResult::CompletelyMoved => {
                        self.ocr_handler.ocr_preview_text = None;
                        self.ocr_handler.selection_changed(&self.selection);
                        self.undo_stack.take_snapshot(&self.selection);
                        return;
                    }
                    SelectionInputResult::SelectionFinished => {
                        self.auto_copy();
                        self.undo_stack.take_snapshot(&self.selection);
                        return;
                    }
                    _ => ()
                }

                match (event.logical_key.as_ref(), self.selection.shift_held, self.selection.ctrl_held) {
                    (Key::Named(NamedKey::Escape), _, _) => {
                        self.hide_window();
                    }
                    (Key::Named(NamedKey::Tab), false, false) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.icon_context.settings.use_polygon = !self.icon_context.settings.use_polygon;
                        }
                    }
                    (Key::Character("c"), false, _) => {
                        self.icon_context.copy_key_held = event.state == winit::event::ElementState::Pressed;
                        if event.state == winit::event::ElementState::Pressed {
                            self.attempt_copy();
                        }
                    }
                    (Key::Character("s"), false, _) => {
                        self.icon_context.screenshot_key_held = event.state == winit::event::ElementState::Pressed;
                        if event.state == winit::event::ElementState::Pressed {
                            self.attempt_screenshot();
                        }
                    }
                    (Key::Named(NamedKey::ArrowDown), _, _) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.polygon.move_by(0., move_dist);
                            self.selection.polygon.clamp_to_screen(self.size);
                            self.selection.bounds.enclose_polygon(&self.selection.polygon);
                            self.ocr_handler.selection_changed(&self.selection);
                            self.undo_stack.take_snapshot(&self.selection);
                        }
                    }
                    (Key::Named(NamedKey::ArrowUp), _, _) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.polygon.move_by(0., -move_dist);
                            self.selection.polygon.clamp_to_screen(self.size);
                            self.selection.bounds.enclose_polygon(&self.selection.polygon);
                            self.ocr_handler.selection_changed(&self.selection);
                            self.undo_stack.take_snapshot(&self.selection);
                        }
                    }
                    (Key::Named(NamedKey::ArrowLeft), _, _) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.polygon.move_by(-move_dist, 0.);
                            self.selection.polygon.clamp_to_screen(self.size);
                            self.selection.bounds.enclose_polygon(&self.selection.polygon);
                            self.ocr_handler.selection_changed(&self.selection);
                            self.undo_stack.take_snapshot(&self.selection);
                        }
                    }
                    (Key::Named(NamedKey::ArrowRight), _, _) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.polygon.move_by(move_dist, 0.);
                            self.selection.polygon.clamp_to_screen(self.size);
                            self.selection.bounds.enclose_polygon(&self.selection.polygon);
                            self.ocr_handler.selection_changed(&self.selection);
                            self.undo_stack.take_snapshot(&self.selection);
                        }
                    }

                    // Toggle settings
                    (Key::Character("1"), false, false) |
                    (Key::Character("2"), false, false) |
                    (Key::Character("3"), false, false) |
                    (Key::Character("4"), false, false) |
                    (Key::Character("5"), false, false) |
                    (Key::Character("6"), false, false) => {
                        if event.state == winit::event::ElementState::Pressed && !event.repeat {
                            let settings = &mut self.icon_context.settings;
                            match event.logical_key.as_ref() {
                                Key::Character("1") => settings.maintain_newline = !settings.maintain_newline,
                                Key::Character("2") => settings.reformat_and_correct = !settings.reformat_and_correct,
                                Key::Character("3") => settings.background_blur_enabled = !settings.background_blur_enabled,
                                Key::Character("4") => settings.add_pilcrow_in_preview = !settings.add_pilcrow_in_preview,
                                Key::Character("5") => settings.close_on_copy = !settings.close_on_copy,
                                Key::Character("6") => settings.auto_copy = !settings.auto_copy,
                                _ => (),
                            }
                        }
                    }

                    (Key::Character("z"), false, _) => {
                        if self.selection.ctrl_held && event.state == winit::event::ElementState::Pressed {
                            self.undo();
                        }
                    }
                    (Key::Character("y"), false, _) => {
                        if self.selection.ctrl_held && event.state == winit::event::ElementState::Pressed {
                            self.redo();
                        }
                    }

                    (Key::Character("a"), false, _) => {
                        if event.state == winit::event::ElementState::Pressed && self.selection.ctrl_held {
                            self.selection.bounds = self.size.into();
                            self.selection.polygon.set_from_bounds(&self.selection.bounds);
                            self.ocr_handler.selection_changed(&self.selection);
                            self.undo_stack.take_snapshot(&self.selection);
                        }
                    }

                    _ => (),
                }
            }
            #[allow(unused)]
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                let (x, y) = {
                    // We use the gobal mouse position and make it relative instead of the relative one
                    // because the relative one can only be set when the mouse moves and it's possible
                    // to click before then.
                    let pos = MouseCursor::pos();
                    let window = &self.window_state.as_ref().unwrap().window;
                    let window_pos = window.inner_position().unwrap_or_default();
                    (pos.0 - window_pos.x, pos.1 - window_pos.y)
                };

                let window_state = self.window_state.as_mut().unwrap();
                let mut was_handled = false;
                if button == winit::event::MouseButton::Left {
                    was_handled = window_state.shader_renderer.mouse_event((x, y), state, &mut self.icon_context);
                }

                if !was_handled {
                    let result = self.selection.mouse_input(state, button, self.relative_mouse_pos, &mut self.icon_context);
                    if result == SelectionInputResult::CompletelyMoved {
                        self.ocr_handler.ocr_preview_text = None; // Clear the preview if the selection completely moved
                    }
                    self.ocr_handler.selection_changed(&self.selection);
                    if result == SelectionInputResult::SelectionFinished && state == ElementState::Released {
                        self.auto_copy();
                        self.undo_stack.take_snapshot(&self.selection);
                    }
                }
            },
            #[allow(unused)]
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                if self.window_state.is_none() {
                    return; // Probably shouldn't happen; just in case
                }

                self.relative_mouse_pos = (position.x as i32, position.y as i32);
                let result: SelectionInputResult = self.selection.cursor_moved(self.relative_mouse_pos, self.size, &mut self.icon_context);

                match result {
                    SelectionInputResult::Changed => {
                        self.ocr_handler.selection_changed(&self.selection);
                    }
                    SelectionInputResult::CompletelyMoved => {
                        self.ocr_handler.ocr_preview_text = None;
                        self.ocr_handler.selection_changed(&self.selection);
                    }
                    SelectionInputResult::SelectionFinished => {
                        self.auto_copy();
                    }
                    _ => ()
                }
            }
            _ => (),
        }
    }
}
