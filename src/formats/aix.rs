//! Aix game database format.

/// One serialized Aix game row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Aix {
    pub lichess_id: Option<String>,
    pub tournament: Option<String>,
    pub movedata: Vec<u8>,
    pub clocks_white: Option<Vec<u16>>,
    pub clocks_black: Option<Vec<u16>>,
    pub evals: Option<Vec<i16>>,
    pub ply_count: u16,
    pub white: Option<String>,
    pub black: Option<String>,
    pub white_rating: Option<i16>,
    pub black_rating: Option<i16>,
    pub time_initial: Option<u16>,
    pub time_increment: Option<u8>,
    pub result: Option<String>,
    pub termination: Option<String>,
    pub white_rating_diff: Option<i16>,
    pub black_rating_diff: Option<i16>,
    pub eco: Option<String>,
    pub opening: Option<String>,
    pub white_title: Option<String>,
    pub black_title: Option<String>,
    /// Microseconds since the Unix epoch in UTC.
    pub utc_timestamp: Option<i64>,

    // ChessGrip extensions. Both start fields being absent means the standard position.
    pub start_fen: Option<String>,
    pub scharnagl: Option<u16>,
    /// Bits set least-significant first for evaluations present in the source data.
    pub eval_presence: Option<Vec<u8>>,
}
