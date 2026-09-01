//! CLI 参数解析测试(不经子进程)。

use clap::Parser;
use clap::error::ErrorKind;
use xhup_cli::Cli;

#[test]
fn accepts_generate_rime_with_output() {
    Cli::try_parse_from(["xhup-cli", "generate", "rime", "--output", "out"]).unwrap();
}

#[test]
fn rejects_generate_rime_without_output() {
    assert!(Cli::try_parse_from(["xhup-cli", "generate", "rime"]).is_err());
}

#[test]
fn rejects_generate_without_target() {
    assert!(Cli::try_parse_from(["xhup-cli", "generate"]).is_err());
}

#[test]
fn rejects_unknown_command() {
    assert!(Cli::try_parse_from(["xhup-cli", "unknown"]).is_err());
}

#[test]
fn provides_help_at_all_levels() {
    for args in [
        ["xhup-cli", "--help"].as_slice(),
        ["xhup-cli", "generate", "--help"].as_slice(),
        ["xhup-cli", "generate", "rime", "--help"].as_slice(),
    ] {
        let error = Cli::try_parse_from(args).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    }
}
