use crate::program::types::FinalizedTypes;
use data::tokens::Span;
use data::SourceSet;
use std::fmt::{Display, Formatter};

use colored::Colorize;

#[derive(Debug, Clone)]
pub enum ParsingMessage {
    ShouldntSee(&'static str),
    StringAttribute(),
    NoReturn(),
    UnexpectedValue(),
    UnexpectedLet(),
    UnexpectedIf(),
    UnexpectedElse(),
    UnexpectedFor(),
    UnexpectedToken(),
    UnexpectedSymbol(),
    UnexpectedVoid(),
    UnexpectedTopElement(),
    UnexpectedReturnType(FinalizedTypes, FinalizedTypes),
    ExpectedEffect(),
    ExpectedCodeBlock(),
    ExpectedVariableName(),
    ExpectedIn(),
    ExpectedWhile(),
    ExtraSymbol(),
    SelfInStatic(),
    FailedToFind(String),
    UnexpectedCharacters(),
    DuplicateStructure(),
    DuplicateFunction(),
    UnknownField(String),
    IncorrectBoundsLength(),
    MismatchedTypes(FinalizedTypes, FinalizedTypes),
    UnknownOperation(String),
    UnknownFunction(),
    MissingArgument(u64, u64),
    AmbiguousMethod(String),
    NoMethod(String, FinalizedTypes),
    NoImpl(FinalizedTypes, String),
    NoTraitImpl(FinalizedTypes, FinalizedTypes),
}

impl Display for ParsingMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            ParsingMessage::ShouldntSee(message) => write!(f, "You shouldn't see this - {}", message),
            ParsingMessage::StringAttribute() => write!(f, "The operator attribute should have a string value"),
            ParsingMessage::NoReturn() => write!(f, "No value was returned!"),
            ParsingMessage::UnexpectedValue() => write!(f, "Unexpected value! Did you forget a semicolon?"),
            ParsingMessage::UnexpectedLet() => write!(f, "Unexpected let! Did you forget a semicolon?"),
            ParsingMessage::UnexpectedIf() => write!(f, "Unexpected if! Did you forget a semicolon?"),
            ParsingMessage::UnexpectedElse() => write!(f, "Unexpected else!"),
            ParsingMessage::UnexpectedFor() => write!(f, "Unexpected for! Did you forget a semicolon?"),
            ParsingMessage::UnexpectedToken() => write!(f, "Unexpected token, expected variable name!"),
            ParsingMessage::UnexpectedSymbol() => write!(f, "Unexpected symbol, expected equals!"),
            ParsingMessage::UnexpectedVoid() => write!(f, "Expected a value, found void!"),
            ParsingMessage::UnexpectedTopElement() => write!(f, "Unexpected top element!"),
            ParsingMessage::UnexpectedReturnType(expected, gotten) => {
                write!(f, "Unexpected return type! Expected a {} but found {}", fix_type(expected), fix_type(gotten))
            }
            ParsingMessage::ExpectedEffect() => write!(f, "Expected an effect!"),
            ParsingMessage::ExpectedCodeBlock() => write!(f, "Expected a code block!"),
            ParsingMessage::ExpectedVariableName() => write!(f, "Expected a variable name!"),
            ParsingMessage::ExpectedWhile() => write!(f, "Expected a while!"),
            ParsingMessage::ExpectedIn() => write!(f, "Missing \"in\" in for loop."),
            ParsingMessage::ExtraSymbol() => write!(f, "Extra symbol!"),
            ParsingMessage::SelfInStatic() => write!(f, "self in static function!"),
            ParsingMessage::FailedToFind(name) => write!(f, "Failed to find type {}, did you import it correctly?", name),
            ParsingMessage::UnexpectedCharacters() => write!(f, "Unexpected characters!"),
            ParsingMessage::DuplicateStructure() => write!(f, "Duplicate structure!"),
            ParsingMessage::DuplicateFunction() => write!(f, "Duplicate function!"),
            ParsingMessage::UnknownField(field) => write!(f, "Unknown field {}!", field),
            ParsingMessage::IncorrectBoundsLength() => write!(f, "Incorrect bounds length!"),
            ParsingMessage::MismatchedTypes(found, bound) => {
                write!(f, "{} isn't of type {}", fix_type(found), fix_type(bound))
            }
            ParsingMessage::UnknownOperation(operation) => write!(f, "Unknown operation '{}'", operation),
            ParsingMessage::UnknownFunction() => write!(f, "Unknown function!"),
            ParsingMessage::MissingArgument(expected, found) => write!(f, "Expected {} arguments but found {}!", expected, found),
            ParsingMessage::AmbiguousMethod(name) => write!(f, "Ambiguous method {}!", name),
            ParsingMessage::NoMethod(name, types) => write!(f, "No method {} for generic {}", name, fix_type(types)),
            ParsingMessage::NoImpl(base, method) => {
                write!(f, "No implementation of method {} for {}", method, fix_type(base))
            }
            ParsingMessage::NoTraitImpl(base, traits) => {
                write!(f, "No implementation of {} for {}", fix_type(traits), fix_type(base))
            }
        };
    }
}

