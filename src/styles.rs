//! Coloring for what the service prints.
//!
//! A [`Styles`] carries the decision of whether to color at all, so that the callers that
//! format text do not each have to make it, and so that they can be tested either way.
//! What the service prints often ends up in a log file or in `docker logs`, where escape
//! sequences are noise, hence [`Styles::for_stdout`].

use anstyle::{AnsiColor, Color, Style};
use std::fmt::Display;
use std::io::IsTerminal;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Styles {
    colored: bool,
}

impl Styles {
    /// Colored only if the standard output is a terminal, and honoring `NO_COLOR`
    /// (<https://no-color.org>).
    pub fn for_stdout() -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        Self {
            colored: std::io::stdout().is_terminal() && !no_color,
        }
    }

    /// Always colored, and never: the service itself decides from its output, so these
    /// are here for the tests of whatever formats text.
    #[cfg(test)]
    pub fn colored() -> Self {
        Self { colored: true }
    }

    #[cfg(test)]
    pub fn plain() -> Self {
        Self { colored: false }
    }

    /// For something that is wrong and wants acting on.
    pub fn error(&self, text: impl Display) -> String {
        self.paint(
            text,
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
    }

    /// For something that is not wrong yet.
    pub fn warning(&self, text: impl Display) -> String {
        self.paint(
            text,
            Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
    }

    /// The given text in the given style, or as it is when not coloring.
    pub fn paint(&self, text: impl Display, style: Style) -> String {
        if self.colored {
            format!("{style}{text}{style:#}")
        } else {
            text.to_string()
        }
    }
}

/// The styling of the CLI help.
pub fn clap_styles() -> clap::builder::Styles {
    fn style(color: AnsiColor) -> Style {
        Style::new().bold().fg_color(Some(Color::Ansi(color)))
    }
    clap::builder::Styles::styled()
        .usage(style(AnsiColor::Yellow).underline())
        .header(style(AnsiColor::Yellow).underline())
        .literal(style(AnsiColor::Green))
        .placeholder(style(AnsiColor::Blue))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn plain_styles_leave_the_text_alone() {
        let styles = Styles::plain();
        assert_eq!(styles.error("EXPIRED"), "EXPIRED");
        assert_eq!(styles.warning("expires soon"), "expires soon");
    }

    #[test]
    fn colored_styles_wrap_the_text_in_escapes() {
        let styles = Styles::colored();
        assert_eq!(
            styles.error("EXPIRED"),
            "\u{1b}[1m\u{1b}[31mEXPIRED\u{1b}[0m"
        );
        assert_eq!(
            styles.warning("expires soon"),
            "\u{1b}[33mexpires soon\u{1b}[0m"
        );
    }
}
