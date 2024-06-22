use inputbot::{KeybdKey::*, MouseCursor};
use renderer::Locals;
use screenshot::screenshot_primary;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowId, WindowLevel, Fullscreen};
use std::thread;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use selection::Selection;

mod renderer;
mod selection;
mod screenshot;

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
    event_loop.run_app(&mut App::default()).expect("Unable to run event loop");
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
    current_selection: Selection,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        if self.window_state.is_none() {
            // Need to screenshot before the window is visible
            let screenshot = screenshot_primary();

            // Create the window
            let window = event_loop.create_window(
                Window::default_attributes()
                    .with_title("OCR Overlay")
                    .with_skip_taskbar(true)
                    .with_decorations(false)
                    .with_fullscreen(Some(Fullscreen::Borderless(None)))
                    .with_resizable(false)
                    .with_window_level(WindowLevel::AlwaysOnTop)
            ).unwrap();

            let (width, height) = {
                let window_size = window.inner_size();
                (window_size.width, window_size.height)
            };
            self.size = (width, height);
            
            let surface_texture = SurfaceTexture::new(
                width, height,
                &window
            );

            let builder = PixelsBuilder::new(width, height, surface_texture);
            let builder = builder.clear_color(pixels::wgpu::Color::WHITE);
            let builder = builder.render_texture_format(pixels::wgpu::TextureFormat::Bgra8UnormSrgb);
            let pixels = builder.build().expect("Unable to create pixels");

            let mut shader_renderer = renderer::Renderer::new(&pixels, width, height).expect("Unable to create shader renderer");
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
            let result = shader_renderer.write_screenshot_to_texture(pixels, screenshot_primary());
            if result.is_err() {
                println!("Error writing screenshot to texture: {:?}", result);
            }

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
            },
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.
                if self.window_state.is_none() {
                    return; // Shouldn't happen, but just in case
                }
                
                let window = &self.window_state.as_ref().unwrap().window;
                if window.is_minimized().unwrap_or(true) {
                    return;
                }

                let pixels = &self.window_state.as_ref().unwrap().pixels;
                let shader_renderer = &self.window_state.as_ref().unwrap().shader_renderer;

                let render_result = pixels.render_with(|encoder, render_target, context| {
                    shader_renderer.update(&context.queue, Locals::new(self.current_selection, self.size));
                    shader_renderer.render(encoder, render_target, context.scaling_renderer.clip_rect());

                    Ok(())
                });

                if render_result.is_err() {
                    println!("Error rendering: {:?}", render_result);
                }

                window.request_redraw();
            },
            #[allow(unused)]
            WindowEvent::KeyboardInput { device_id, event, is_synthetic } => {
                if self.window_state.is_none() {
                    return; // Probably shouldn't happen; just in case
                }
                let window = &self.window_state.as_ref().unwrap().window;

                match event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        window.set_minimized(true);
                    },
                    _ => (),
                }
            },
            #[allow(unused)]
            WindowEvent::MouseInput { device_id, state, button } => {
                match button {
                    winit::event::MouseButton::Left => {
                        if state == winit::event::ElementState::Pressed {
                            let (x, y) = MouseCursor::pos();
                            self.current_selection.x = x;
                            self.current_selection.y = y;
                            self.current_selection.width = 0;
                            self.current_selection.height = 0;
                            self.current_selection.mouse_down = true;
                        } else {
                            self.current_selection.mouse_down = false;
                        }
                        self.window_state.as_ref().unwrap().window.request_redraw();
                    },
                    _ => (),
                }
            },
            #[allow(unused)]
            WindowEvent::CursorMoved { device_id, position } => {
                if self.window_state.is_none() {
                    return; // Probably shouldn't happen; just in case
                }

                if(!self.current_selection.mouse_down) {
                    return;
                }
                
                let (x, y) = MouseCursor::pos();
                self.current_selection.width = x - self.current_selection.x;
                self.current_selection.height = y - self.current_selection.y;

                self.window_state.as_ref().unwrap().window.request_redraw();
            }
            _ => (),
        }
    }
}