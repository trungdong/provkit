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
    /// From a file extension (`.json`, `.provn`).
    pub fn from_path(path: &Path) -> Option<Format> {
        match path.extension()?.to_str()? {
            "json" => Some(Format::Json),
            "provn" => Some(Format::Provn),
            _ => None,
        }
    }

    /// From the text itself: `{` is PROV-JSON and `document` is PROV-N,
    /// after leading whitespace and PROV-N comments.
    pub fn sniff(text: &str) -> Option<Format> {
        let mut rest = text;
        loop {
            rest = rest.trim_start();
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
        } else if rest.starts_with("document") {
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
    }

    #[test]
    fn extension_maps_to_format() {
        assert_eq!(Format::from_path(Path::new("a.json")), Some(Format::Json));
        assert_eq!(Format::from_path(Path::new("a.provn")), Some(Format::Provn));
        assert_eq!(Format::from_path(Path::new("a.txt")), None);
        assert_eq!(Format::from_path(Path::new("-")), None);
    }
}
