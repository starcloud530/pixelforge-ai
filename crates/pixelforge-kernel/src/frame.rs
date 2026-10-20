/// One frame of palette indices. Length is always `width * height`.
///
/// `delay_ms` is how long this frame stays up during loop playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pixels: Vec<u8>,
    delay_ms: u32,
}

impl Frame {
    pub(crate) fn blank(len: usize, delay_ms: u32) -> Self {
        Self {
            pixels: vec![0; len],
            delay_ms,
        }
    }

    pub(crate) fn from_pixels(pixels: Vec<u8>, delay_ms: u32) -> Self {
        Self { pixels, delay_ms }
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub(crate) fn delay_ms(&self) -> u32 {
        self.delay_ms
    }

    pub(crate) fn set_delay_ms(&mut self, delay_ms: u32) {
        self.delay_ms = delay_ms;
    }

    pub(crate) fn set(&mut self, offset: usize, value: u8) {
        self.pixels[offset] = value;
    }

    pub(crate) fn set_pixels(&mut self, pixels: Vec<u8>) {
        self.pixels = pixels;
    }
}
