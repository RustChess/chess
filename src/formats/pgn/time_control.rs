use core::{fmt, str};

use crate::formats::str_prelude::*;

use super::{Period, TimeControl};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid PGN time control")]
pub struct Error;

impl TimeControl {
    pub fn hourglass(seconds: u32) -> Self {
        Self::Periods(vec![Period::Hourglass { seconds }])
    }

    pub fn increment(seconds: u32, increment: u32) -> Self {
        Self::Periods(vec![Period::Increment { seconds, increment }])
    }

    pub fn moves(moves: u32, seconds: u32) -> Self {
        Self::Periods(vec![Period::Moves { moves, seconds }])
    }

    pub fn sudden_death(seconds: u32) -> Self {
        Self::Periods(vec![Period::SuddenDeath { seconds }])
    }
}

impl fmt::Display for TimeControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => f.write_str("?"),
            Self::None => f.write_str("-"),
            Self::Periods(periods) => {
                for (index, period) in periods.iter().enumerate() {
                    if index > 0 {
                        f.write_str(":")?;
                    }
                    period.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}

impl str::FromStr for TimeControl {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        time_control.parse(value).map_err(|_| Error)
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Moves { moves, seconds } => write!(f, "{moves}/{seconds}"),
            Self::SuddenDeath { seconds } => write!(f, "{seconds}"),
            Self::Increment { seconds, increment } => write!(f, "{seconds}+{increment}"),
            Self::Hourglass { seconds } => write!(f, "*{seconds}"),
        }
    }
}

pub fn time_control(input: &mut Input<'_>) -> ModalResult<TimeControl> {
    alt((
        '?'.value(TimeControl::Unknown),
        '-'.value(TimeControl::None),
        periods.map(TimeControl::Periods),
    ))
    .parse_next(input)
}

fn periods(input: &mut Input<'_>) -> ModalResult<Vec<Period>> {
    (repeat(0.., terminated(moves, ':')), period)
        .map(|(mut periods, last): (Vec<_>, _)| {
            periods.push(last);
            periods
        })
        .parse_next(input)
}

pub fn period(input: &mut Input<'_>) -> ModalResult<Period> {
    alt((moves, increment, hourglass, sudden_death)).parse_next(input)
}

fn moves(input: &mut Input<'_>) -> ModalResult<Period> {
    separated_pair(dec_uint, '/', dec_uint)
        .map(|(moves, seconds)| Period::Moves { moves, seconds })
        .parse_next(input)
}

fn sudden_death(input: &mut Input<'_>) -> ModalResult<Period> {
    dec_uint.map(|seconds| Period::SuddenDeath { seconds }).parse_next(input)
}

fn increment(input: &mut Input<'_>) -> ModalResult<Period> {
    separated_pair(dec_uint, '+', dec_uint)
        .map(|(seconds, increment)| Period::Increment { seconds, increment })
        .parse_next(input)
}

fn hourglass(input: &mut Input<'_>) -> ModalResult<Period> {
    preceded('*', dec_uint).map(|seconds| Period::Hourglass { seconds }).parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        for value in ["?", "-", "40/9000", "300", "600+0", "*180", "40/9000:3600+30"] {
            assert_eq!(value.parse::<TimeControl>().unwrap().to_string(), value);
        }

        assert!("300:40/9000".parse::<TimeControl>().is_err());
        assert!("600+0:300".parse::<TimeControl>().is_err());
        assert!("*180:300".parse::<TimeControl>().is_err());
    }
}
