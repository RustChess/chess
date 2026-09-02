use crate::position::{Position, Result, VariantEnum as Enum};

pub trait Variant: Copy + Sized {
    /// The variant enum value for this variant.
    ///
    /// This serves as the "universal bridge" from implemenations of
    /// the `Variant` trait to an enum value.
    const VARIANT: Enum;
}

pub trait Validate: Variant {
    fn validate_castling(position: Position<Unvalidated>) -> Result<()>;
}

// Commented out as unused - if we ever grow more traits like Validate,
// this would be the combination of them - a variant the is supported
// would have "core/full support".
//
// pub trait Supported: Validate {}
// impl<T: Validate> Supported for T {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Chess;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Freestyle;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Unvalidated;

impl Variant for Chess {
    const VARIANT: Enum = Enum::Chess;
}

impl Variant for Freestyle {
    const VARIANT: Enum = Enum::Freestyle;
}

impl Variant for Unvalidated {
    const VARIANT: Enum = Enum::Unvalidated;
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
