use crate::parser::control_parser::{parse_do_while, parse_for, parse_if, parse_while};
use crate::parser::operator_parser::parse_operator;
use crate::parser::util::{parse_generics, ParserUtils};
use data::tokens::{Span, Token, TokenTypes};
use std::mem;
use syntax::async_util::UnparsedType;
use syntax::errors::ParsingError;
use syntax::errors::{ErrorSource, ParsingMessage};
use syntax::program::code::{EffectType, Effects, Expression, ExpressionType};
use syntax::program::function::CodeBody;

/// Parsers a block of code into its return type (if all code paths lead to a single type, or else a line) and the code body.
pub fn parse_code(parser_utils: &mut ParserUtils) -> Result<(ExpressionType, CodeBody), ParsingError> {
    let mut lines = Vec::default();
    let mut types = ExpressionType::Line;
    while let Some(expression) = parse_line(parser_utils, ParseState::None)? {
        if expression.expression_type != ExpressionType::Line {
            types.clone_from(&expression.expression_type);
        }
        lines.push(expression);
        if parser_utils.tokens[parser_utils.index].token_type == TokenTypes::LineEnd {
            parser_utils.index += 1;
        }
    }
    parser_utils.imports.last_id += 1;
    return Ok((types, CodeBody::new(lines, (parser_utils.imports.last_id - 1).to_string())));
}

/// The state of the parser
#[derive(PartialEq, Clone)]
pub enum ParseState {
    /// No state, defaults to top elements
    None,
    /// Used when inside the variable of a control statement.
    /// Ex:
    /// if value == 2 {
    /// value == 2 would be parsed as a ControlVariable
    ControlVariable,
    /// When inside the arguments of a function.
    /// Ex:
    /// printf("String");
    /// "String" would be parsed as a argument.
    Argument,
    /// When inside of an operator, such as 1 + 2
    InOperator,
    /// When inside both an operator and control variable.
    ControlOperator,
    /// When inside a new expression.
    New,
}

