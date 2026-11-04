//! Named font roles and their carrier definitions.
use assets::carriers::{self, Carrier};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OreUiFont {
    Seven,
    Ten,
    Five,
    FiveBold,
}

impl OreUiFont {
    // Optional custom typefaces are omitted from redistributable builds until their font
    // licensing is clear. OreUI uses the licensed Inter base face through its normal fallback.
    pub const ALL: [Self; 0] = [];

    /// Selects the shared carrier definition for this theme role.
    pub const fn carrier(self) -> &'static Carrier {
        match self {
            Self::Seven => &carriers::FONT_SEVEN,
            Self::Ten => &carriers::FONT_TEN,
            Self::Five => &carriers::FONT_FIVE,
            Self::FiveBold => &carriers::FONT_FIVE_BOLD,
        }
    }

    /// Returns the shipped family name used by the theme and font catalog.
    pub const fn name(self) -> &'static str {
        self.carrier().font_face.unwrap().name
    }

    /// Returns the catalog face used by first-party OreUI text.
    ///
    /// The optional Cinnangles faces are not shipped, so OreUI must select the licensed Inter
    /// fallback explicitly instead of relying on a missing-face lookup to fall back implicitly.
    pub const fn theme_name(self) -> &'static str {
        match self {
            Self::Seven | Self::Ten | Self::Five | Self::FiveBold => {
                carriers::FONT.font_face.unwrap().name
            }
        }
    }
}
