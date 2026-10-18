use crate::color::Rgba;
use crate::error::KernelError;
use crate::TRANSPARENT_INDEX;

/// Indexed color list. Entry 0 is always transparent.
///
/// Import, edit, and lock workflows from issue #4 are not implemented.
/// This type only stores the entries paint is allowed to use.
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

    /// Small built-in palette so a canvas can paint before issue #4.
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
    fn rejects_more_than_256_entries() {
        let mut colors = vec![Rgba::TRANSPARENT];
        colors.extend(std::iter::repeat(Rgba::rgb(1, 1, 1)).take(256));
        assert_eq!(
            Palette::new(colors).unwrap_err(),
            KernelError::PaletteTooLarge { len: 257 }
        );
    }
}