/// Parses a single line of code
// skipcq: RS-R1000 Match statements have complexity calculated incorrectly
pub fn parse_line(parser_utils: &mut ParserUtils, state: ParseState) -> Result<Option<Expression>, ParsingError> {
    // The current effect
    let mut effect: Option<Effects> = None;
    // The current type of expression
    let mut expression_type = ExpressionType::Line;
    loop {
        let token = parser_utils.tokens[parser_utils.index].clone();
        let span = Span::new(parser_utils.file, parser_utils.index);

        parser_utils.index += 1;
        if effect.is_some() {
            match token.token_type {
                TokenTypes::Float
                | TokenTypes::Integer
                | TokenTypes::Char
                | TokenTypes::True
                | TokenTypes::False
                | TokenTypes::StringStart
                | TokenTypes::CodeEnd
                | TokenTypes::BlockEnd
                | TokenTypes::Let
                | TokenTypes::If
                | TokenTypes::For
                | TokenTypes::While
                | TokenTypes::Do => {
                    return Err(span.make_error(ParsingMessage::UnexpectedValue));
                }
                _ => {}
            }
        }

        match parse_basic_line(
            parser_utils,
            &mut expression_type,
            &token,
            &state,
            Span::new(parser_utils.file, parser_utils.index - 1),
            &mut effect,
        )? {
            ControlFlow::Returning(returning) => return Ok(Some(returning)),
            ControlFlow::Skipping => continue,
            ControlFlow::Finish => break,
            ControlFlow::NotFound => {}
        }

        match token.token_type {
            TokenTypes::CodeEnd | TokenTypes::BlockEnd | TokenTypes::EOF => {
                return Ok(None);
            }
            TokenTypes::Variable => {
                let next = parser_utils.tokens.get(parser_utils.index).unwrap();
                if TokenTypes::ParenOpen == next.token_type {
                    //Skip because ParenOpen handles this.
                } else if TokenTypes::Operator == next.token_type {
                    //Skip if a generic method is being called next to preserve the last effect.
                    if is_generic(&token, parser_utils) {
                        continue;
                    } else {
                        effect = Some(Effects::new(
                            Span::new(parser_utils.file, parser_utils.index),
                            EffectType::LoadVariable(token.to_string(parser_utils.buffer)),
                        ))
                    }
                } else {
                    if effect.is_some() {
                        return Err(span.make_error(ParsingMessage::UnexpectedValue));
                    }

                    effect = Some(Effects::new(
                        Span::new(parser_utils.file, parser_utils.index),
                        EffectType::LoadVariable(token.to_string(parser_utils.buffer)),
                    ))
                }
            }
            TokenTypes::Return => expression_type = ExpressionType::Return(Span::new(parser_utils.file, parser_utils.index)),
            TokenTypes::New => {
                if effect.is_some() {
                    return Err(span.make_error(ParsingMessage::UnexpectedValue));
                }
                effect = Some(parse_new(parser_utils, &span)?);
            }
            TokenTypes::BlockStart => {
                if ParseState::ControlVariable == state || ParseState::ControlOperator == state {
                    parser_utils.index -= 1;
                    break;
                } else {
                    if effect.is_some() {
                        return Err(span.make_error(ParsingMessage::UnexpectedValue));
                    }

                    // Get the code in the next block.
                    let (returning, body) = parse_code(parser_utils)?;
                    // If the inner block returns/breaks, then the outer one should too
                    if matches!(expression_type, ExpressionType::Line) {
                        expression_type = returning;
                    }
                    effect =
                        Some(Effects::new(Span::new(parser_utils.file, parser_utils.index), EffectType::CodeBody(body)));
                }
            }
            TokenTypes::Let => {
                if effect.is_some() {
                    return Err(span.make_error(ParsingMessage::UnexpectedLet));
                }
            }
            TokenTypes::If => {
                if effect.is_some() {
                    return Err(span.make_error(ParsingMessage::UnexpectedIf));
                }

                let expression = parse_if(parser_utils)?;
                // If the if returns/breaks, the outer block should too
                if matches!(expression_type, ExpressionType::Line) {
                    expression_type = expression.expression_type;
                }
                return Ok(Some(Expression::new(expression_type, expression.effect)));
            }
            TokenTypes::For => {
                if effect.is_some() {
                    return Err(span.make_error(ParsingMessage::UnexpectedFor));
                }
                return Ok(Some(Expression::new(expression_type, parse_for(parser_utils)?)));
            }
            TokenTypes::While => {
                if effect.is_some() {
                    return Err(span.make_error(ParsingMessage::UnexpectedFor));
                }
                return Ok(Some(Expression::new(expression_type, parse_while(parser_utils)?)));
            }
            TokenTypes::Do => {
                if effect.is_some() {
                    return Err(span.make_error(ParsingMessage::UnexpectedFor));
                }
                return Ok(Some(Expression::new(expression_type, parse_do_while(parser_utils)?)));
            }
            TokenTypes::Equals => {
                let other = parser_utils.tokens.get(parser_utils.index).unwrap().token_type.clone();
                // Check to make sure this isn't an operation like == or +=
                if effect.is_some() && other != TokenTypes::Operator && other != TokenTypes::Equals {
                    let value = parse_line(parser_utils, ParseState::None)?;
                    if let Some(value) = value {
                        effect = Some(Effects::new(
                            Span::new(parser_utils.file, parser_utils.index),
                            EffectType::Set(Box::new(effect.unwrap()), Box::new(value.effect)),
                        ));
                    } else {
                        return Err(span.make_error(ParsingMessage::UnexpectedVoid));
                    }
                    break;
                } else {
                    // It must be an operator, parse it like one.
                    let operator = parse_operator(effect, parser_utils, &state)?;
                    if ParseState::InOperator == state || ParseState::ControlOperator == state {
                        return Ok(Some(Expression::new(expression_type, operator)));
                    } else {
                        effect = Some(operator);
                    }
                }
            }
            TokenTypes::Operator => {
                let last = parser_utils.tokens.get(parser_utils.index - 2).unwrap();
                // If there is a variable right next to a less than, it's probably a generic method call.
                // Example: test<Value>() or Structure<Value>::method()
                parser_utils.index -= 1;
                if (last.token_type == TokenTypes::Variable || last.token_type == TokenTypes::CallingType)
                    && is_generic(&parser_utils.tokens[parser_utils.index - 1], parser_utils)
                {
                    parser_utils.index += 1;
                    effect = Some(parse_generic_method(effect, parser_utils)?);
                } else {
                    parser_utils.index += 1;
                    let operator = parse_operator(effect, parser_utils, &state)?;
                    // Operators inside operators return immediately so operators can be combined
                    // later on for operators like [].
                    if ParseState::InOperator == state || ParseState::ControlOperator == state {
                        return Ok(Some(Expression::new(expression_type, operator)));
                    } else {
                        effect = Some(operator);
                    }
                }
            }
            TokenTypes::CallingType => {
                let next: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
                if next.token_type == TokenTypes::ParenOpen || is_generic(&token, parser_utils) {
                    // Ignored, ParenOpen or Operator handles this
                } else {
                    if effect.is_none() {
                        return Err(span.make_error(ParsingMessage::ExtraSymbol));
                    }
                    effect = Some(Effects::new(
                        Span::new(parser_utils.file, parser_utils.index),
                        EffectType::Load(Box::new(effect.unwrap()), token.to_string(parser_utils.buffer)),
                    ))
                }
            }
            TokenTypes::Else => return Err(span.make_error(ParsingMessage::UnexpectedElse)),
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    if effect.is_none() {
        return Ok(None);
    }

    return Ok(Some(Expression::new(expression_type, effect.unwrap())));
}

/// Tells the function what to do after the line is parsed
enum ControlFlow {
    NotFound,
    Skipping,
    Finish,
    Returning(Expression),
}

/// Handles some basic cases separately to reduce the complexity of the main function
// skipcq: RS-R1000 Match statements have complexity calculated incorrectly
fn parse_basic_line(
    parser_utils: &mut ParserUtils,
    expression_type: &mut ExpressionType,
    token: &Token,
    state: &ParseState,
    span: Span,
    effect: &mut Option<Effects>,
) -> Result<ControlFlow, ParsingError> {
    return Ok(match token.token_type {
        TokenTypes::BlockEnd if *state == ParseState::New => ControlFlow::Finish,
        TokenTypes::Return => {
            *expression_type = ExpressionType::Return(span);
            ControlFlow::Skipping
        }
        TokenTypes::Float => {
            *effect = Some(Effects::new(
                Span::new(parser_utils.file, parser_utils.index),
                EffectType::Float(token.to_string(parser_utils.buffer).parse().unwrap()),
            ));
            ControlFlow::Skipping
        }
        TokenTypes::Integer => {
            *effect = Some(Effects::new(
                Span::new(parser_utils.file, parser_utils.index),
                EffectType::Int(token.to_string(parser_utils.buffer).parse().unwrap()),
            ));
            ControlFlow::Skipping
        }
        TokenTypes::Char => {
            *effect = Some(Effects::new(
                Span::new(parser_utils.file, parser_utils.index),
                EffectType::Char(token.to_string(parser_utils.buffer).as_bytes()[1] as char),
            ));
            ControlFlow::Skipping
        }
        TokenTypes::True => {
            *effect = Some(Effects::new(Span::new(parser_utils.file, parser_utils.index), EffectType::Bool(true)));
            ControlFlow::Skipping
        }
        TokenTypes::False => {
            *effect = Some(Effects::new(Span::new(parser_utils.file, parser_utils.index), EffectType::Bool(false)));
            ControlFlow::Skipping
        }
        TokenTypes::StringStart => {
            *effect = Some(parse_string(parser_utils)?);
            ControlFlow::Skipping
        }
        TokenTypes::Let => ControlFlow::Returning(Expression::new(expression_type.clone(), parse_let(parser_utils)?)),
        TokenTypes::If => {
            let expression = parse_if(parser_utils)?;
            let mut expression_type = expression_type.clone();
            // If the if returns/breaks, the outer block should too
            if expression_type == ExpressionType::Line {
                expression_type = expression.expression_type;
            }
            ControlFlow::Returning(Expression::new(expression_type, expression.effect))
        }
        TokenTypes::For => ControlFlow::Returning(Expression::new(expression_type.clone(), parse_for(parser_utils)?)),
        TokenTypes::While => ControlFlow::Returning(Expression::new(expression_type.clone(), parse_while(parser_utils)?)),
        TokenTypes::Do => ControlFlow::Returning(Expression::new(expression_type.clone(), parse_do_while(parser_utils)?)),
        TokenTypes::LineEnd | TokenTypes::ParenClose | TokenTypes::ArgumentEnd => {
            parser_utils.index -= 1;
            ControlFlow::Finish
        }
        TokenTypes::Comment => ControlFlow::Skipping,
        TokenTypes::ParenOpen => {
            let last = parser_utils.tokens.get(parser_utils.index - 2).unwrap().clone();
            match last.token_type {
                TokenTypes::Variable | TokenTypes::CallingType => {
                    // Name of the method = the last token
                    let name = last.to_string(parser_utils.buffer);
                    let mut temp = None;
                    mem::swap(&mut temp, effect);
                    // The calling effect must be boxed if it exists.
                    *effect = Some(Effects {
                        types: EffectType::MethodCall(
                            temp.map(|inner| Box::new(inner)),
                            name.clone(),
                            get_effects(parser_utils)?,
                            vec![],
                        ),
                        span,
                    });
                    ControlFlow::Skipping
                }
                // If it's not a method call, it's a parenthesized effect.
                _ => {
                    if let Some(expression) = parse_line(parser_utils, ParseState::None)? {
                        *effect = Some(Effects::new(
                            Span::new(parser_utils.file, parser_utils.index),
                            EffectType::Paren(Box::new(expression.effect)),
                        ));
                        parser_utils.index += 1;
                        ControlFlow::Skipping
                    } else {
                        // TODO figure out if this actually ever triggers
                        //effect = None;
                        panic!("Unknown code path - report this!");
                    }
                }
            }
        }
        TokenTypes::Period => {
            if parser_utils.tokens[parser_utils.index].token_type == TokenTypes::Period {
                let mut temp = None;
                mem::swap(&mut temp, effect);
                let operator = parse_operator(temp, parser_utils, &state)?;
                // Operators inside operators return immediately so operators can be combined
                // later on for operators like [].
                if ParseState::InOperator == *state || ParseState::ControlOperator == *state {
                    return Ok(ControlFlow::Returning(Expression::new(expression_type.clone(), operator)));
                } else {
                    *effect = Some(operator);
                }
            }
            ControlFlow::Skipping
        }
        _ => ControlFlow::NotFound,
    });
}

/// Parses tokens from the Raven code into a string
fn parse_string(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let mut string = String::default(); //the string from the Raven code

    loop {
        //loop through the tokens until a StringEnd is reached

        //get the next token
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;

        match token.token_type {
            TokenTypes::StringEnd => {
                // End of string, must have a null character at the end
                let found = token.to_string(parser_utils.buffer);
                string += &found[0..found.len() - 1];
                return Ok(Effects::new(
                    Span::new(parser_utils.file, parser_utils.index - 1),
                    EffectType::String(string + "\0"),
                ));
            }
            TokenTypes::StringEscape => {
                // Escape token

                // get the text from the Raven file starting at the last token up to the current escape character
                let found = token.to_string(parser_utils.buffer);

                // check if it a hex value, because if it is, then it will 4 characters long (\xAA)
                let is_hex = found.len() >= 3 && &found[found.len() - 3..found.len() - 2] == "x";
                let string_end = found.len() - (if is_hex { 4 } else { 2 });

                // add the text to the string, because this text is part of the string in the Raven Code
                string += &found[0..string_end];

                // match the character after the \ to see what type of escape character it is
                let index = if is_hex { found.len() - 3 } else { found.len() - 1 };
                match &found[index..=index] {
                    "n" => {
                        string += "\n";
                    }
                    "t" => {
                        string += "\t";
                    }
                    "r" => {
                        string += "\r";
                    }
                    "\\" => {
                        string += "\\";
                    }
                    "\'" => {
                        string += "\'";
                    }
                    "\"" => {
                        string += "\"";
                    }
                    "x" => {
                        // Convert the hex to a character, and append it to the string
                        string.push(
                            u8::from_str_radix(&found[found.len() - 2..found.len()], 16).expect("Unexpected hex value")
                                as char,
                        );
                    }
                    _ => {
                        // not a supported character
                        panic!("Unexpected escape character: {}", parser_utils.buffer[token.end_offset - 1] as char)
                    }
                }
            }
            TokenTypes::StringStart => {} //the first token is always a StringStart, so skip this
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }
}

/// Parses a generic method call
fn parse_generic_method(effect: Option<Effects>, parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let name = parser_utils.tokens[parser_utils.index - 2].to_string(parser_utils.buffer);
    let token = parser_utils.index - 2;
    // Get the explicit generics
    let explicit_generics: Vec<UnparsedType> = if let UnparsedType::Generic(_, bounds) =
        parse_generics(UnparsedType::Basic(Span::default(), String::default()), parser_utils)
    {
        //TODO figure out how to check for un-resolved generics with generic method calls
        bounds
    } else {
        vec![]
    };
    if parser_utils.tokens[parser_utils.index].token_type == TokenTypes::Colon
        && parser_utils.tokens[parser_utils.index + 1].token_type == TokenTypes::Colon
        && parser_utils.tokens[parser_utils.index + 2].token_type == TokenTypes::Variable
    {
        let calling = parser_utils.tokens[parser_utils.index + 2].to_string(parser_utils.buffer);
        parser_utils.index += 4;
        let out = Effects {
            types: EffectType::MethodCall(
                effect.map(|inner| Box::new(inner)),
                format!("{}::{}", name.clone(), calling),
                get_effects(parser_utils)?,
                explicit_generics,
            ),
            span: Span::new(parser_utils.file, token),
        };
        return Ok(out);
    }
    parser_utils.index += 1;
    return Ok(Effects {
        types: EffectType::MethodCall(
            effect.map(|inner| Box::new(inner)),
            name.clone(),
            get_effects(parser_utils)?,
            explicit_generics,
        ),
        span: Span::new(parser_utils.file, token),
    });
}

/// Gets a list of effect arguments
fn get_effects(parser_utils: &mut ParserUtils) -> Result<Vec<Effects>, ParsingError> {
    let mut effects = Vec::default();
    // Parse the method call arguments
    if parser_utils.tokens[parser_utils.index].token_type != TokenTypes::ParenClose {
        let start = parser_utils.index;
        while let Some(mut expression) = parse_line(parser_utils, ParseState::None)? {
            expression.effect.span.extend_span_backwards(start);
            effects.push(expression.effect);
            if parser_utils.tokens[parser_utils.index].token_type != TokenTypes::ArgumentEnd {
                break;
            }
            parser_utils.index += 1;
        }
    }
    parser_utils.index += 1;
    return Ok(effects);
}

/// Parses a let statement
fn parse_let(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let name;
    let mut error_token;
    {
        let next = &parser_utils.tokens[parser_utils.index];
        if TokenTypes::Variable == next.token_type {
            name = next.to_string(parser_utils.buffer);
        } else {
            return Err(Span::new(parser_utils.file, parser_utils.index).make_error(ParsingMessage::UnexpectedToken));
        }

        if TokenTypes::Equals != parser_utils.tokens.get(parser_utils.index + 1).unwrap().token_type {
            return Err(Span::new(parser_utils.file, parser_utils.index).make_error(ParsingMessage::UnexpectedSymbol));
        }
        parser_utils.index += 2;
        error_token = Span::new(parser_utils.file, parser_utils.index);
    }

    // If the rest of the line doesn't exist, return an error because the value must be set to something.
    return match parse_line(parser_utils, ParseState::None)? {
        Some(line) => {
            error_token.extend_span(parser_utils.index - 2);
            Ok(Effects::new(error_token, EffectType::CreateVariable(name, Box::new(line.effect))))
        }
        None => Err(Span::new(parser_utils.file, parser_utils.index).make_error(ParsingMessage::UnexpectedVoid)),
    };
}

/// Parses a new program call
fn parse_new(parser_utils: &mut ParserUtils, span: &Span) -> Result<Effects, ParsingError> {
    let mut types: Option<UnparsedType> = None;

    let values;

    let type_token = parser_utils.index;

    loop {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => {
                types = Some(UnparsedType::Basic(
                    Span::new(parser_utils.file, parser_utils.index - 1),
                    token.to_string(parser_utils.buffer),
                ))
            }
            //Handle making new structs with generics.
            TokenTypes::Operator => {
                types = Some(parse_generics(types.unwrap(), parser_utils));
            }
            TokenTypes::BlockStart => {
                values = parse_new_args(parser_utils, span)?;
                break;
            }
            TokenTypes::InvalidCharacters => {}
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    return Ok(Effects::new(Span::new(parser_utils.file, type_token), EffectType::CreateStruct(types.unwrap(), values)));
}

/// Parses the arguments to a new struct effect
fn parse_new_args(parser_utils: &mut ParserUtils, span: &Span) -> Result<Vec<(String, Effects)>, ParsingError> {
    let mut values = Vec::default();
    let mut name = String::default();
    loop {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => name = token.to_string(parser_utils.buffer),
            TokenTypes::Colon | TokenTypes::ArgumentEnd => {
                let effect = if TokenTypes::Colon == token.token_type {
                    match parse_line(parser_utils, ParseState::New)? {
                        Some(inner) => inner.effect,
                        None => return Err(span.make_error(ParsingMessage::ExpectedEffect)),
                    }
                } else {
                    Effects::new(
                        Span::new(parser_utils.file, parser_utils.index - 1),
                        EffectType::LoadVariable(name.clone()),
                    )
                };
                values.push((name, effect));
                name = String::default();
                if parser_utils.tokens[parser_utils.index].token_type == TokenTypes::ArgumentEnd {
                    parser_utils.index += 1;
                }
            }
            TokenTypes::BlockEnd | TokenTypes::ParenClose => break,
            TokenTypes::InvalidCharacters => {}
            TokenTypes::Comment => {}
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    return Ok(values);
}

/// Checks if a type is generic or if it's just followed by an operator
fn is_generic(token: &Token, parser_utils: &ParserUtils) -> bool {
    let next: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
    return parser_utils.buffer[token.end_offset] != b' ' && next.to_string(parser_utils.buffer) == "<";
}
