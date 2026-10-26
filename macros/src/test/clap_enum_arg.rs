use clap::{Parser, error::ErrorKind};

use crate::EnumArg;

#[derive(Clone, Copy, Debug, Default, EnumArg, Eq, PartialEq)]
/// Choose an enum value.
enum Enum {
    /// Choose A.
    #[default]
    A,
    Bc,
    DE,
}

#[derive(Clone, Copy, Debug, EnumArg, Eq, PartialEq)]
enum Optional {
    A,
    Bc,
}

#[derive(Debug, Parser)]
struct Command {
    #[command(flatten)]
    not_enum: Enum,
}

#[derive(Debug, Parser)]
struct OptionalCommand {
    #[command(flatten)]
    optional: Option<Optional>,
}

#[test]
fn parses_value_and_variant_flags() {
    assert_eq!(Command::try_parse_from(["test"]).unwrap().not_enum, Enum::A);
    assert_eq!(Command::try_parse_from(["test", "--enum", "bc"]).unwrap().not_enum, Enum::Bc,);
    assert_eq!(Command::try_parse_from(["test", "--enum", "d_e"]).unwrap().not_enum, Enum::DE,);
    assert_eq!(Command::try_parse_from(["test", "--bc"]).unwrap().not_enum, Enum::Bc);
    assert_eq!(Command::try_parse_from(["test", "--d-e"]).unwrap().not_enum, Enum::DE);
}

#[test]
fn rejects_multiple_forms() {
    let error = Command::try_parse_from(["test", "--enum", "bc", "--d-e"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
}

#[test]
fn uses_doc_comments_as_help() {
    let command = <Command as clap::CommandFactory>::command();
    let enum_arg = command.get_arguments().find(|argument| argument.get_id() == "enum").unwrap();
    assert_eq!(
        enum_arg.get_help().map(ToString::to_string).as_deref(),
        Some("Choose an enum value")
    );
    let a = command.get_arguments().find(|argument| argument.get_id() == "a").unwrap();
    assert_eq!(a.get_help().map(ToString::to_string).as_deref(), Some("Choose A"));
}

#[test]
fn parses_optional_enum() {
    assert_eq!(OptionalCommand::try_parse_from(["test"]).unwrap().optional, None);
    assert_eq!(
        OptionalCommand::try_parse_from(["test", "--optional", "bc"]).unwrap().optional,
        Some(Optional::Bc),
    );
    assert_eq!(
        OptionalCommand::try_parse_from(["test", "--a"]).unwrap().optional,
        Some(Optional::A),
    );
}
