use scrap::{Display, Capturer};

pub(crate) struct Screenshot {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

pub fn get_screenshot() -> Screenshot {
    // TODO: Use the display with the mouse
    let display = Display::primary().expect("Unable to get primary display");
    let mut capturer = Capturer::new(display).expect("Unable to create capturer");
    let (width, height) = (capturer.width(), capturer.height());
    let frame = capturer.frame().expect("Unable to get frame");

    Screenshot {
        width,
        height,
        bytes: frame.to_vec(),
    }
}