use clipboard::{ClipboardContext, ClipboardProvider};
use inputbot::{KeybdKey::*, MouseCursor};
use ocr_handler::OCRHandler;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use screenshot::screenshot_from_handle;
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
    relative_mouse_pos: (i32, i32),
}

impl App {
    fn redraw(&mut self) {
        let state = self.window_state.as_mut().unwrap();

        let pixels = &state.pixels;
        let shader_renderer = &mut state.shader_renderer;

        self.ocr_handler.update_ocr_preview_text();

        let render_result = pixels.render_with(|encoder, render_target, context| {
            shader_renderer.update(
                context,
                self.size,
                self.selection,
                self.ocr_handler.ocr_preview_text.clone(),
                self.relative_mouse_pos
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

            // Temporary: save the screenshot
            println!("{:?}", screenshot);
            let image: image::ImageBuffer<image::Rgba<_>, _> = image::ImageBuffer::from_raw(screenshot.width as u32, screenshot.height as u32, screenshot.bytes.clone()).unwrap();
            image.save("screenshot.png").unwrap();

            self.ocr_handler.set_screenshot(screenshot.clone()); // TODO: Remove this .clone() somehow

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
            let builder =
                builder.render_texture_format(pixels::wgpu::TextureFormat::Bgra8UnormSrgb);
            let pixels = builder.build().expect("Unable to create pixels");

            let shader_renderer = renderer::Renderer::new(&pixels, width, height, screenshot.bytes.as_slice())
                .expect("Unable to create shader renderer");

            self.window_state = Some(WindowState {
                window,
                pixels,
                shader_renderer,
            });

            self.redraw();
            
            let window = &self.window_state.as_ref().unwrap().window;
            window.set_visible(true);
            window.focus_window();

            println!("Done initializing");
        } else {
            // Show the window
            let window_state = self.window_state.as_mut().unwrap();
            let pixels = &window_state.pixels;
            let shader_renderer = &mut window_state.shader_renderer;
            
            let window = &window_state.window;
            let screenshot = screenshot_from_handle(
                window.current_monitor().unwrap_or(event_loop.primary_monitor().unwrap_or(event_loop.available_monitors().next().expect("No monitors found")))
            );

            self.ocr_handler.set_screenshot(screenshot.clone()); // TODO: Remove this .clone() somehow
            let result = shader_renderer.write_screenshot_to_texture(pixels, screenshot);
            if result.is_err() {
                println!("Error writing screenshot to texture: {:?}", result);
            }

            self.selection.reset();
            self.redraw();

            let window_state = self.window_state.as_mut().unwrap();
            let window = &window_state.window;
            window.set_visible(true);
            window.focus_window();
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
                        window.set_visible(false);
                    }
                    Key::Named(NamedKey::Shift) => {
                        self.selection.shift_held =
                            event.state == winit::event::ElementState::Pressed;
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.start_drag_location = self.relative_mouse_pos;
                            self.selection.start_drag_bounds_origin = (self.selection.bounds.x, self.selection.bounds.y);
                        }
                    }
                    Key::Named(NamedKey::Control) => {
                        self.selection.ctrl_held =
                            event.state == winit::event::ElementState::Pressed;
                    }
                    Key::Character("c") => {
                        if event.state == winit::event::ElementState::Pressed {
                            if self.ocr_handler.ocr_preview_text.is_none() {
                                return;
                            }

                            // Copy the OCR text to the clipboard
                            let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                            ctx.set_contents(self.ocr_handler.ocr_preview_text.clone().unwrap());
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.bounds.y += move_dist;
                            self.selection.bounds.clamp_to_screen(self.size);
                            self.ocr_handler.selection_changed(self.selection);
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.bounds.y -= move_dist;
                            self.selection.bounds.clamp_to_screen(self.size);
                            self.ocr_handler.selection_changed(self.selection);
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.bounds.x -= move_dist;
                            self.selection.bounds.clamp_to_screen(self.size);
                            self.ocr_handler.selection_changed(self.selection);
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if event.state == winit::event::ElementState::Pressed {
                            self.selection.bounds.x += move_dist;
                            self.selection.bounds.clamp_to_screen(self.size);
                            self.ocr_handler.selection_changed(self.selection);
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
            } => match button {
                winit::event::MouseButton::Left => {
                    let (x, y) = MouseCursor::pos();

                    let was_handled = self.window_state.as_mut().unwrap().shader_renderer.mouse_event((x, y), state);

                    if !was_handled {
                        if state == winit::event::ElementState::Pressed {
                            if !self.selection.shift_held {
                                self.selection.bounds.x = x;
                                self.selection.bounds.y = y;
                                self.selection.bounds.width = 0;
                                self.selection.bounds.height = 0;
                            } else {
                                self.selection.start_drag_location = (x, y);
                                self.selection.start_drag_bounds_origin = (self.selection.bounds.x, self.selection.bounds.y);
                            }
                            self.selection.mouse_down = true;
                            self.ocr_handler.ocr_preview_text = None; // Clear the preview if the selection completely moved
                        } else {
                            self.selection.mouse_down = false;
                        }
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

                self.relative_mouse_pos = (position.x as i32, position.y as i32);

                if (!self.selection.mouse_down) {
                    return;
                }

                // If shift is held, move the selection instead of resizing
                let (x, y) = (position.x as i32, position.y as i32);
                if !self.selection.shift_held {
                    self.selection.bounds.width = x - self.selection.bounds.x;
                    self.selection.bounds.height = y - self.selection.bounds.y;
                } else {
                    let (start_x, start_y) = self.selection.start_drag_location;
                    let (start_bounds_x, start_bounds_y) = self.selection.start_drag_bounds_origin;
                    let (dx, dy) = (x - start_x, y - start_y);
                    self.selection.bounds.x = start_bounds_x + dx;
                    self.selection.bounds.y = start_bounds_y + dy;
                    self.selection.bounds.clamp_to_screen(self.size);
                }
                self.window_state.as_ref().unwrap().window.request_redraw();

                self.ocr_handler.selection_changed(self.selection);
            },
            WindowEvent::Resized(new_size) => {
                if self.window_state.is_none() {
                    return; // Probably shouldn't happen; just in case
                }

                let window_state = self.window_state.as_mut().unwrap();

                // If size is not equal to the size of the monitor the window is on, it's still initializing
                let window = &window_state.window;
                let monitor_size = window.current_monitor().unwrap().size();
                if new_size.width != monitor_size.width || new_size.height != monitor_size.height {
                    return;
                }

                // If the size is equal to our current size, don't do anything
                if new_size.width == self.size.0 && new_size.height == self.size.1 {
                    return;
                }

                println!("Resized to {:?}", new_size);

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
            _ => (),
        }
    }
}
