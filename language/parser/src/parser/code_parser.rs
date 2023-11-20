use crate::parser::control_parser::{parse_do_while, parse_for, parse_if, parse_while};
use crate::parser::operator_parser::parse_operator;
use crate::parser::util::{add_generics, ParserUtils};
use crate::tokens::tokens::{Token, TokenTypes};
use std::mem;
use syntax::async_util::UnparsedType;
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;

/// Parsers a block of code into its return type (if all code paths lead to a single type, or else a line) and the code body.
pub fn parse_code(
    parser_utils: &mut ParserUtils,
) -> Result<(ExpressionType, CodeBody), ParsingError> {
    let mut lines = Vec::default();
    let mut types = ExpressionType::Line;
    while let Some(expression) = parse_line(parser_utils, ParseState::None)? {
        if expression.expression_type != ExpressionType::Line {
            types = expression.expression_type;
        }
        lines.push(expression);
    }
    parser_utils.imports.last_id += 1;
    return Ok((
        types,
        CodeBody::new(lines, (parser_utils.imports.last_id - 1).to_string()),
    ));
}

#[derive(PartialEq, Clone)]
pub enum ParseState {
    None,
    // Used when inside the variable of a control statement.
    // Ex:
    // if value == 2 {
    // value == 2 would be parsed as a ControlVariable
    ControlVariable,
    // When inside the arguments of a function.
    // Ex:
    // printf("String");
    // "String" would be parsed as a argument.
    Argument,
    // When inside of an operator, such as 1 + 2
    InOperator,
    // When inside both an operator and control variable.
    ControlOperator,
    // When inside a new expression.
    New,
}

// skipcq: RS-R1000
pub fn parse_line(
    parser_utils: &mut ParserUtils,
    state: ParseState,
) -> Result<Option<Expression>, ParsingError> {
    // The current effect
    let mut effect: Option<Effects> = None;
    // The current type of expression
    let mut expression_type = ExpressionType::Line;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap().clone();

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
                    return Err(token.make_error(
                        parser_utils.file.clone(),
                        format!("Unexpected value! Did you forget a semicolon?"),
                    ));
                }
                _ => {}
            }
        }

        match parse_basic_line(
            parser_utils,
            &mut expression_type,
            &token,
            &state,
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
                        effect = Some(Effects::LoadVariable(token.to_string(parser_utils.buffer)))
                    }
                } else {
                    if effect.is_some() {
                        return Err(token.make_error(
                            parser_utils.file.clone(),
                            format!("Unexpected value! Did you forget a semicolon?"),
                        ));
                    }
                    effect = Some(Effects::LoadVariable(token.to_string(parser_utils.buffer)))
                }
            }
            TokenTypes::New => {
                if effect.is_some() {
                    return Err(token.make_error(
                        parser_utils.file.clone(),
                        format!("Unexpected new! Did you forget a semicolon?"),
                    ));
                }
                effect = Some(parse_new(parser_utils)?);
            }
            TokenTypes::BlockStart => {
                if ParseState::ControlVariable == state || ParseState::ControlOperator == state {
                    parser_utils.index -= 1;
                    break;
                } else {
                    if effect.is_some() {
                        return Err(token.make_error(
                            parser_utils.file.clone(),
                            format!("Unexpected block! Did you forget a semicolon?"),
                        ));
                    }

                    // Get the code in the next block.
                    let (returning, body) = parse_code(parser_utils)?;
                    // If the inner block returns/breaks, then the outer one should too
                    if expression_type == ExpressionType::Line {
                        expression_type = returning;
                    }
                    effect = Some(Effects::CodeBody(body));
                }
            }
            TokenTypes::Equals => {
                let other = parser_utils
                    .tokens
                    .get(parser_utils.index)
                    .unwrap()
                    .token_type
                    .clone();
                // Check to make sure this isn't an operation like == or +=
                if effect.is_some() && other != TokenTypes::Operator && other != TokenTypes::Equals
                {
                    let value = parse_line(parser_utils, ParseState::None)?;
                    if let Some(value) = value {
                        effect = Some(Effects::Set(
                            Box::new(effect.unwrap()),
                            Box::new(value.effect),
                        ));
                    } else {
                        return Err(token.make_error(
                            parser_utils.file.clone(),
                            "Tried to assign a void value!".to_string(),
                        ));
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
                // Example: test<Value>()
                parser_utils.index -= 1;
                if (last.token_type == TokenTypes::Variable
                    || last.token_type == TokenTypes::CallingType)
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
                        return Err(
                            token.make_error(parser_utils.file.clone(), format!("Extra symbol!"))
                        );
                    }
                    effect = Some(Effects::Load(
                        Box::new(effect.unwrap()),
                        token.to_string(parser_utils.buffer),
                    ))
                }
            }
            TokenTypes::Else => {
                return Err(
                    token.make_error(parser_utils.file.clone(), "Unexpected Else!".to_string())
                )
            }
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    return Ok(Some(Expression::new(
        expression_type,
        effect.unwrap_or(Effects::NOP),
    )));
}

