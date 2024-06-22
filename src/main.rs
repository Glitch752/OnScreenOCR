use inputbot::KeybdKey::*;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId, WindowLevel};
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

#[derive(Default)]
struct App {
    window: Option<Window>
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        if self.window.is_some() {
            // There's already a window open; do nothing.
            return;
        }

        self.window = Some(event_loop.create_window(
            Window::default_attributes()
                .with_title("OCR Overlay")
                // .with_skip_taskbar(true)
                .with_transparent(true)
                .with_decorations(false)
                .with_maximized(true)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
        ).unwrap());
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

                draw(self);

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                self.window.as_ref().unwrap().request_redraw();
            },
            WindowEvent::MouseInput { device_id, state, button } => {
                // Handle mouse input.
                println!("Mouse input: {:?} {:?} {:?}", device_id, state, button);
            },
            _ => (),
        }
    }
}

fn draw(app: &mut App) {
    // Draw the application.
    //
    // This is called when the application is ready to draw and is the place to put the code
    // to draw the application. This is called after the RedrawRequested event is received.
}