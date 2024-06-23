use inputbot::{KeybdKey::*, MouseCursor};
use ocr_handler::OCRHandler;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use screenshot::screenshot_primary;
use selection::Selection;
use std::thread;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Fullscreen, Window, WindowId, WindowLevel};

mod ocr_handler;
mod renderer;
mod screenshot;
mod selection;
mod wgpu_text;

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

#[derive(Default)]
struct App {
    window_state: Option<WindowState>,
    size: (u32, u32),
    selection: Selection,
    ocr_handler: OCRHandler,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        if self.window_state.is_none() {
            // Need to screenshot before the window is visible
            let screenshot = screenshot_primary();
            self.ocr_handler.set_screenshot(screenshot.clone());

            // Create the window
            let window = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("OCR Overlay")
                        .with_skip_taskbar(true)
                        .with_decorations(false)
                        .with_fullscreen(Some(Fullscreen::Borderless(None)))
                        .with_resizable(false)
                        .with_window_level(WindowLevel::AlwaysOnTop),
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
            let builder =
                builder.render_texture_format(pixels::wgpu::TextureFormat::Bgra8UnormSrgb);
            let pixels = builder.build().expect("Unable to create pixels");

            let mut shader_renderer = renderer::Renderer::new(&pixels, width, height)
                .expect("Unable to create shader renderer");
            let result = shader_renderer.write_screenshot_to_texture(&pixels, screenshot);
            if result.is_err() {
                println!("Error writing screenshot to texture: {:?}", result);
            }

            self.window_state = Some(WindowState {
                window,
                pixels,
                shader_renderer,
            });
        } else {
            // Show the window
            let window_state = self.window_state.as_mut().unwrap();
            let window = &window_state.window;
            let pixels = &window_state.pixels;
            let shader_renderer = &mut window_state.shader_renderer;
            
            let screenshot = screenshot_primary();
            self.ocr_handler.set_screenshot(screenshot.clone());
            let result = shader_renderer.write_screenshot_to_texture(pixels, screenshot);
            if result.is_err() {
                println!("Error writing screenshot to texture: {:?}", result);
            }

            self.selection.reset();

            window.set_minimized(false);
            window.focus_window();
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.
                if self.window_state.is_none() {
                    return; // Shouldn't happen, but just in case
                }

                let state = self.window_state.as_mut().unwrap();
                let window = &state.window;
                if window.is_minimized().unwrap_or(true) {
                    return;
                }

                let pixels = &state.pixels;
                let shader_renderer = &mut state.shader_renderer;

                self.ocr_handler.update_ocr_preview_text();

                let render_result = pixels.render_with(|encoder, render_target, context| {
                    shader_renderer.update(
                        context,
                        self.size,
                        self.selection,
                        self.ocr_handler.ocr_preview_text.clone()
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

                match event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        window.set_minimized(true);
                    }
                    Key::Named(NamedKey::Shift) => {
                        self.selection.shift_held =
                            event.state == winit::event::ElementState::Pressed;
                    }
                    _ => (),
                }
            }
            #[allow(unused)]
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => match button {
                winit::event::MouseButton::Left => {
                    if state == winit::event::ElementState::Pressed {
                        let (x, y) = MouseCursor::pos();
                        self.selection.bounds.x = x;
                        self.selection.bounds.y = y;
                        self.selection.bounds.width = 0;
                        self.selection.bounds.height = 0;
                        self.selection.mouse_down = true;
                        self.ocr_handler.ocr_preview_text = None; // Clear the preview if the selection completely moved
                    } else {
                        self.selection.mouse_down = false;
                    }
                    self.window_state.as_ref().unwrap().window.request_redraw();
                }
                _ => (),
            },
            #[allow(unused)]
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                if self.window_state.is_none() {
                    return; // Probably shouldn't happen; just in case
                }

                if (!self.selection.mouse_down) {
                    return;
                }

                // If shift is held, move the selection instead of resizing
                let (x, y) = (position.x as i32, position.y as i32);
                if !self.selection.shift_held {
                    self.selection.bounds.width = x - self.selection.bounds.x;
                    self.selection.bounds.height = y - self.selection.bounds.y;
                } else {
                    self.selection.bounds.x = x - self.selection.bounds.width;
                    self.selection.bounds.y = y - self.selection.bounds.height;
                }
                self.window_state.as_ref().unwrap().window.request_redraw();

                self.ocr_handler.selection_changed(self.selection);
            }
            _ => (),
        }
    }
}
