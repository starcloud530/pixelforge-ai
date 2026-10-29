/// One palette entry. Index 0 in a [`crate::Palette`] is fully transparent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Chebyshev distance over `r`, `g`, `b`, and `a`.
    ///
    /// A connected-component selection uses this so tolerance stays on the
    /// stored palette entry and does not invent a new color.
    pub fn channel_distance(self, other: Self) -> u8 {
        self.r
            .abs_diff(other.r)
            .max(self.g.abs_diff(other.g))
            .max(self.b.abs_diff(other.b))
            .max(self.a.abs_diff(other.a))
    }
}
