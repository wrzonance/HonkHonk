//! Process-wide logging setup (#154). Replaces ad-hoc `eprintln!` with
//! `tracing`. `init()` is called once at the very top of `main()`.

use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;

/// Default verbosity when `HONKHONK_LOG` is unset: our crate at `info`,
/// dependencies quiet at `warn`.
const DEFAULT_DIRECTIVE: &str = "warn,honkhonk=info";

/// Resolves the `EnvFilter` directive from a `HONKHONK_LOG` value: the env
/// string when set and non-blank, otherwise the default. Pure, for testing.
pub fn log_directive(env: Option<&str>) -> String {
    match env {
        Some(s) if !s.trim().is_empty() => s.to_string(),
        _ => DEFAULT_DIRECTIVE.to_string(),
    }
}

/// Installs the global `tracing` subscriber: compact `LEVEL target: message`
/// to stderr, ANSI only on a TTY, no timestamps. Verbosity from `HONKHONK_LOG`
/// (default `warn,honkhonk=info`). Call once, first thing in `main()`.
pub fn init() {
    let directive = log_directive(std::env::var("HONKHONK_LOG").ok().as_deref());
    // `parse_lossy` never panics: invalid directives are dropped with a warning.
    let filter = EnvFilter::builder().parse_lossy(directive);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .without_time()
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_falls_back_to_default_when_unset() {
        assert_eq!(log_directive(None), "warn,honkhonk=info");
    }

    #[test]
    fn directive_uses_env_value_when_set() {
        assert_eq!(log_directive(Some("debug")), "debug");
        assert_eq!(log_directive(Some("honkhonk=trace")), "honkhonk=trace");
    }

    #[test]
    fn directive_ignores_blank_env() {
        assert_eq!(log_directive(Some("")), "warn,honkhonk=info");
        assert_eq!(log_directive(Some("   ")), "warn,honkhonk=info");
    }
}
