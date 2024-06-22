use inputbot::KeybdKey::*;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId, WindowLevel};
use std::sync::mpsc;
use std::thread;
use std::cell::RefCell;

fn main() {
    let (tx, rx) = mpsc::channel();

    TKey.bind(move || {
        if LShiftKey.is_pressed() && LSuper.is_pressed() {
            // We need to open the window on the main thread
            tx.send(()).unwrap();
        }
    });

    thread::spawn(move || {
        inputbot::handle_input_events();
    });
    
    println!("Listening for keybinds");

    // Only run event loop on user interaction
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    for _ in rx {
        println!("Opening OCR overlay");
        open_ocr_overlay(event_loop);
    }
}

#[derive(Default)]
struct App {
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
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

                // Draw.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                self.window.as_ref().unwrap().request_redraw();
            },
            _ => (),
        }
    }
}

fn open_ocr_overlay(event_loop: EventLoop<()>) {
    let mut app = App::default();
    event_loop.run_app(&mut app);
}