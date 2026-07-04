//! Slash-command parsing for the TUI input box (`/exit`, `/semantic small|large`,
//! `/config`, `/help`). Pure and UI-agnostic so it's unit-testable without an
//! `App`/`Engine` fixture; `tui::App` owns all the side effects.

use crate::config::ModelSize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Exit,
    Help,
    Config,
    Semantic(ModelSize),
    Unknown(String),
}

/// Returns `None` if `input` isn't a slash command at all (normal search text).
/// A recognized-looking command with a bad/missing argument still parses to
/// `Some(Unknown(..))`, not `None` — the caller can then show an error instead
/// of silently searching for the literal command text. Owns its data (rather
/// than borrowing `input`) so callers can freely mutate the input box right
/// after parsing.
pub fn parse_command(input: &str) -> Option<ParsedCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let mut parts = trimmed.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next();
    let has_extra = parts.next().is_some();

    Some(match (cmd, arg, has_extra) {
        ("/exit" | "/quit", None, false) => ParsedCommand::Exit,
        ("/help", None, false) => ParsedCommand::Help,
        ("/config", None, false) => ParsedCommand::Config,
        ("/semantic", Some("small"), false) => ParsedCommand::Semantic(ModelSize::Small),
        ("/semantic", Some("large"), false) => ParsedCommand::Semantic(ModelSize::Large),
        _ => ParsedCommand::Unknown(trimmed.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_slash_input_is_not_a_command() {
        assert_eq!(parse_command("electromagnetic induction"), None);
        assert_eq!(parse_command(""), None);
        assert_eq!(parse_command("  "), None);
    }

    #[test]
    fn recognizes_exit_and_quit() {
        assert_eq!(parse_command("/exit"), Some(ParsedCommand::Exit));
        assert_eq!(parse_command("/quit"), Some(ParsedCommand::Exit));
        assert_eq!(parse_command("  /exit  "), Some(ParsedCommand::Exit));
    }

    #[test]
    fn recognizes_help_and_config() {
        assert_eq!(parse_command("/help"), Some(ParsedCommand::Help));
        assert_eq!(parse_command("/config"), Some(ParsedCommand::Config));
    }

    #[test]
    fn recognizes_semantic_switch() {
        assert_eq!(
            parse_command("/semantic small"),
            Some(ParsedCommand::Semantic(ModelSize::Small))
        );
        assert_eq!(
            parse_command("/semantic large"),
            Some(ParsedCommand::Semantic(ModelSize::Large))
        );
    }

    #[test]
    fn bad_or_missing_args_are_unknown_not_none() {
        assert_eq!(
            parse_command("/semantic"),
            Some(ParsedCommand::Unknown("/semantic".to_string()))
        );
        assert_eq!(
            parse_command("/semantic medium"),
            Some(ParsedCommand::Unknown("/semantic medium".to_string()))
        );
        assert_eq!(
            parse_command("/semantic small extra"),
            Some(ParsedCommand::Unknown("/semantic small extra".to_string()))
        );
        assert_eq!(
            parse_command("/bogus"),
            Some(ParsedCommand::Unknown("/bogus".to_string()))
        );
        assert_eq!(
            parse_command("/exit now"),
            Some(ParsedCommand::Unknown("/exit now".to_string()))
        );
    }
}
