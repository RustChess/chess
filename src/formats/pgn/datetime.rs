use core::{fmt, str};

use crate::formats::str_prelude::*;

use super::{Date, Time};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("invalid PGN date")]
    Date,
    #[error("invalid PGN time")]
    Time,
}

impl Date {
    pub fn date(self) -> Option<time::Date> {
        let year = number(self.year)?;
        let month = time::Month::try_from(number(self.month)? as u8).ok()?;
        let day = number(self.day)? as u8;
        time::Date::from_calendar_date(year, month, day).ok()
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_digits(f, self.year)?;
        f.write_str(".")?;
        fmt_digits(f, self.month)?;
        f.write_str(".")?;
        fmt_digits(f, self.day)
    }
}

impl str::FromStr for Date {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        date.parse(value).map_err(|_| Error::Date)
    }
}

impl Time {
    pub fn time(self) -> Option<time::Time> {
        let hour = number(self.hour)? as u8;
        let minute = number(self.minute)? as u8;
        let second = number(self.second)? as u8;
        time::Time::from_hms(hour, minute, second).ok()
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_digits(f, self.hour)?;
        f.write_str(":")?;
        fmt_digits(f, self.minute)?;
        f.write_str(":")?;
        fmt_digits(f, self.second)
    }
}

impl str::FromStr for Time {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        time.parse(value).map_err(|_| Error::Time)
    }
}

pub fn date(input: &mut Input<'_>) -> ModalResult<Date> {
    seq! {Date {
        year: digits::<4>,
        _: '.',
        month: digits::<2>,
        _: '.',
        day: digits::<2>,
    }}
    .parse_next(input)
}

pub fn time(input: &mut Input<'_>) -> ModalResult<Time> {
    seq! {Time {
        hour: digits::<2>,
        _: ':',
        minute: digits::<2>,
        _: ':',
        second: digits::<2>,
    }}
    .parse_next(input)
}

fn digits<const N: usize>(input: &mut Input<'_>) -> ModalResult<[Option<u8>; N]> {
    take(N)
        .verify(|digits: &str| digits.bytes().all(|digit| digit == b'?' || digit.is_ascii_digit()))
        .map(|digits: &str| {
            let digits = digits.as_bytes();
            core::array::from_fn(|index| match digits[index] {
                b'?' => None,
                digit => Some(digit - b'0'),
            })
        })
        .parse_next(input)
}

fn fmt_digits<const N: usize>(f: &mut fmt::Formatter<'_>, digits: [Option<u8>; N]) -> fmt::Result {
    for digit in digits {
        match digit {
            Some(digit) => write!(f, "{digit}")?,
            None => f.write_str("?")?,
        }
    }
    Ok(())
}

fn number<const N: usize>(digits: [Option<u8>; N]) -> Option<i32> {
    digits.into_iter().try_fold(0, |number, digit| Some(number * 10 + i32::from(digit?)))
}
