use inputbot::KeybdKey::*;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowId, WindowLevel, Fullscreen};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::thread;

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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Selection {
    x: i32,
    y: i32,
    width: i32,
    height: i32
}

#[derive(Default)]
struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,

    last_selection: Selection,
    current_selection: Selection
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        if self.window.is_some() {
            // There's already a window open; do nothing.
            return;
        }

        // TODO: Use the monitor that the mouse is currently on
        let monitor = event_loop.primary_monitor().unwrap();

        self.window = Some(Rc::new(event_loop.create_window(
            Window::default_attributes()
                .with_title("OCR Overlay")
                .with_skip_taskbar(true)
                .with_transparent(true)
                .with_decorations(false)
                .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))))
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
        ).unwrap()));

        let context = softbuffer::Context::new(self.window.clone().unwrap()).unwrap();
        let surface = softbuffer::Surface::new(&context, self.window.clone().unwrap()).unwrap();

        self.surface = Some(surface);
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

                // Only draw if the current selection has changed
                if self.last_selection != self.current_selection {
                    draw(self);
                    self.last_selection = self.current_selection;
                    
                    // Queue a RedrawRequested event.
                    //
                    // You only need to call this if you've determined that you need to redraw in
                    // applications which do not always need to. Applications that redraw continuously
                    // can render here instead.
                    self.window.as_ref().unwrap().request_redraw();
                }
            },
            WindowEvent::MouseInput { device_id, state, button } => {
                // Handle mouse input.
                println!("Mouse input: {:?} {:?} {:?}", device_id, state, button);
            },
            WindowEvent::KeyboardInput { device_id, event, is_synthetic } => {
                // Handle keyboard input.
                println!("Keyboard input: {:?} {:?} {:?}", device_id, event, is_synthetic);
            }
            _ => (),
        }
    }
}

fn draw(app: &mut App) {
    println!("Drawing window");

    if app.surface.is_none() || app.window.is_none() {
        return;
    }

    let window = app.window.as_ref().unwrap();
    let surface = app.surface.as_mut().unwrap();

    // Draw the application.
    //
    // This is called when the application is ready to draw and is the place to put the code
    // to draw the application. This is called after the RedrawRequested event is received.
    let (width, height) = {
        let size = window.inner_size();
        (size.width, size.height)
    };
    surface
        .resize(
            NonZeroU32::new(width).unwrap(),
            NonZeroU32::new(height).unwrap(),
        )
        .unwrap();

    let mut buffer = surface.buffer_mut().unwrap();
    // for index in 0..(width * height) {
    //     let y = index / width;
    //     let x = index % width;
    //     let red = x % 255;
    //     let green = y % 255;
    //     let blue = (x * y) % 255;

    //     buffer[index as usize] = blue | (green << 8) | (red << 16);
    // }
    buffer.fill(0xFF181818);

    buffer.present().unwrap();
}