use core::{fmt, str};

use crate::formats::str_prelude::*;

use super::Tag;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid ECO code")]
pub struct Error;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Eco {
    volume: char,
    number: u8,
    suffix: Option<Text>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Other {
    pub key: Text,
    pub value: String,
}

fn other(key: Text, value: String) -> Tag {
    Tag::Other(Other { key, value })
}

impl Tag {
    pub fn freestyle() -> Self {
        use Tag::*;
        Variant("Fischerandom".to_string())
    }

    pub fn variant(&self) -> Option<&str> {
        use Tag::*;
        if let Variant(value) = self { Some(value) } else { None }
    }

    pub fn name(&self) -> &str {
        use Tag::*;
        match self {
            Title(_) => "Title",
            Annotator(_) => "Annotator",
            Event(_) => "Event",
            EventDate(_) => "EventDate",
            EventType(_) => "EventType",
            EventCategory(_) => "EventCategory",
            EventRounds(_) => "EventRounds",
            EventCountry(_) => "EventCountry",
            Id(_) => "Id",
            Site(_) => "Site",
            Date(_) => "Date",
            EndDate(_) => "EndDate",
            EndTime(_) => "EndTime",
            Time(_) => "Time",
            TimeControl(_) => "TimeControl",
            Timezone(_) => "Timezone",
            UtcDate(_) => "UTCDate",
            UtcTime(_) => "UTCTime",
            Round(_) => "Round",
            White(_) => "White",
            WhiteName(_) => "WhiteName",
            WhiteTitle(_) => "WhiteTitle",
            WhiteElo(_) => "WhiteElo",
            WhiteFideId(_) => "WhiteFideId",
            WhiteTeam(_) => "WhiteTeam",
            WhiteTeamCountry(_) => "WhiteTeamCountry",
            Black(_) => "Black",
            BlackName(_) => "BlackName",
            BlackTitle(_) => "BlackTitle",
            BlackElo(_) => "BlackElo",
            BlackFideId(_) => "BlackFideId",
            BlackTeam(_) => "BlackTeam",
            BlackTeamCountry(_) => "BlackTeamCountry",
            Outcome(_) => "Result",
            Rated(_) => "Rated",
            Termination(_) => "Termination",
            Eco(_) => "ECO",
            Fen(_) => "FEN",
            SetUp(_) => "SetUp",
            Variant(_) => "Variant",
            Chess960Id(_) => "Chess960Id",
            Orientation(_) => "Orientation",
            TimeType(_) => "TimeType",
            Source(_) => "Source",
            SourceDate(_) => "SourceDate",
            Publisher(_) => "Publisher",
            Valuation(_) => "Valuation",
            Other(tag) => tag.key.as_ref(),
        }
    }

    pub(super) fn from_pair(key: Text, value: String) -> Option<Self> {
        use Tag::*;

        fn text(value: String, tag: impl FnOnce(Text) -> Tag) -> Option<Tag> {
            Text::new(value).ok().map(tag)
        }

        fn parse<T: str::FromStr>(key: Text, value: String, tag: impl FnOnce(T) -> Tag) -> Tag {
            match value.parse() {
                Ok(value) => tag(value),
                Err(_) => other(key, value),
            }
        }

        Some(match key.as_ref() {
            "Title" => Title(value),
            "Annotator" => return text(value, Annotator),
            "Event" => Event(value),
            "EventDate" => parse(key, value, EventDate),
            "EventType" => return text(value, EventType),
            "EventCategory" => return text(value, EventCategory),
            "EventRounds" => return text(value, EventRounds),
            "EventCountry" => return text(value, EventCountry),
            "Id" => return text(value, Id),
            "Site" => Site(value),
            "Date" => Date(value.parse().ok()?),
            "EndDate" => parse(key, value, EndDate),
            "EndTime" => parse(key, value, EndTime),
            "Time" => Time(value.parse().ok()?),
            "TimeControl" => parse(key, value, TimeControl),
            "Timezone" => return text(value, Timezone),
            "UTCDate" => UtcDate(value.parse().ok()?),
            "UTCTime" => UtcTime(value.parse().ok()?),
            "Round" => Round(value),
            "White" => White(value),
            "WhiteName" => return text(value, WhiteName),
            "WhiteTitle" => return text(value, WhiteTitle),
            "WhiteElo" => parse(key, value, WhiteElo),
            "WhiteFideId" => parse(key, value, WhiteFideId),
            "WhiteTeam" => return text(value, WhiteTeam),
            "WhiteTeamCountry" => return text(value, WhiteTeamCountry),
            "Black" => Black(value),
            "BlackName" => return text(value, BlackName),
            "BlackTitle" => return text(value, BlackTitle),
            "BlackElo" => parse(key, value, BlackElo),
            "BlackFideId" => parse(key, value, BlackFideId),
            "BlackTeam" => return text(value, BlackTeam),
            "BlackTeamCountry" => return text(value, BlackTeamCountry),
            "Result" => Outcome(if value == "*" {
                None
            } else {
                Some(crate::game::Outcome::from_result(&value)?)
            }),
            "Rated" => parse(key, value, Rated),
            "Termination" => return text(value, Termination),
            "ECO" => parse(key, value, Eco),
            "FEN" => Fen(crate::formats::fen::parts.parse(value.as_str()).ok()?),
            "SetUp" => SetUp(match value.as_str() {
                "0" => false,
                "1" => true,
                _ => return None,
            }),
            "Variant" => Variant(value),
            "Orientation" => Orientation(value.parse().ok()?),
            "TimeType" => return text(value, TimeType),
            "Source" => return text(value, Source),
            "SourceDate" => parse(key, value, SourceDate),
            "Publisher" => return text(value, Publisher),
            "Valuation" => parse(key, value, Valuation),
            _ => other(key, value),
        })
    }

