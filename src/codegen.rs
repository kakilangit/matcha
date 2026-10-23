//! Turns the current pattern into a ready-to-paste Rust snippet.

use matcha::engine::{EngineKind, Flags};

/// Rust code that builds the pattern with the chosen engine and flags, plus a
/// `replace_all` call when a replacement template is set.
#[must_use]
pub fn rust_snippet(pattern: &str, flags: Flags, engine: EngineKind, replacement: &str) -> String {
    let krate = engine.crate_path();
    let pattern = raw_string(pattern);
    let options = builder_options(flags, engine);

    let mut lines = if options.is_empty() {
        vec![
            format!("use {krate}::Regex;"),
            String::new(),
            format!("let re = Regex::new({pattern}).unwrap();"),
        ]
    } else {
        let mut lines = vec![
            format!("use {krate}::RegexBuilder;"),
            String::new(),
            format!("let re = RegexBuilder::new({pattern})"),
        ];
        lines.extend(options.iter().map(|option| format!("    .{option}")));
        lines.push("    .build()".to_owned());
        lines.push("    .unwrap();".to_owned());
        lines
    };
    if !replacement.is_empty() {
        lines.push(format!(
            "let result = re.replace_all(text, {});",
            raw_string(replacement)
        ));
    }
    let mut code = lines.join("\n");
    code.push('\n');
    code
}

/// Builder calls for every flag that differs from the engine's default.
fn builder_options(flags: Flags, engine: EngineKind) -> Vec<String> {
    let defaults = Flags::default();
    let unicode = engine.unicode_option();
    [
        (
            "case_insensitive",
            flags.case_insensitive,
            defaults.case_insensitive,
        ),
        ("multi_line", flags.multi_line, defaults.multi_line),
        (
            "dot_matches_new_line",
            flags.dot_matches_new_line,
            defaults.dot_matches_new_line,
        ),
        (
            "ignore_whitespace",
            flags.ignore_whitespace,
            defaults.ignore_whitespace,
        ),
        (unicode, flags.unicode, defaults.unicode),
    ]
    .into_iter()
    .filter(|(_, value, default)| value != default)
    .map(|(name, value, _)| format!("{name}({value})"))
    .collect()
}

/// A raw string literal with enough `#`s that the text cannot end it early.
fn raw_string(text: &str) -> String {
    let mut hashes = 0;
    while text.contains(&format!("\"{}", "#".repeat(hashes))) {
        hashes += 1;
    }
    let hashes = "#".repeat(hashes);
    format!("r{hashes}\"{text}\"{hashes}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_pattern_uses_regex_new() {
        assert_eq!(
            rust_snippet(r"\d+", Flags::default(), EngineKind::Standard, ""),
            "use regex::Regex;\n\nlet re = Regex::new(r\"\\d+\").unwrap();\n"
        );
    }

    #[test]
    fn flags_use_the_builder() {
        let flags = Flags {
            case_insensitive: true,
            unicode: false,
            ..Flags::default()
        };
        assert_eq!(
            rust_snippet("abc", flags, EngineKind::Standard, "$1"),
            "use regex::RegexBuilder;\n\n\
             let re = RegexBuilder::new(r\"abc\")\n    \
             .case_insensitive(true)\n    \
             .unicode(false)\n    \
             .build()\n    \
             .unwrap();\n\
             let result = re.replace_all(text, r\"$1\");\n"
        );
    }

    #[test]
    fn fancy_engine_uses_its_own_names() {
        let flags = Flags {
            unicode: false,
            ..Flags::default()
        };
        let code = rust_snippet(r"a(?=b)", flags, EngineKind::Fancy, "");
        assert!(code.starts_with("use fancy_regex::RegexBuilder;"), "{code}");
        assert!(code.contains(".unicode_mode(false)"), "{code}");
    }

    #[test]
    fn raw_strings_survive_quotes_and_hashes() {
        assert_eq!(raw_string("plain"), "r\"plain\"");
        assert_eq!(raw_string("say \"hi\""), "r#\"say \"hi\"\"#");
        assert_eq!(raw_string("\"#"), "r##\"\"#\"##");
    }
}
