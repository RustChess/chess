use std::str::FromStr;

use crate::position::Result;

crate::finite_set!(
    /// A supported chess variant.
    #[non_exhaustive]
    #[derive(Default)]
    #[cfg_attr(feature = "serde", derive(serde_with::DeserializeFromStr))]
    SupportedEnum,
    SupportedTable {
        #[default]
        Chess = 1 as chess,
        Freestyle = 2 as freestyle,
    }
);

crate::finite_set!(
    /// A supported chess variant or Unvalidated.
    #[non_exhaustive]
    VariantEnum,
    VariantTable {
        Unvalidated = 0 as unvalidated,
        Chess = 1 as chess,
        Freestyle = 2 as freestyle,
    }
);

impl From<SupportedEnum> for VariantEnum {
    fn from(supported: SupportedEnum) -> Self {
        supported.variant()
    }
}

impl SupportedEnum {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub const fn variant(self) -> VariantEnum {
        match self {
            SupportedEnum::Chess => VariantEnum::Chess,
            SupportedEnum::Freestyle => VariantEnum::Freestyle,
        }
    }
}

impl FromStr for SupportedEnum {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "chess" => Ok(Self::Chess),
            "freestyle" => Ok(Self::Freestyle),
            _ => Err(format!("expected chess or freestyle, got {value:?}")),
        }
    }
}

impl From<VariantEnum> for Option<SupportedEnum> {
    fn from(variant: VariantEnum) -> Self {
        variant.supported()
    }
}

impl VariantEnum {
    pub const fn supported(self) -> Option<SupportedEnum> {
        match self {
            VariantEnum::Unvalidated => None,
            VariantEnum::Chess => Some(SupportedEnum::Chess),
            VariantEnum::Freestyle => Some(SupportedEnum::Freestyle),
        }
    }
}

// Trait-side equivalent to the idea that Variant = Supported | Unvalidated.
pub trait Supported: Validate {
    const SUPPORTED: SupportedEnum;

    fn is_chess() -> bool {
        matches!(Self::SUPPORTED, SupportedEnum::Chess)
    }

    fn is_freestyle() -> bool {
        matches!(Self::SUPPORTED, SupportedEnum::Freestyle)
    }
}

pub trait Variant: Copy + Sized {
    /// The variant enum value for this variant.
    ///
    /// This serves as the "universal bridge" from implemenations of
    /// the `Variant` trait to an enum value.
    const VARIANT: VariantEnum;

    fn validate(position: crate::Position<Unvalidated>) -> Result<crate::Position<Self>>;
}

pub trait Validate: Variant {
    fn validate_castling(position: crate::Position<Unvalidated>) -> Result<()>;
}

/// A universal receiver for any supported or unvalidated chess game.
///
/// When parsing from PGN, we try to infer the supported variant,
/// otherwise report which we tried and fall back to unvalidated.
#[non_exhaustive]
#[derive(Clone, PartialEq)]
pub enum Game<Error = ()> {
    Chess(crate::Game<Chess>),
    Freestyle(crate::Game<Freestyle>),
    Unvalidated { game: crate::Game<Unvalidated>, error: Error },
}

#[non_exhaustive]
#[derive(Clone, Copy, PartialEq)]
pub enum Position {
    Chess(crate::Position<Chess>),
    Freestyle(crate::Position<Freestyle>),
}

impl From<crate::Position<Chess>> for Position {
    fn from(position: crate::Position<Chess>) -> Self {
        Self::Chess(position)
    }
}

impl From<crate::Position<Freestyle>> for Position {
    fn from(position: crate::Position<Freestyle>) -> Self {
        Self::Freestyle(position)
    }
}

impl<Error> Game<Error> {
    pub fn variant(&self) -> VariantEnum {
        match self {
            Game::Chess(_) => VariantEnum::Chess,
            Game::Freestyle(_) => VariantEnum::Freestyle,
            Game::Unvalidated { .. } => VariantEnum::Unvalidated,
        }
    }

    pub fn supported(&self) -> Option<SupportedEnum> {
        self.variant().supported()
    }

    pub fn is_chess(&self) -> bool {
        matches!(self, Game::Chess(_))
    }

    pub fn chess(self) -> Option<crate::Game<Chess>> {
        match self {
            Game::Chess(game) => Some(game),
            _ => None,
        }
    }

    pub fn is_freestyle(&self) -> bool {
        matches!(self, Game::Freestyle(_))
    }

    pub fn freestyle(self) -> Option<crate::Game<Freestyle>> {
        match self {
            Game::Freestyle(game) => Some(game),
            _ => None,
        }
    }

    pub fn is_unvalidated(&self) -> bool {
        matches!(self, Game::Unvalidated { .. })
    }

    pub fn unvalidated(self) -> crate::Game<Unvalidated> {
        match self {
            Game::Chess(game) => game.unvalidated(),
            Game::Freestyle(game) => game.unvalidated(),
            Game::Unvalidated { game, .. } => game,
        }
    }

    pub fn error(&self) -> Option<&Error> {
        match self {
            Game::Unvalidated { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Chess;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Freestyle;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Unvalidated;

impl Variant for Chess {
    const VARIANT: VariantEnum = SupportedEnum::Chess.variant();

    fn validate(position: crate::Position<Unvalidated>) -> Result<crate::Position<Self>> {
        position.validate()
    }
}

impl Supported for Chess {
    const SUPPORTED: SupportedEnum = SupportedEnum::Chess;
}

impl Variant for Freestyle {
    const VARIANT: VariantEnum = SupportedEnum::Freestyle.variant();

    fn validate(position: crate::Position<Unvalidated>) -> Result<crate::Position<Self>> {
        position.validate()
    }
}

impl Supported for Freestyle {
    const SUPPORTED: SupportedEnum = SupportedEnum::Freestyle;
}

impl Variant for Unvalidated {
    const VARIANT: VariantEnum = VariantEnum::Unvalidated;

    fn validate(position: crate::Position<Unvalidated>) -> Result<crate::Position<Self>> {
        Ok(position)
    }
}

impl Validate for Chess {
    fn validate_castling(position: crate::Position<Unvalidated>) -> Result<()> {
        crate::Position::<Self>::validate_castling(position)
    }
}

impl Validate for Freestyle {
    fn validate_castling(position: crate::Position<Unvalidated>) -> Result<()> {
        crate::Position::<Self>::validate_castling(position)
    }
}