enum ControlFlow {
    NotFound,
    Skipping,
    Finish,
    Returning(Expression),
}

// skipcq: RS-R1000
fn parse_basic_line(
    parser_utils: &mut ParserUtils,
    expression_type: &mut ExpressionType,
    token: &Token,
    state: &ParseState,
    effect: &mut Option<Effects>,
) -> Result<ControlFlow, ParsingError> {
    return Ok(match token.token_type {
        TokenTypes::BlockEnd if *state == ParseState::New => ControlFlow::Finish,
        TokenTypes::Return => {
            *expression_type = ExpressionType::Return;
            ControlFlow::Skipping
        }
        TokenTypes::Float => {
            *effect = Some(Effects::Float(
                token.to_string(parser_utils.buffer).parse().unwrap(),
            ));
            ControlFlow::Skipping
        }
        TokenTypes::Integer => {
            *effect = Some(Effects::Int(
                token.to_string(parser_utils.buffer).parse().unwrap(),
            ));
            ControlFlow::Skipping
        }
        TokenTypes::Char => {
            *effect = Some(Effects::Char(
                token.to_string(parser_utils.buffer).as_bytes()[1] as char,
            ));
            ControlFlow::Skipping
        }
        TokenTypes::True => {
            *effect = Some(Effects::Bool(true));
            ControlFlow::Skipping
        }
        TokenTypes::False => {
            *effect = Some(Effects::Bool(false));
            ControlFlow::Skipping
        }
        TokenTypes::StringStart => {
            *effect = Some(parse_string(parser_utils)?);
            ControlFlow::Skipping
        }
        TokenTypes::Let => {
            ControlFlow::Returning(Expression::new(*expression_type, parse_let(parser_utils)?))
        }
        TokenTypes::If => {
            let expression = parse_if(parser_utils)?;
            let mut expression_type = *expression_type;
            // If the if returns/breaks, the outer block should too
            if expression_type == ExpressionType::Line {
                expression_type = expression.expression_type;
            }
            ControlFlow::Returning(Expression::new(expression_type, expression.effect))
        }
        TokenTypes::For => {
            ControlFlow::Returning(Expression::new(*expression_type, parse_for(parser_utils)?))
        }
        TokenTypes::While => ControlFlow::Returning(Expression::new(
            *expression_type,
            parse_while(parser_utils)?,
        )),
        TokenTypes::Do => ControlFlow::Returning(Expression::new(
            *expression_type,
            parse_do_while(parser_utils)?,
        )),
        TokenTypes::LineEnd | TokenTypes::ParenClose | TokenTypes::ArgumentEnd => {
            ControlFlow::Finish
        }
        TokenTypes::Comment => ControlFlow::Skipping,
        TokenTypes::ParenOpen => {
            let last = parser_utils
                .tokens
                .get(parser_utils.index - 2)
                .unwrap()
                .clone();
            match last.token_type {
                TokenTypes::Variable | TokenTypes::CallingType => {
                    // Name of the method = the last token
                    let name = last.to_string(parser_utils.buffer);
                    let mut temp = None;
                    mem::swap(&mut temp, effect);
                    // The calling effect must be boxed if it exists.
                    *effect = Some(Effects::MethodCall(
                        temp.map(|inner| Box::new(inner)),
                        name.clone(),
                        get_effects(parser_utils)?,
                        None,
                    ));
                    ControlFlow::Skipping
                }
                // If it's not a method call, it's a parenthesized effect.
                _ => {
                    if let Some(expression) = parse_line(parser_utils, ParseState::None)? {
                        *effect = Some(Effects::Paren(Box::new(expression.effect)));
                        ControlFlow::Skipping
                    } else {
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
                    return Ok(ControlFlow::Returning(Expression::new(
                        *expression_type,
                        operator,
                    )));
                } else {
                    *effect = Some(operator);
                }
            }
            ControlFlow::Skipping
        }
        _ => ControlFlow::NotFound,
    });
}

///Parses tokens from the Raven code into a string
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
                return Ok(Effects::String(string + "\0"));
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
                let index = if is_hex {
                    found.len() - 3
                } else {
                    found.len() - 1
                };
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
                            u8::from_str_radix(&found[found.len() - 2..found.len()], 16)
                                .expect("Unexpected hex value") as char,
                        );
                    }
                    _ => {
                        // not a supported character
                        panic!(
                            "Unexpected escape character: {}",
                            parser_utils.buffer[token.end_offset - 1] as char
                        )
                    }
                }
            }
            TokenTypes::StringStart => {} //the first token is always a StringStart, so skip this
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }
}

