/// One frame of palette indices. Length is always `width * height`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pixels: Vec<u8>,
}

impl Frame {
    pub(crate) fn blank(len: usize) -> Self {
        Self {
            pixels: vec![0; len],
        }
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub(crate) fn set(&mut self, offset: usize, value: u8) {
        self.pixels[offset] = value;
    }
}
