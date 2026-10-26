use core::{fmt, ops};

use crate::{
    Square,
    formats::str_prelude::*,
    game::{Arrow, Color, Marker},
};

use super::{Command, Eval};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Time {
    pub hours: u32,
    pub minutes: u8,
    pub seconds: u8,
    pub centiseconds: u8,
}

impl Time {
    pub fn from_centiseconds(centiseconds: u32) -> Self {
        let seconds = centiseconds / 100;
        Self {
            hours: seconds / 3600,
            minutes: ((seconds / 60) % 60) as u8,
            seconds: (seconds % 60) as u8,
            centiseconds: (centiseconds % 100) as u8,
        }
    }

    pub fn centiseconds(&self) -> Option<u32> {
        self.hours
            .checked_mul(3600)?
            .checked_add(u32::from(self.minutes) * 60)?
            .checked_add(u32::from(self.seconds))?
            .checked_mul(100)?
            .checked_add(u32::from(self.centiseconds))
    }
}

macro_rules! time_command {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub Time);

        impl ops::Deref for $name {
            type Target = Time;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

// clk
time_command!(Clock);
// egt = elapsed game time
time_command!(ElapsedGame);
// emt = elapsed move time
time_command!(ElapsedMove);
// mct = mechanical clock time
time_command!(MechanicalClock);

// cal = colored arrow list
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ColoredArrows(pub Vec<Arrow>);

// csl = colored square list
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ColoredSquares(pub Vec<Marker>);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Other {
    pub command: Text,
    pub parameters: Vec<String>,
}

impl ops::Deref for ColoredArrows {
    type Target = [Arrow];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::Deref for ColoredSquares {
    type Target = [Marker];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Command {
    pub fn from_other(other: Other) -> Self {
        let typed = match other.command.as_ref() {
            "cal" => colored_arrows(&other.parameters).map(Self::ColoredArrows),
            "csl" => colored_squares(&other.parameters).map(Self::ColoredSquares),
            "clk" => elapsed(&other.parameters).map(|time| Self::Clock(Clock(time))),
            "egt" => elapsed(&other.parameters).map(|time| Self::ElapsedGame(ElapsedGame(time))),
            "emt" => elapsed(&other.parameters).map(|time| Self::ElapsedMove(ElapsedMove(time))),
            "eval" => evaluation(&other.parameters).map(Self::Evaluation),
            "exp" if other.parameters.as_slice() == ["true"] => Some(Self::Expanded),
            "mct" => mechanical(&other.parameters)
                .map(|time| Self::MechanicalClock(MechanicalClock(time))),
            _ => None,
        };
        typed.unwrap_or(Self::Other(other))
    }

    pub fn into_other(self) -> Option<Other> {
        use Command::*;

        let (command, parameters) = match self {
            Clock(value) => ("clk", vec![value.0.to_string()]),
            ColoredArrows(_) | ColoredSquares(_) | Expanded => return None,
            ElapsedGame(value) => ("egt", vec![value.0.to_string()]),
            ElapsedMove(value) => ("emt", vec![value.0.to_string()]),
            Evaluation(value) => ("eval", vec![value.to_string()]),
            MechanicalClock(value) => ("mct", vec![mechanical_text(value.0)]),
            Other(other) => return Some(other),
        };
        Some(self::Other {
            command: Text::new(command).expect("PGN command names are non-empty"),
            parameters,
        })
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Command::*;

        match self {
            Clock(value) => write!(f, "[%clk {}]", value.0),
            ColoredArrows(arrows) => list(f, "cal", &arrows.0, arrow),
            ColoredSquares(markers) => list(f, "csl", &markers.0, marker),
            ElapsedGame(value) => write!(f, "[%egt {}]", value.0),
            ElapsedMove(value) => write!(f, "[%emt {}]", value.0),
            Evaluation(value) => write!(f, "[%eval {value}]"),
            Expanded => f.write_str("[%exp true]"),
            MechanicalClock(value) => write!(f, "[%mct {}]", mechanical_text(value.0)),
            Other(other) => other.fmt(f),
        }
    }
}

impl fmt::Display for Other {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[%{}", self.command)?;
        for (index, parameter) in self.parameters.iter().enumerate() {
            f.write_str(if index == 0 { " " } else { "," })?;
            f.write_str(parameter)?;
        }
        f.write_str("]")
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{:02}:{:02}", self.hours, self.minutes, self.seconds)?;
        Fraction(self.centiseconds).fmt(f)
    }
}

struct Fraction(u8);

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            0 => Ok(()),
            centiseconds if centiseconds % 10 == 0 => write!(f, ".{}", centiseconds / 10),
            centiseconds => write!(f, ".{centiseconds:02}"),
        }
    }
}

