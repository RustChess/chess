//! PGN format

// https://www.chessprogramming.org/Portable_Game_Notation
// https://www.saremba.de/chessgml/standards/pgn/pgn-complete.htm
// https://github.com/mliebelt/pgn-spec-commented
// https://github.com/mliebelt/pgn-spec-commented/blob/main/pgn-spec-supplement.md
//
// Arrows and coloured squares
// [%cal Gc2c3,Rc3d4] green arrow c2-c3, red arrow c3-d4
// [$csl Ra3,Ga4] a3 red, a4 green
// # insert mini board in move list
// https://chesstempo.com/manual/en/manual.html#pgnviewercommentannotations
//

pub struct Pgn;