    pub(super) fn metadata(&self) -> Option<Other> {
        let value = {
            use Tag::*;
            match self {
                EventDate(value) | EndDate(value) | SourceDate(value) | UtcDate(value) => {
                    value.to_string()
                }
                EndTime(value) | Time(value) | UtcTime(value) => value.to_string(),
                Annotator(value)
                | EventType(value)
                | EventCategory(value)
                | EventRounds(value)
                | EventCountry(value)
                | Id(value)
                | WhiteName(value)
                | WhiteTitle(value)
                | WhiteTeam(value)
                | WhiteTeamCountry(value)
                | BlackName(value)
                | BlackTitle(value)
                | BlackTeam(value)
                | BlackTeamCountry(value)
                | Termination(value)
                | TimeType(value)
                | Timezone(value)
                | Source(value)
                | Publisher(value) => value.to_string(),
                WhiteElo(value) | BlackElo(value) => value.to_string(),
                WhiteFideId(value) | BlackFideId(value) => value.to_string(),
                Rated(value) => value.to_string(),
                Eco(value) => value.to_string(),
                _ => return None,
            }
        };
        Some(Other { key: Text::new(self.name()).expect("PGN tag names are non-empty"), value })
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Tag::*;
        let value = match self {
            Title(value) | Event(value) | Site(value) | Round(value) | White(value)
            | Black(value) | Variant(value) => value.clone(),
            EventDate(value) | Date(value) | EndDate(value) | UtcDate(value)
            | SourceDate(value) => value.to_string(),
            Annotator(value)
            | EventType(value)
            | EventCategory(value)
            | EventRounds(value)
            | EventCountry(value)
            | Id(value)
            | WhiteName(value)
            | WhiteTitle(value)
            | WhiteTeam(value)
            | WhiteTeamCountry(value)
            | BlackName(value)
            | BlackTitle(value)
            | BlackTeam(value)
            | BlackTeamCountry(value)
            | Termination(value)
            | TimeType(value)
            | Timezone(value)
            | Source(value)
            | Publisher(value) => value.to_string(),
            EndTime(value) | Time(value) | UtcTime(value) => value.to_string(),
            TimeControl(value) => value.to_string(),
            WhiteElo(value) | BlackElo(value) => value.to_string(),
            WhiteFideId(value) | BlackFideId(value) => value.to_string(),
            Outcome(outcome) => {
                outcome.map(crate::game::Outcome::result).unwrap_or("*").to_string()
            }
            Rated(value) => value.to_string(),
            Eco(value) => value.to_string(),
            Fen(position) => position.fen(),
            SetUp(setup) => if *setup { "1" } else { "0" }.to_string(),
            Chess960Id(id) => id.to_string(),
            Orientation(player) => player.name().to_string(),
            Valuation(value) => value.to_string(),
            Other(tag) => return write_tag(f, tag.key.as_ref(), &tag.value),
        };
        write_tag(f, self.name(), &value)
    }
}

impl fmt::Display for Eco {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:02}", self.volume, self.number)?;
        if let Some(suffix) = &self.suffix {
            suffix.fmt(f)?;
        }
        Ok(())
    }
}

impl str::FromStr for Eco {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        eco.parse(value).map_err(|_| Error)
    }
}

fn eco(input: &mut Input<'_>) -> ModalResult<Eco> {
    let volume = one_of('A'..='E').parse_next(input)?;
    let number = take(2u8)
        .verify(|digits: &str| digits.bytes().all(|digit| digit.is_ascii_digit()))
        .map(|digits: &str| digits.parse().expect("two ASCII digits fit in u8"))
        .parse_next(input)?;
    let suffix = opt(take_while(1.., |c: char| !c.is_whitespace()).map(|suffix: &str| {
        Text::new(suffix).expect("the suffix parser requires at least one character")
    }))
    .parse_next(input)?;
    Ok(Eco { volume, number, suffix })
}

pub(super) fn write_tag(f: &mut fmt::Formatter<'_>, key: &str, value: &str) -> fmt::Result {
    let value = value.replace('\\', "\\\\").replace('"', "\\\"");
    write!(f, "[{key} \"{value}\"]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Text {
        Text::new(value).unwrap()
    }

    #[test]
    fn metadata() {
        use Tag::*;

        let tags = [
            ("Annotator", "Schepler,Aaron", Annotator(text("Schepler,Aaron"))),
            ("EventDate", "2025.10.19", EventDate("2025.10.19".parse().unwrap())),
            ("EventType", "team", EventType(text("team"))),
            ("EventCountry", "GRE", EventCountry(text("GRE"))),
            ("WhiteElo", "2596", WhiteElo(2596)),
            ("WhiteFideId", "4300033", WhiteFideId(4_300_033)),
            ("ECO", "E04b", Eco("E04b".parse().unwrap())),
            ("Source", "The Week in Chess 1616", Source(text("The Week in Chess 1616"))),
            ("SourceDate", "2025.10.27", SourceDate("2025.10.27".parse().unwrap())),
            ("Publisher", "Mark Crowther", Publisher(text("Mark Crowther"))),
        ];

        for (key, value, expected) in tags {
            let tag = Tag::from_pair(text(key), value.to_string()).unwrap();
            assert_eq!(tag, expected);
            assert_eq!(tag.to_string(), format!("[{key} \"{value}\"]"));
        }
    }

    #[test]
    fn invalid_metadata_is_preserved() {
        let tag = Tag::from_pair(text("WhiteElo"), "unrated".to_string()).unwrap();
        assert_eq!(tag, Tag::Other(Other { key: text("WhiteElo"), value: "unrated".to_string() }));
        assert_eq!(Tag::from_pair(text("EventCountry"), String::new()), None);
    }
}
