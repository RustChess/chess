//! ChessBase nation codes.

use crate::formats::Text;

const CODES: [&str; 252] = [
    "AFG", "ALB", "ALG", "ANG", "AND", "ANT", "ARG", "ARM", "AUS", "AUT", "AZE", "BAH", "BHN",
    "BAN", "BAR", "BLR", "BEL", "BLZ", "BER", "BOL", "BIH", "BOT", "BRA", "IVB", "BRU", "BUL",
    "BUR", "CAN", "CHI", "CHN", "COL", "CRI", "CRO", "CUB", "CYP", "CZE", "DEN", "DJI", "DOM",
    "ECU", "EGY", "ENG", "ESP", "EST", "ETH", "FAI", "FIJ", "FIN", "FRA", "FRM", "GAM", "GEO",
    "GER", "GHA", "GRE", "GUA", "GCI", "GUY", "HAI", "HON", "HKG", "HUN", "ISL", "IND", "INA",
    "IRI", "IRQ", "ISR", "IRL", "ITA", "IVO", "JAM", "JPN", "JCI", "JOR", "KAZ", "KEN", "KOR",
    "KGZ", "KUW", "LAT", "LBN", "LBY", "LIE", "LTU", "LUX", "MAC", "MAD", "MAS", "MLI", "MLT",
    "MAU", "MRI", "MEX", "MDA", "MNC", "MGL", "MAR", "MOZ", "MYA", "NAM", "NEP", "NED", "AHO",
    "NZL", "NCA", "NGR", "NOR", "PAK", "PLE", "PAN", "PNG", "PAR", "PER", "PHI", "POL", "POR",
    "PUR", "QAT", "ROM", "RUS", "ESA", "SMR", "SCO", "SEN", "SEY", "SIN", "SVK", "SLO", "RSA",
    "SRI", "SUD", "SUR", "SWE", "SUI", "SYR", "TJK", "TAN", "THA", "TRI", "TUN", "TUR", "TKM",
    "UGA", "UKR", "UAE", "USA", "URU", "URS", "UZB", "VEN", "VIE", "ISV", "WLS", "YEM", "YUG",
    "ZAM", "ZIM", "ZRE", "DDR", "CSR", "CAM", "CHD", "KAP", "KBA", "COM", "CON", "NKO", "LAO",
    "LES", "MWI", "MDV", "MSH", "OMN", "NAU", "MIC", "NIG", "SAU", "TOG", "TON", "VAN", "VAT",
    "TUV", "SWA", "SIE", "SLU", "PAP", "SVI", "SAM", "SKI", "SAL", "GE2", "ZAR", "RWA", "LBR",
    "NET", "TWN", "ASU", "AGG", "BNN", "BTN", "BRI", "CMB", "CAY", "CAR", "CIB", "CIA", "COA",
    "CIN", "ELG", "ERI", "FGB", "FRG", "FRP", "GAB", "GGB", "GRA", "GRL", "FGA", "GMA", "GUI",
    "GUB", "IOM", "JMY", "MFR", "MYF", "MSG", "NCF", "NNN", "NNA", "NMI", "OTM", "PAL", "PIG",
    "RUF", "SAO", "SOM", "SVN", "HGB", "PGB", "TKI", "TCI", "WFR", "NIR", "ISS", "GBR", "SAA",
    "MNT", "SER", "CAT", "BAS", "KOS",
];

pub fn code(nation: u8) -> Option<Text> {
    nation
        .checked_sub(1)
        .and_then(|index| CODES.get(usize::from(index)))
        .map(|code| Text::new(code).expect("ChessBase nation codes are non-empty"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        assert_eq!(code(0), None);
        assert_eq!(code(7).as_deref(), Some("ARG"));
        assert_eq!(code(55).as_deref(), Some("GRE"));
        assert_eq!(code(252).as_deref(), Some("KOS"));
        assert_eq!(code(253), None);
    }
}