fn fix_type(types: &FinalizedTypes) -> String {
    let mut string = types.to_string();
    if let Some(start) = string.find('$') {
        string.replace_range(start..string.find('<').unwrap_or(string.len()), "");
    }
    return string;
}

/// An error somewhere in a source file, with exact location.
#[derive(Clone, Debug)]
pub struct ParsingError {
    /// The location of the error
    pub span: Span,
    /// The error message
    pub message: ParsingMessage,
}

pub trait ErrorSource {
    fn make_error(&self, message: ParsingMessage) -> ParsingError;
}

impl ErrorSource for Span {
    fn make_error(&self, message: ParsingMessage) -> ParsingError {
        return ParsingError::new(self.clone(), message);
    }
}

impl ParsingError {
    /// Creates a new error
    pub fn new(span: Span, message: ParsingMessage) -> Self {
        return Self { span, message };
    }

    /// Prints the error to console
    pub fn print(&self, sources: &Vec<Box<dyn SourceSet>>) {
        let mut file = None;
        'outer: for source in sources {
            for readable in source.get_files() {
                if self.span.file == readable.hash() {
                    file = Some(readable);
                    break 'outer;
                }
            }
        }

        if file.is_none() {
            eprintln!("Missing file: {}", self.message);
            return;
        }
        let file = file.unwrap();
        let contents = file.contents();
        let tokens = file.read();
        let mut token = tokens[self.span.start].clone();
        if self.span.start != self.span.end {
            let end = &tokens[self.span.end];
            token.end = end.end;
            token.end_offset = end.end_offset;
        }

        // Multi-line tokens aren't supported, set the end to the start
        if token.start.0 != token.end.0 {
            token.start_offset = token.end_offset - token.end.1 as usize;
            token.start = (token.end.0, 0);
        }

        if token.end_offset == token.start_offset {
            token.start_offset -= 1;
        }

        let line = contents.lines().nth((token.start.0 as usize).max(1) - 1).unwrap_or("???");
        eprintln!("{}", self.message.to_string().bright_red());
        eprintln!("{}", format!("in file {}:{}:{}", file.path(), token.start.0, token.start.1).bright_red());
        eprintln!("{} {}", " ".repeat(token.start.0.to_string().len()), "|".bright_cyan());
        eprintln!("{} {} {}", token.start.0.to_string().bright_cyan(), "|".bright_cyan(), line.bright_red());
        eprintln!(
            "{} {} {}{}",
            " ".repeat(token.start.0.to_string().len()),
            "|".bright_cyan(),
            " ".repeat(token.start.1 as usize),
            "^".repeat(token.end_offset - token.start_offset).bright_red()
        );
    }
}
