/// One frame: a stack of palette-index buffers, plus how long it stays up.
///
/// Every layer has length `width * height`. Index 0 is transparent. A frame
/// written before layers existed is one buffer; the loader turns that into a
/// single layer. [`Self::composite`] stacks those buffers from bottom to top.
/// A layer the caller marks as skipped is left out. Index 0 does not cover,
/// except on a polish layer where the parallel erase mark is set: that cell
/// is explicit transparent and clears the composite. Any other index replaces
/// the cell. Palette alpha and partial layer opacity are not blended here.
/// RGBA export blends partial opacity afterwards and does not write a new index.
///
/// `delay_ms` is how long this frame stays up during loop playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    layers: Vec<Vec<u8>>,
    /// Parallel to [`Self::layers`]. `0` is unmodified. `1` on a polish layer
    /// is an explicit erase. Omitted from the file when every cell is `0`.
    erase: Vec<Vec<u8>>,
    delay_ms: u32,
}

impl Frame {
    pub(crate) fn blank(layer_count: usize, len: usize, delay_ms: u32) -> Self {
        debug_assert!(layer_count >= 1, "a frame keeps at least one layer");
        Self {
            layers: vec![vec![0; len]; layer_count],
            erase: vec![vec![0; len]; layer_count],
            delay_ms,
        }
    }

    pub(crate) fn from_layers(layers: Vec<Vec<u8>>, delay_ms: u32) -> Self {
        let erase = layers.iter().map(|pixels| vec![0; pixels.len()]).collect();
        Self::from_layers_with_erase(layers, erase, delay_ms)
    }

    pub(crate) fn from_layers_with_erase(
        layers: Vec<Vec<u8>>,
        erase: Vec<Vec<u8>>,
        delay_ms: u32,
    ) -> Self {
        debug_assert!(!layers.is_empty(), "a frame keeps at least one layer");
        debug_assert_eq!(layers.len(), erase.len(), "each layer has an erase mask");
        Self {
            layers,
            erase,
            delay_ms,
        }
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

    pub(crate) fn layer_erase(&self, layer: usize) -> &[u8] {
        &self.erase[layer]
    }

    pub(crate) fn layers(&self) -> &[Vec<u8>] {
        &self.layers
    }

    pub(crate) fn erasures(&self) -> &[Vec<u8>] {
        &self.erase
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

    pub(crate) fn set_erase(&mut self, layer: usize, offset: usize, value: u8) {
        self.erase[layer][offset] = value;
    }

    pub(crate) fn set_stack(&mut self, layers: Vec<Vec<u8>>, erase: Vec<Vec<u8>>) {
        debug_assert!(!layers.is_empty(), "a frame keeps at least one layer");
        debug_assert_eq!(layers.len(), erase.len(), "each layer has an erase mask");
        self.layers = layers;
        self.erase = erase;
    }

    pub(crate) fn insert_layer(&mut self, index: usize, pixels: Vec<u8>) {
        let erase = vec![0; pixels.len()];
        self.insert_cel(index, pixels, erase);
    }

    pub(crate) fn insert_cel(&mut self, index: usize, pixels: Vec<u8>, erase: Vec<u8>) {
        debug_assert_eq!(pixels.len(), erase.len(), "erase mask matches the buffer");
        self.layers.insert(index, pixels);
        self.erase.insert(index, erase);
    }

    pub(crate) fn remove_layer(&mut self, index: usize) -> (Vec<u8>, Vec<u8>) {
        let pixels = self.layers.remove(index);
        let erase = self.erase.remove(index);
        (pixels, erase)
    }

    pub(crate) fn contains_index(&self, index: u8) -> bool {
        self.layers.iter().any(|pixels| pixels.contains(&index))
    }

    /// Bottom-to-top index stack.
    ///
    /// `visible` and `opacity` match the layer stack. A hidden layer, or a
    /// layer whose opacity is 0, is skipped. Index 0 does not cover, unless
    /// `polish` is set for that layer and the erase mark is non-zero: that
    /// cell becomes transparent. Any other index replaces the cell, including
    /// when opacity is only partial: an index cannot store a blend.
    pub(crate) fn composite(&self, visible: &[bool], opacity: &[u8], polish: &[bool]) -> Vec<u8> {
        debug_assert_eq!(visible.len(), self.layers.len());
        debug_assert_eq!(opacity.len(), self.layers.len());
        debug_assert_eq!(polish.len(), self.layers.len());
        let mut out = vec![0u8; self.layers[0].len()];
        for (layer_index, (layer, erase)) in self.layers.iter().zip(self.erase.iter()).enumerate() {
            if !visible[layer_index] || opacity[layer_index] == 0 {
                continue;
            }
            let punch = polish[layer_index];
            for ((dst, src), mark) in out.iter_mut().zip(layer.iter()).zip(erase.iter()) {
                if *src != 0 {
                    *dst = *src;
                } else if punch && *mark != 0 {
                    *dst = 0;
                }
            }
        }
        out
    }
}
