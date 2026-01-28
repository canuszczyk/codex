/// The current Codex CLI version as embedded at compile time.
/// Can be overridden at build time via CODEX_VERSION_OVERRIDE env var.
pub const CODEX_CLI_VERSION: &str = match option_env!("CODEX_VERSION_OVERRIDE") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};
