//! Format names, detection and sniffing.

use std::path::Path;

use clap::ValueEnum;

/// A serialisation the tool reads and writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// PROV-JSON.
    Json,
    /// PROV-N.
    Provn,
}

impl Format {
    /// The grammar's whitespace: space, tab, CR and LF. No other Unicode
    /// space separates PROV-N tokens.
    const WS: [char; 4] = [' ', '\t', '\r', '\n'];

    /// From a file extension (`.json`, `.provn`), in any letter case.
    pub fn from_path(path: &Path) -> Option<Format> {
        let ext = path.extension()?.to_str()?;
        if ext.eq_ignore_ascii_case("json") {
            Some(Format::Json)
        } else if ext.eq_ignore_ascii_case("provn") {
            Some(Format::Provn)
        } else {
            None
        }
    }

    /// From the text itself: `{` is PROV-JSON and `document` as a whole
    /// word is PROV-N, after leading whitespace and PROV-N comments. A comment
    /// may follow `document` directly.
    pub fn sniff(text: &str) -> Option<Format> {
        // The readers drop a leading byte-order mark, so sniffing skips it too.
        let mut rest = text.strip_prefix('\u{FEFF}').unwrap_or(text);
        // Mirrors `skip_trivia` in the library's PROV-N lexer, which is not
        // public: the grammar's whitespace and both comment forms.
        loop {
            rest = rest.trim_start_matches(Self::WS);
            if let Some(r) = rest.strip_prefix("//") {
                rest = r.split_once(['\n', '\r']).map_or("", |(_, tail)| tail);
            } else if let Some(r) = rest.strip_prefix("/*") {
                rest = r.split_once("*/").map_or("", |(_, tail)| tail);
            } else {
                break;
            }
        }
        if rest.starts_with('{') {
            Some(Format::Json)
        } else if rest.strip_prefix("document").is_some_and(|tail| {
            tail.is_empty()
                || tail.starts_with(Self::WS)
                || tail.starts_with("//")
                || tail.starts_with("/*")
        }) {
            Some(Format::Provn)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_skips_whitespace_and_comments() {
        assert_eq!(Format::sniff("  \n{\"a\":1}"), Some(Format::Json));
        assert_eq!(
            Format::sniff("// c\n/* d\n{ */ document"),
            Some(Format::Provn)
        );
        assert_eq!(Format::sniff("// only a comment"), None);
        assert_eq!(Format::sniff("/* unterminated"), None);
        assert_eq!(Format::sniff("hello"), None);
        assert_eq!(Format::sniff("\u{FEFF}{}"), Some(Format::Json));
        assert_eq!(Format::sniff("document"), Some(Format::Provn));
        assert_eq!(Format::sniff("document\n"), Some(Format::Provn));
        assert_eq!(Format::sniff("documentary"), None);
        assert_eq!(Format::sniff("document("), None);
        assert_eq!(Format::sniff("document/*x*/"), Some(Format::Provn));
        assert_eq!(Format::sniff("document// x\n"), Some(Format::Provn));
        assert_eq!(Format::sniff("document/x"), None);
        assert_eq!(Format::sniff("\u{2003}document"), None);
        assert_eq!(Format::sniff("\u{c}document\n"), None);
        assert_eq!(Format::sniff("document\u{a0}x"), None);
        assert_eq!(Format::sniff("\u{a0}{}"), None);
    }

    #[test]
    fn extension_maps_to_format() {
        assert_eq!(Format::from_path(Path::new("a.json")), Some(Format::Json));
        assert_eq!(Format::from_path(Path::new("a.provn")), Some(Format::Provn));
        assert_eq!(Format::from_path(Path::new("a.txt")), None);
        assert_eq!(Format::from_path(Path::new("-")), None);
        assert_eq!(Format::from_path(Path::new("a.JSON")), Some(Format::Json));
        assert_eq!(Format::from_path(Path::new("a.ProvN")), Some(Format::Provn));
    }
}
