//! Regex compilation, matching and replacement, independent of the UI.

use std::ops::Range;

/// Maximum number of matches collected for a single search. Keeps the UI
/// responsive when a pattern matches almost every position of a large text.
pub const MATCH_LIMIT: usize = 10_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EngineKind {
    /// The `regex` crate: linear time, no look-around or backreferences.
    #[default]
    Standard,
    /// The `fancy-regex` crate: adds look-around and backreferences.
    Fancy,
}

impl EngineKind {
    pub fn label(self) -> &'static str {
        match self {
            EngineKind::Standard => "regex",
            EngineKind::Fancy => "fancy-regex",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_matches_new_line: bool,
    pub ignore_whitespace: bool,
    pub unicode: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            case_insensitive: false,
            multi_line: false,
            dot_matches_new_line: false,
            ignore_whitespace: false,
            unicode: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Capture group number, starting at 1.
    pub index: usize,
    pub name: Option<String>,
    /// Byte range in the searched text, or `None` if the group did not participate.
    pub range: Option<Range<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub range: Range<usize>,
    pub groups: Vec<Group>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Matches {
    pub matches: Vec<Match>,
    /// True when the search stopped at [`MATCH_LIMIT`].
    pub truncated: bool,
}

pub enum Regex {
    Standard(regex::Regex),
    Fancy(fancy_regex::Regex),
}

impl Regex {
    pub fn new(pattern: &str, flags: Flags, engine: EngineKind) -> Result<Self, String> {
        match engine {
            EngineKind::Standard => regex::RegexBuilder::new(pattern)
                .case_insensitive(flags.case_insensitive)
                .multi_line(flags.multi_line)
                .dot_matches_new_line(flags.dot_matches_new_line)
                .ignore_whitespace(flags.ignore_whitespace)
                .unicode(flags.unicode)
                .build()
                .map(Regex::Standard)
                .map_err(|err| err.to_string()),
            EngineKind::Fancy => fancy_regex::RegexBuilder::new(pattern)
                .case_insensitive(flags.case_insensitive)
                .multi_line(flags.multi_line)
                .dot_matches_new_line(flags.dot_matches_new_line)
                .ignore_whitespace(flags.ignore_whitespace)
                .unicode_mode(flags.unicode)
                .build()
                .map(Regex::Fancy)
                .map_err(|err| err.to_string()),
        }
    }

    /// Names of capture groups 1.., `None` for unnamed groups.
    fn group_names(&self) -> Vec<Option<String>> {
        let names: Vec<Option<&str>> = match self {
            Regex::Standard(re) => re.capture_names().collect(),
            Regex::Fancy(re) => re.capture_names().collect(),
        };
        names
            .into_iter()
            .skip(1)
            .map(|name| name.map(str::to_owned))
            .collect()
    }

    /// Finds up to [`MATCH_LIMIT`] non-overlapping matches.
    ///
    /// Only `fancy-regex` can fail here, when a search exceeds its backtracking limit.
    pub fn find_all(&self, text: &str) -> Result<Matches, String> {
        let names = self.group_names();
        let mut result = Matches::default();
        let mut push = |range: Range<usize>, groups: Vec<Option<Range<usize>>>| {
            if result.matches.len() == MATCH_LIMIT {
                result.truncated = true;
                return false;
            }
            let groups = groups
                .into_iter()
                .zip(&names)
                .enumerate()
                .map(|(ix, (range, name))| Group {
                    index: ix + 1,
                    name: name.clone(),
                    range,
                })
                .collect();
            result.matches.push(Match { range, groups });
            true
        };

        match self {
            Regex::Standard(re) => {
                for caps in re.captures_iter(text) {
                    let whole = caps.get(0).expect("group 0 always participates");
                    let groups = (1..caps.len())
                        .map(|ix| caps.get(ix).map(|m| m.range()))
                        .collect();
                    if !push(whole.range(), groups) {
                        break;
                    }
                }
            }
            Regex::Fancy(re) => {
                for caps in re.captures_iter(text) {
                    let caps = caps.map_err(|err| err.to_string())?;
                    let whole = caps.get(0).expect("group 0 always participates");
                    let groups = (1..caps.len())
                        .map(|ix| caps.get(ix).map(|m| m.start()..m.end()))
                        .collect();
                    if !push(whole.start()..whole.end(), groups) {
                        break;
                    }
                }
            }
        }
        Ok(result)
    }

    /// Replaces every match using a template such as `$1`, `${name}` or `$$`.
    pub fn replace_all(&self, text: &str, template: &str) -> Result<String, String> {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        match self {
            Regex::Standard(re) => {
                for caps in re.captures_iter(text) {
                    let whole = caps.get(0).expect("group 0 always participates");
                    out.push_str(&text[last..whole.start()]);
                    caps.expand(template, &mut out);
                    last = whole.end();
                }
            }
            Regex::Fancy(re) => {
                for caps in re.captures_iter(text) {
                    let caps = caps.map_err(|err| err.to_string())?;
                    let whole = caps.get(0).expect("group 0 always participates");
                    out.push_str(&text[last..whole.start()]);
                    caps.expand(template, &mut out);
                    last = whole.end();
                }
            }
        }
        out.push_str(&text[last..]);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std_re(pattern: &str) -> Regex {
        Regex::new(pattern, Flags::default(), EngineKind::Standard).unwrap()
    }

    fn fancy_re(pattern: &str) -> Regex {
        Regex::new(pattern, Flags::default(), EngineKind::Fancy).unwrap()
    }

    #[test]
    fn finds_matches_with_groups() {
        let found = std_re(r"(?<year>\d{4})-(\d{2})")
            .find_all("2024-01 and 2025-12")
            .unwrap();
        assert!(!found.truncated);
        assert_eq!(found.matches.len(), 2);
        let first = &found.matches[0];
        assert_eq!(first.range, 0..7);
        assert_eq!(
            first.groups,
            vec![
                Group {
                    index: 1,
                    name: Some("year".into()),
                    range: Some(0..4),
                },
                Group {
                    index: 2,
                    name: None,
                    range: Some(5..7),
                },
            ]
        );
        assert_eq!(found.matches[1].range, 12..19);
    }

    #[test]
    fn reports_non_participating_groups() {
        let found = std_re(r"a(b)?").find_all("a").unwrap();
        assert_eq!(found.matches[0].groups[0].range, None);
    }

    #[test]
    fn applies_flags() {
        let flags = Flags {
            case_insensitive: true,
            multi_line: true,
            ..Flags::default()
        };
        let re = Regex::new(r"^abc$", flags, EngineKind::Standard).unwrap();
        assert_eq!(re.find_all("ABC\nabc\nxabc").unwrap().matches.len(), 2);

        let flags = Flags {
            dot_matches_new_line: true,
            ..Flags::default()
        };
        let re = Regex::new(r"a.b", flags, EngineKind::Fancy).unwrap();
        assert_eq!(re.find_all("a\nb").unwrap().matches.len(), 1);

        let flags = Flags {
            ignore_whitespace: true,
            ..Flags::default()
        };
        let re = Regex::new("a b # comment", flags, EngineKind::Standard).unwrap();
        assert_eq!(re.find_all("ab").unwrap().matches.len(), 1);
    }

    #[test]
    fn standard_engine_rejects_look_around() {
        let err = Regex::new(r"a(?=b)", Flags::default(), EngineKind::Standard)
            .err()
            .unwrap();
        assert!(err.contains("look-around"), "{err}");
    }

    #[test]
    fn fancy_engine_supports_look_around_and_backreferences() {
        let found = fancy_re(r"\w+(?=!)").find_all("hi there!").unwrap();
        assert_eq!(found.matches[0].range, 3..8);

        let found = fancy_re(r"(\w)\1").find_all("abba").unwrap();
        assert_eq!(found.matches[0].range, 1..3);
    }

    #[test]
    fn invalid_pattern_is_an_error() {
        assert!(Regex::new("(", Flags::default(), EngineKind::Standard).is_err());
        assert!(Regex::new("(", Flags::default(), EngineKind::Fancy).is_err());
    }

    #[test]
    fn stops_at_match_limit() {
        let text = "a".repeat(MATCH_LIMIT + 5);
        let found = std_re("a").find_all(&text).unwrap();
        assert!(found.truncated);
        assert_eq!(found.matches.len(), MATCH_LIMIT);
    }

    #[test]
    fn replaces_with_numbered_and_named_groups() {
        let re = std_re(r"(?<y>\d{4})-(\d{2})");
        assert_eq!(
            re.replace_all("on 2024-01.", "$2/${y} $$").unwrap(),
            "on 01/2024 $."
        );

        let re = fancy_re(r"(?<w>\w+)(?= )");
        assert_eq!(re.replace_all("ab cd", "[${w}]").unwrap(), "[ab] cd");
    }
}
