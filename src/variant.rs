use crate::position::{Position, Result, SupportedEnum, VariantEnum};

pub trait Variant: Copy + Sized {
    /// The variant enum value for this variant.
    ///
    /// This serves as the "universal bridge" from implemenations of
    /// the `Variant` trait to an enum value.
    const VARIANT: VariantEnum;
}

pub trait Validate: Variant {
    fn validate_castling(position: Position<Unvalidated>) -> Result<()>;
}

// Really, there should be a condition that SUPPORTED
// must map to its corresponding VARIANT.
pub trait Supported: Validate {
    const SUPPORTED: SupportedEnum;
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Chess;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Freestyle;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Unvalidated;

impl Variant for Chess {
    const VARIANT: VariantEnum = VariantEnum::Chess;
}

impl Supported for Chess {
    const SUPPORTED: SupportedEnum = SupportedEnum::Chess;
}

impl Variant for Freestyle {
    const VARIANT: VariantEnum = VariantEnum::Freestyle;
}

impl Supported for Freestyle {
    const SUPPORTED: SupportedEnum = SupportedEnum::Freestyle;
}

impl Variant for Unvalidated {
    const VARIANT: VariantEnum = VariantEnum::Unvalidated;
}

impl Validate for Chess {
    fn validate_castling(position: Position<Unvalidated>) -> Result<()> {
        Position::<Self>::validate_castling(position)
    }
}

impl Validate for Freestyle {
    fn validate_castling(position: Position<Unvalidated>) -> Result<()> {
        Position::<Self>::validate_castling(position)
    }
}
