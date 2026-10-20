/// One frame: a stack of palette-index buffers, plus how long it stays up.
///
/// Every layer has length `width * height`. Index 0 is transparent. A frame
/// written before layers existed is one buffer; the loader turns that into a
/// single layer. [`Self::composite`] stacks those buffers from bottom to top:
/// a non-zero index covers the layers under it, and index 0 leaves them
/// visible. Palette alpha is not blended here. Export resolves the winning
/// index afterwards.
///
/// `delay_ms` is how long this frame stays up during loop playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    layers: Vec<Vec<u8>>,
    delay_ms: u32,
}

impl Frame {
    pub(crate) fn blank(layer_count: usize, len: usize, delay_ms: u32) -> Self {
        debug_assert!(layer_count >= 1, "a frame keeps at least one layer");
        Self {
            layers: vec![vec![0; len]; layer_count],
            delay_ms,
        }
    }

    pub(crate) fn from_layers(layers: Vec<Vec<u8>>, delay_ms: u32) -> Self {
        debug_assert!(!layers.is_empty(), "a frame keeps at least one layer");
        Self { layers, delay_ms }
    }

    /// Bottom layer. Editing uses [`Self::layer_pixels`] so a higher layer is
    /// not mistaken for this buffer.
    pub fn pixels(&self) -> &[u8] {
        &self.layers[0]
    }

    pub(crate) fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub(crate) fn layer_pixels(&self, layer: usize) -> &[u8] {
        &self.layers[layer]
    }

    pub(crate) fn layers(&self) -> &[Vec<u8>] {
        &self.layers
    }

    pub(crate) fn delay_ms(&self) -> u32 {
        self.delay_ms
    }

    pub(crate) fn set_delay_ms(&mut self, delay_ms: u32) {
        self.delay_ms = delay_ms;
    }

    pub(crate) fn set_on(&mut self, layer: usize, offset: usize, value: u8) {
        self.layers[layer][offset] = value;
    }

    pub(crate) fn set_all_layers(&mut self, layers: Vec<Vec<u8>>) {
        debug_assert!(!layers.is_empty(), "a frame keeps at least one layer");
        self.layers = layers;
    }

    pub(crate) fn insert_layer(&mut self, index: usize, pixels: Vec<u8>) {
        self.layers.insert(index, pixels);
    }

    pub(crate) fn remove_layer(&mut self, index: usize) -> Vec<u8> {
        self.layers.remove(index)
    }

    pub(crate) fn contains_index(&self, index: u8) -> bool {
        self.layers.iter().any(|pixels| pixels.contains(&index))
    }

    /// Bottom-to-top index stack. Index 0 does not cover; any other index does.
    pub(crate) fn composite(&self) -> Vec<u8> {
        let mut out = self.layers[0].clone();
        for layer in self.layers.iter().skip(1) {
            for (dst, src) in out.iter_mut().zip(layer.iter()) {
                if *src != 0 {
                    *dst = *src;
                }
            }
        }
        out
    }
}
