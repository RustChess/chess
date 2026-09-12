use core::{fmt, ops::Deref, str::FromStr};

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Text(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("text is empty")]
pub struct Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl Text {
    #[inline]
    pub fn new(text: impl AsRef<str>) -> Result<Self> {
        let text = text.as_ref().trim();
        (!text.is_empty()).then(|| Self(text.to_string())).ok_or(Error)
    }

    pub fn merge(&mut self, text: &Text) {
        self.0.push_str("\n\n");
        self.0.push_str(&text.0);
    }
}

impl FromStr for Text {
    type Err = Error;

    #[inline]
    fn from_str(text: &str) -> Result<Self> {
        Self::new(text)
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for Text {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