fn colored_arrows(parameters: &[String]) -> Option<ColoredArrows> {
    (!parameters.is_empty())
        .then(|| parameters.iter().map(|value| colored_arrow.parse(value.as_str()).ok()).collect())
        .flatten()
        .map(ColoredArrows)
}

fn colored_squares(parameters: &[String]) -> Option<ColoredSquares> {
    (!parameters.is_empty())
        .then(|| parameters.iter().map(|value| colored_square.parse(value.as_str()).ok()).collect())
        .flatten()
        .map(ColoredSquares)
}

fn elapsed(parameters: &[String]) -> Option<Time> {
    let [value] = parameters else { return None };
    elapsed_time.parse(value.as_str()).ok()
}

fn mechanical(parameters: &[String]) -> Option<Time> {
    let [value] = parameters else { return None };
    mechanical_time.parse(value.as_str()).ok()
}

fn evaluation(parameters: &[String]) -> Option<Eval> {
    (1..=2).contains(&parameters.len()).then(|| parameters.join(",").parse().ok()).flatten()
}

pub fn colored_arrow(input: &mut Input<'_>) -> ModalResult<Arrow> {
    seq! {Arrow {
        color: color.map(Some),
        from: square,
        to: square,
    }}
    .parse_next(input)
}

pub fn colored_square(input: &mut Input<'_>) -> ModalResult<Marker> {
    seq! {Marker {
        color: color.map(Some),
        square: square,
    }}
    .parse_next(input)
}

pub fn color(input: &mut Input<'_>) -> ModalResult<Color> {
    alt((
        'R'.value(Color::Red),
        'G'.value(Color::Green),
        'B'.value(Color::Blue),
        'Y'.value(Color::Yellow),
    ))
    .parse_next(input)
}

fn square(input: &mut Input<'_>) -> ModalResult<Square> {
    take(2_usize).try_map(str::parse).parse_next(input)
}

pub fn elapsed_time(input: &mut Input<'_>) -> ModalResult<Time> {
    seq! {Time {
        hours: dec_uint,
        _: ':',
        minutes: two_digits.verify(|value| *value < 60),
        _: ':',
        seconds: two_digits.verify(|value| *value < 60),
        centiseconds: centiseconds,
    }}
    .parse_next(input)
}

pub fn mechanical_time(input: &mut Input<'_>) -> ModalResult<Time> {
    seq! {Time {
        hours: two_digits.map(u32::from),
        _: ':',
        minutes: two_digits.verify(|value| *value < 60),
        _: ':',
        seconds: two_digits.verify(|value| *value < 60),
        centiseconds: centiseconds,
    }}
    .parse_next(input)
}

fn two_digits(input: &mut Input<'_>) -> ModalResult<u8> {
    take(2_usize).try_map(str::parse).parse_next(input)
}

fn centiseconds(input: &mut Input<'_>) -> ModalResult<u8> {
    Ok(opt(preceded('.', take_while(1..=2, |c: char| c.is_ascii_digit())))
        .parse_next(input)?
        .map(|value: &str| {
            let digits = value.len();
            let value: u8 = value.parse().expect("one or two digits fit in u8");
            if digits == 1 { value * 10 } else { value }
        })
        .unwrap_or_default())
}

fn list<T>(
    f: &mut fmt::Formatter<'_>,
    command: &str,
    values: &[T],
    write: impl Fn(&mut fmt::Formatter<'_>, &T) -> fmt::Result,
) -> fmt::Result {
    write!(f, "[%{command} ")?;
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            f.write_str(",")?;
        }
        write(f, value)?;
    }
    f.write_str("]")
}

fn arrow(f: &mut fmt::Formatter<'_>, arrow: &Arrow) -> fmt::Result {
    write!(f, "{}{}{}", color_code(arrow.color), arrow.from, arrow.to)
}

fn marker(f: &mut fmt::Formatter<'_>, marker: &Marker) -> fmt::Result {
    write!(f, "{}{}", color_code(marker.color), marker.square)
}

fn color_code(color: Option<Color>) -> char {
    match color {
        None | Some(Color::Red) => 'R',
        Some(Color::Green) => 'G',
        Some(Color::Blue) => 'B',
        Some(Color::Yellow) => 'Y',
    }
}

fn mechanical_text(time: Time) -> String {
    format!(
        "{:02}:{:02}:{:02}{}",
        time.hours,
        time.minutes,
        time.seconds,
        Fraction(time.centiseconds)
    )
}