/// Parses a generic method call
fn parse_generic_method(
    effect: Option<Effects>,
    parser_utils: &mut ParserUtils,
) -> Result<Effects, ParsingError> {
    let name = parser_utils
        .tokens
        .get(parser_utils.index - 2)
        .unwrap()
        .to_string(parser_utils.buffer);
    // Get the type being expressed. Should only be one type.
    let returning: Option<UnparsedType> =
        if let UnparsedType::Generic(_, bounds) = add_generics(String::default(), parser_utils).0 {
            if bounds.len() != 1 {
                parser_utils
                    .tokens
                    .get(parser_utils.index - 1)
                    .unwrap()
                    .make_error(
                        parser_utils.file.clone(),
                        format!("Expected one generic argument!"),
                    );
            }
            let types: &UnparsedType = bounds.first().unwrap();
            Some(types.clone())
        } else {
            None
        };

    parser_utils.index += 1;
    return Ok(Effects::MethodCall(
        effect.map(|inner| Box::new(inner)),
        name.clone(),
        get_effects(parser_utils)?,
        returning,
    ));
}

fn get_effects(parser_utils: &mut ParserUtils) -> Result<Vec<Effects>, ParsingError> {
    let mut effects = Vec::default();
    // Parse the method call arguments
    if parser_utils
        .tokens
        .get(parser_utils.index)
        .unwrap()
        .token_type
        != TokenTypes::ParenClose
    {
        while let Some(expression) = parse_line(parser_utils, ParseState::None)? {
            effects.push(expression.effect);
            if parser_utils
                .tokens
                .get(parser_utils.index - 1)
                .unwrap()
                .token_type
                == TokenTypes::ArgumentEnd
            {
            } else {
                break;
            }
        }
    } else {
        parser_utils.index += 1;
    }
    return Ok(effects);
}

fn parse_let(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let name;
    {
        let next = parser_utils.tokens.get(parser_utils.index).unwrap();
        if TokenTypes::Variable == next.token_type {
            name = next.to_string(parser_utils.buffer);
        } else {
            return Err(next.make_error(
                parser_utils.file.clone(),
                "Unexpected token, expected variable name!".to_string(),
            ));
        }

        if TokenTypes::Equals
            != parser_utils
                .tokens
                .get(parser_utils.index + 1)
                .unwrap()
                .token_type
        {
            return Err(next.make_error(
                parser_utils.file.clone(),
                format!("Unexpected {:?}, expected equals!", next),
            ));
        }
        parser_utils.index += 2;
    }

    // If the rest of the line doesn't exist, return an error because the value must be set to something.
    return match parse_line(parser_utils, ParseState::None)? {
        Some(line) => Ok(Effects::CreateVariable(name, Box::new(line.effect))),
        None => Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Expected value, found void!".to_string(),
            )),
    };
}

fn parse_new(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let mut types: Option<UnparsedType> = None;

    let values;

    loop {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => {
                types = Some(UnparsedType::Basic(token.to_string(parser_utils.buffer)))
            }
            //Handle making new structs with generics.
            TokenTypes::Operator => {
                types = Some(add_generics(types.unwrap().to_string(), parser_utils).0);
            }
            TokenTypes::BlockStart => {
                values = parse_new_args(parser_utils)?;
                break;
            }
            TokenTypes::InvalidCharacters => {}
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    return Ok(Effects::CreateStruct(types.unwrap(), values));
}

fn parse_new_args(parser_utils: &mut ParserUtils) -> Result<Vec<(String, Effects)>, ParsingError> {
    let mut values = Vec::default();
    let mut name = String::default();
    loop {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => name = token.to_string(parser_utils.buffer),
            TokenTypes::Colon | TokenTypes::ArgumentEnd => {
                let effect = if TokenTypes::Colon == token.token_type {
                    let token = token.clone();
                    match parse_line(parser_utils, ParseState::New)? {
                        Some(inner) => inner.effect,
                        None => {
                            return Err(token.make_error(
                                parser_utils.file.clone(),
                                format!("Expected effect!"),
                            ))
                        }
                    }
                } else {
                    Effects::LoadVariable(name.clone())
                };
                values.push((name, effect));
                name = String::default();
            }
            TokenTypes::BlockEnd => break,
            TokenTypes::LineEnd => {
                if parser_utils
                    .tokens
                    .get(parser_utils.index - 2)
                    .unwrap()
                    .token_type
                    == TokenTypes::BlockEnd
                {
                    parser_utils.index -= 1;
                    break;
                }
            }
            TokenTypes::InvalidCharacters => {}
            TokenTypes::Comment => {}
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    return Ok(values);
}

fn is_generic(token: &Token, parser_utils: &ParserUtils) -> bool {
    let next: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
    return parser_utils.buffer[token.end_offset] != b' '
        && next.to_string(parser_utils.buffer) == "<";
}
