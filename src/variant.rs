use crate::position::{Position, Result};

pub trait Variant: Copy + Sized {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Chess;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Freestyle;

impl Variant for Chess {}

impl Variant for Freestyle {}

pub trait Supported: Validate {}

impl<T: Validate> Supported for T {}

pub trait Validate: Variant {
    fn validate_castling(position: Position<Unvalidated>) -> Result<()>;
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Unvalidated;

impl Variant for Unvalidated {}
