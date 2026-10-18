use crate::color::Rgba;
use crate::error::KernelError;
use crate::TRANSPARENT_INDEX;

/// Indexed color list. Entry 0 is always transparent.
///
/// Paint entries can be recolored or extended. Import and locking from
/// issue #4 are not implemented. Pixels store indices into this list, so
/// replacing an entry changes every cell that uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    colors: Vec<Rgba>,
}

impl Palette {
    /// Builds a palette. `colors[0]` must be [`Rgba::TRANSPARENT`].
    pub fn new(colors: Vec<Rgba>) -> Result<Self, KernelError> {
        if colors.is_empty() {
            return Err(KernelError::EmptyPalette);
        }
        if colors.len() > usize::from(u8::MAX) + 1 {
            return Err(KernelError::PaletteTooLarge { len: colors.len() });
        }
        if colors[0] != Rgba::TRANSPARENT {
            return Err(KernelError::TransparentSlot);
        }
        Ok(Self { colors })
    }

    /// Small built-in palette used until a project file loads another one.
    ///
    /// The browser reads these entries from the wasm bridge. It does not
    /// keep a second copy.
    pub fn studio_default() -> Self {
        Self::new(vec![
            Rgba::TRANSPARENT,
            Rgba::rgb(26, 28, 36),
            Rgba::rgb(244, 241, 222),
            Rgba::rgb(232, 93, 76),
            Rgba::rgb(244, 162, 89),
            Rgba::rgb(242, 204, 102),
            Rgba::rgb(106, 176, 106),
            Rgba::rgb(74, 140, 196),
            Rgba::rgb(140, 110, 184),
        ])
        .expect("studio palette keeps index 0 transparent")
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    pub fn colors(&self) -> &[Rgba] {
        &self.colors
    }

    pub fn get(&self, index: u8) -> Option<Rgba> {
        self.colors.get(usize::from(index)).copied()
    }

    /// True when `index` refers to an opaque entry paint may write.
    pub fn contains_paint_index(&self, index: u8) -> bool {
        index != TRANSPARENT_INDEX && usize::from(index) < self.colors.len()
    }

    /// Replaces the RGB of a paint entry. Alpha is stored as opaque.
    ///
    /// Index 0 stays transparent. Returns the color that was replaced.
    pub fn set_rgb(&mut self, index: u8, r: u8, g: u8, b: u8) -> Result<Rgba, KernelError> {
        if index == TRANSPARENT_INDEX {
            return Err(KernelError::TransparentSlot);
        }
        let slot = self
            .colors
            .get_mut(usize::from(index))
            .ok_or(KernelError::PaletteIndex { index })?;
        let previous = *slot;
        *slot = Rgba::rgb(r, g, b);
        Ok(previous)
    }

    /// Appends an opaque color and returns its index.
    pub fn push_rgb(&mut self, r: u8, g: u8, b: u8) -> Result<u8, KernelError> {
        if self.colors.len() >= usize::from(u8::MAX) + 1 {
            return Err(KernelError::PaletteTooLarge {
                len: self.colors.len() + 1,
            });
        }
        let index = u8::try_from(self.colors.len()).expect("length was checked against u8");
        self.colors.push(Rgba::rgb(r, g, b));
        Ok(index)
    }

    pub(crate) fn write(&mut self, index: u8, color: Rgba) {
        self.colors[usize::from(index)] = color;
    }

    pub(crate) fn pop(&mut self) -> Rgba {
        debug_assert!(self.colors.len() > 1, "the transparent slot stays");
        self.colors
            .pop()
            .expect("palette keeps the transparent slot")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_zero_is_transparent() {
        let palette = Palette::studio_default();
        assert!(!palette.is_empty());
        assert_eq!(palette.len(), 9);
        assert_eq!(palette.get(0), Some(Rgba::TRANSPARENT));
        assert!(!palette.contains_paint_index(0));
        assert!(palette.contains_paint_index(1));
        assert!(!palette.contains_paint_index(9));
    }

    #[test]
    fn rejects_empty_and_opaque_slot_zero() {
        assert_eq!(Palette::new(vec![]).unwrap_err(), KernelError::EmptyPalette);
        assert_eq!(
            Palette::new(vec![Rgba::rgb(1, 2, 3)]).unwrap_err(),
            KernelError::TransparentSlot
        );
    }

    #[test]
    fn set_rgb_replaces_a_paint_entry() {
        let mut palette = Palette::studio_default();
        let previous = palette.set_rgb(1, 9, 8, 7).unwrap();
        assert_eq!(previous, Rgba::rgb(26, 28, 36));
        assert_eq!(palette.get(1), Some(Rgba::rgb(9, 8, 7)));
        assert_eq!(palette.get(0), Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn set_rgb_rejects_transparent_and_missing_slots() {
        let mut palette = Palette::studio_default();
        assert_eq!(
            palette.set_rgb(0, 1, 2, 3).unwrap_err(),
            KernelError::TransparentSlot
        );
        assert_eq!(palette.get(0), Some(Rgba::TRANSPARENT));
        assert_eq!(
            palette.set_rgb(9, 1, 2, 3).unwrap_err(),
            KernelError::PaletteIndex { index: 9 }
        );
        assert_eq!(palette.len(), 9);
    }

    #[test]
    fn push_rgb_appends_until_256() {
        let mut palette = Palette::studio_default();
        assert_eq!(palette.push_rgb(1, 2, 3).unwrap(), 9);
        assert_eq!(palette.get(9), Some(Rgba::rgb(1, 2, 3)));
        while palette.len() < 256 {
            palette.push_rgb(4, 5, 6).unwrap();
        }
        assert_eq!(palette.len(), 256);
        assert!(palette.contains_paint_index(255));
        assert_eq!(
            palette.push_rgb(7, 8, 9).unwrap_err(),
            KernelError::PaletteTooLarge { len: 257 }
        );
    }

    #[test]
    fn rejects_more_than_256_entries() {
        let mut colors = vec![Rgba::TRANSPARENT];
        colors.extend(std::iter::repeat(Rgba::rgb(1, 1, 1)).take(256));
        assert_eq!(
            Palette::new(colors).unwrap_err(),
            KernelError::PaletteTooLarge { len: 257 }
        );
    }
}
