#![feature(duration_millis_float)]
#![feature(fs_try_exists)]

use clipboard::{ClipboardContext, ClipboardProvider};
use clipboard_image::copy_image_to_clipboard;
use inputbot::{KeybdKey::*, MouseCursor};
use ocr_handler::{FormatOptions, OCRHandler, LATEST_SCREENSHOT_PATH};
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use screenshot::screenshot_from_handle;
use selection::Selection;
use std::sync::mpsc;
use std::thread;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
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

fn main() {
    // Only run event loop on user interaction
    let event_loop = EventLoop::new().expect("Unable to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let loop_proxy = event_loop.create_proxy();
    ZKey.bind(move || {
        if LShiftKey.is_pressed() && LAltKey.is_pressed() {
            // We need to open the window on the main thread
            loop_proxy.send_event(()).expect("Unable to send event");
        }
    });

    thread::spawn(|| {
        inputbot::handle_input_events();
    });

    println!("Listening for keybinds");
    event_loop
        .run_app(&mut App::default())
        .expect("Unable to run event loop");
}

struct WindowState {
    window: Window,
    pixels: Pixels,
    shader_renderer: renderer::Renderer,
}

struct App {
    window_state: Option<WindowState>,
    size: (u32, u32),
    selection: Selection,
    ocr_handler: OCRHandler,
    relative_mouse_pos: (i32, i32),

    icon_context: IconContext,
    icon_event_receiver: mpsc::Receiver<IconEvent>,
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
            
            icon_context,
            icon_event_receiver,
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

        let state = self.window_state.as_mut().unwrap();

        let pixels = &state.pixels;
        let shader_renderer = &mut state.shader_renderer;

        let ocr_text_changed = self.ocr_handler.update_ocr_preview_text();

        self.icon_context.has_selection = self.selection.bounds.width != 0 && self.selection.bounds.height != 0;

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

        if ocr_text_changed {
            if self.icon_context.settings.auto_copy {
                self.attempt_copy();
            }
        }

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
                    self.icon_context.settings.tesseract_settings.reload();
                    self.ocr_handler.update_ocr_settings(self.icon_context.settings.tesseract_settings.clone());
                }
                IconEvent::ChangeUsePolygon => {
                    self.selection.reset();
                    self.ocr_handler.reset_state();
                }
            }
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
    }

    fn attempt_screenshot(&mut self) {
        if self.selection.bounds.width == 0 || self.selection.bounds.height == 0 {
            return;
        }
        if !std::fs::try_exists(LATEST_SCREENSHOT_PATH).unwrap_or(false) {
            return;
        }

        let mut pos_bounds = self.selection.bounds.to_positive_size();
        if pos_bounds.width < 5 || pos_bounds.height < 5 {
            return;
        }
        
        // Load from LATEST_SCREENSHOT_PATH and crop
        let img = image::open(LATEST_SCREENSHOT_PATH);
        if img.is_err() {
            eprintln!("Error loading image: {:?}", img);
            return;
        }
        let mut img = img.unwrap();
        
        if pos_bounds.x + pos_bounds.width > img.width() as i32 {
            pos_bounds.width = img.width() as i32 - pos_bounds.x;
        }
        if pos_bounds.y + pos_bounds.height > img.height() as i32 {
            pos_bounds.height = img.height() as i32 - pos_bounds.y;
        }

        img.crop(
            pos_bounds.x as u32,
            pos_bounds.y as u32,
            pos_bounds.width as u32,
            pos_bounds.height as u32,
        );

        copy_image_to_clipboard(&img);
        
        if self.icon_context.settings.close_on_copy {
            self.hide_window();
        }
    }

    fn hide_window(&mut self) {
        self.window_state.as_ref().unwrap().window.set_visible(false);
        self.icon_context.settings.save();
    }

    fn fix_keys_held(&mut self) {
        self.selection.shift_held = inputbot::KeybdKey::LShiftKey.is_pressed();
        self.selection.ctrl_held = inputbot::KeybdKey::LControlKey.is_pressed();
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

            self.window_state = Some(WindowState {
                window,
                pixels,
                shader_renderer,
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

            let window_state = self.window_state.as_mut().unwrap();
            let window = &window_state.window;
            window.set_visible(true);
            window.focus_window();
            self.redraw();

            self.fix_keys_held();
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

                let mut move_dist = 10;
                if self.selection.shift_held {
                    move_dist /= 10;
                } else if self.selection.ctrl_held {
                    move_dist *= 5;
                }

                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.hide_window();
                    }
                    Key::Named(NamedKey::Shift) => {
                        self.selection.shift_held = event.state == winit::event::ElementState::Pressed;
                    }
                    Key::Named(NamedKey::Control) => {
                        self.selection.ctrl_held =
                            event.state == winit::event::ElementState::Pressed;
                    }
                    Key::Named(NamedKey::Tab) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.icon_context.settings.use_polygon = !self.icon_context.settings.use_polygon;
                        }
                    }
                    Key::Character("c") => {
                        self.icon_context.copy_key_held = event.state == winit::event::ElementState::Pressed;
                        if event.state == winit::event::ElementState::Pressed {
                            self.attempt_copy();
                        }
                    }
                    Key::Character("s") => {
                        self.icon_context.screenshot_key_held = event.state == winit::event::ElementState::Pressed;
                        if event.state == winit::event::ElementState::Pressed {
                            self.attempt_screenshot();
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.bounds.y += move_dist;
                            self.selection.bounds.clamp_to_screen(self.size);
                            self.ocr_handler.selection_changed(&self.selection);
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.bounds.y -= move_dist;
                            self.selection.bounds.clamp_to_screen(self.size);
                            self.ocr_handler.selection_changed(&self.selection);
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.bounds.x -= move_dist;
                            self.selection.bounds.clamp_to_screen(self.size);
                            self.ocr_handler.selection_changed(&self.selection);
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.bounds.x += move_dist;
                            self.selection.bounds.clamp_to_screen(self.size);
                            self.ocr_handler.selection_changed(&self.selection);
                        }
                    }

                    // Toggle settings
                    Key::Character("1") | Key::Character("2") | Key::Character("3") | Key::Character("4") => {
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
                    if self.selection.mouse_input(state, button, self.relative_mouse_pos, &mut self.icon_context) {
                        self.ocr_handler.ocr_preview_text = None; // Clear the preview if the selection completely moved
                    }
                }

                self.window_state.as_ref().unwrap().window.request_redraw();
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
                let changed = self.selection.cursor_moved(self.relative_mouse_pos, self.size);

                if changed {
                    self.window_state.as_ref().unwrap().window.request_redraw();
                    self.ocr_handler.selection_changed(&self.selection);
                }
            }
            _ => (),
        }
    }
}
