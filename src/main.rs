use inputbot::KeybdKey::*;
use pixels::raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowId, WindowLevel, Fullscreen};
use std::thread;
use pixels::{Pixels, SurfaceTexture};

use winapi::shared::windef::HWND;

fn main() {
    // Only run event loop on user interaction
    let event_loop = EventLoop::new().expect("Unable to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let loop_proxy = event_loop.create_proxy();
    TKey.bind(move || {
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct Selection {
    x: i32,
    y: i32,
    width: i32,
    height: i32
}

impl Default for Selection {
    fn default() -> Self {
        Selection {
            x: 300,
            y: 300,
            width: 500,
            height: 500
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<Window>,
    pixels: Option<Pixels>,

    size: (u32, u32),

    current_selection: Selection
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(event_loop.create_window(
            Window::default_attributes()
                .with_title("OCR Overlay")
                .with_skip_taskbar(true)
                .with_transparent(true)
                .with_decorations(false)
                .with_fullscreen(Some(Fullscreen::Borderless(None)))
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
        ).unwrap());

        let (width, height) = {
            let window = self.window.as_ref().unwrap();
            let window_size = window.inner_size();
            (window_size.width, window_size.height)
        };
        self.size = (width, height);
        
        self.window.as_mut().unwrap().set_minimized(true);
        
        let surface_texture = SurfaceTexture::new(
            width, height,
            self.window.as_ref().unwrap()
        );
        self.pixels = Some(Pixels::new(width, height, surface_texture).expect("Unable to create pixel buffer"));
        self.pixels.as_mut().unwrap().clear_color(pixels::wgpu::Color::TRANSPARENT);
        
        self.window.as_ref().unwrap().request_redraw();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        let window = self.window.as_mut().unwrap();
        window.set_minimized(false);
        window.focus_window();
        window.request_redraw();
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
                if self.window.is_none() {
                    return;
                }
                let window = self.window.as_ref().unwrap();
                if window.is_minimized().unwrap_or(false) {
                    return;
                }

                // Only draw if the current selection has changed
                draw(self);
            },
            WindowEvent::MouseInput { device_id, state, button } => {
                // Handle mouse input.
                println!("Mouse input: {:?} {:?} {:?}", device_id, state, button);
            },
            #[allow(unused)]
            WindowEvent::KeyboardInput { device_id, event, is_synthetic } => {
                if self.window.is_none() {
                    return; // Probably shouldn't happen; just in case
                }
                let window = self.window.as_ref().unwrap();

                match event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        window.set_minimized(true);
                    },
                    _ => (),
                }
            },
            _ => (),
        }
    }
}

fn draw(app: &mut App) {
    if app.pixels.is_none() || app.window.is_none() {
        return;
    }

    let pixels = app.pixels.as_mut().unwrap();
    let frame = pixels.frame_mut();
    let (width, height) = app.size;

    println!("Drawing window");

    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
        let x = (i % width as usize) as i32;
        let y = (i / height as usize) as i32;

        let inside_the_box = x >= app.current_selection.x
            && x < app.current_selection.x + app.current_selection.width
            && y >= app.current_selection.y
            && y < app.current_selection.y + app.current_selection.height;

        let rgba = if inside_the_box {
            [0x0, 0x0, 0x0, 0x0]
        } else {
            [0x48, 0xb2, 0xe8, 0x50]
        };

        pixel.copy_from_slice(&rgba);
    }

    pixels.render().expect("Unable to render pixels");
}