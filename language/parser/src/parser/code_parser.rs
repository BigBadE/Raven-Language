use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;
use syntax::async_util::UnparsedType;
use crate::parser::control_parser::{parse_do_while, parse_for, parse_if, parse_while};
use crate::parser::operator_parser::parse_operator;
use crate::parser::util::{add_generics, ParserUtils};
use crate::tokens::tokens::{Token, TokenTypes};

/// Parsers a block of code into its return type (if all code paths lead to a single type, or else a line) and the code body.
pub fn parse_code(parser_utils: &mut ParserUtils) -> Result<(ExpressionType, CodeBody), ParsingError> {
    let mut lines = Vec::new();
    let mut types = ExpressionType::Line;
    while let Some(expression) =
        parse_line(parser_utils, ParseState::None)? {
        if expression.expression_type != ExpressionType::Line {
            types = expression.expression_type;
        }
        lines.push(expression);
    }
    parser_utils.imports.last_id += 1;
    return Ok((types, CodeBody::new(lines, (parser_utils.imports.last_id - 1).to_string())));
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
    ControlOperator
}

pub fn parse_line(parser_utils: &mut ParserUtils, state: ParseState)
                  -> Result<Option<Expression>, ParsingError> {
    // The current effect
    let mut effect: Option<Effects> = None;
    // The current type of expression
    let mut expression_type = ExpressionType::Line;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap().clone();

        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::ParenOpen => {
                let last = parser_utils.tokens.get(parser_utils.index - 2).unwrap().clone();
                match last.token_type {
                    TokenTypes::Variable | TokenTypes::CallingType => {
                        let mut effects = Vec::new();
                        if parser_utils.tokens.get(parser_utils.index).unwrap().token_type != TokenTypes::ParenClose {
                            // If there are arguments to the method, parse them
                            while let Some(expression) = parse_line(parser_utils, ParseState::None)? {
                                effects.push(expression.effect);
                                if parser_utils.tokens.get(parser_utils.index - 1).unwrap().token_type
                                    == TokenTypes::ArgumentEnd {} else {
                                    break;
                                }
                            }
                        } else {
                            // No arguments
                            parser_utils.index += 1;
                        }

                        // Name of the method = the last token
                        let name = last.to_string(parser_utils.buffer);
                        // The calling effect must be boxed if it exists.
                        effect = Some(Effects::MethodCall(effect.map(|inner| Box::new(inner)),
                                                          name.clone(), effects, None));
                    }
                    // If it's not a method call, it's a parenthesized effect.
                    _ => if let Some(expression) = parse_line(parser_utils, state.clone())? {
                        effect = Some(Effects::Paren(Box::new(expression.effect)));
                    } else {
                        //effect = None;
                        panic!("Unknown code path - report this!");
                    }
                }
            }
            TokenTypes::Float => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected float! Did you forget a semicolon?")));
                }
                effect = Some(Effects::Float(token.to_string(parser_utils.buffer).parse().unwrap()))
            }
            TokenTypes::Integer => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected integer! Did you forget a semicolon? {:?}", effect.unwrap())));
                }
                effect = Some(Effects::Int(token.to_string(parser_utils.buffer).parse().unwrap()))
            }
            TokenTypes::True => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected boolean! Did you forget a semicolon?")));
                }
                effect = Some(Effects::Bool(true))
            }
            TokenTypes::False => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected boolean! Did you forget a semicolon?")));
                }
                effect = Some(Effects::Bool(false))
            }
            TokenTypes::StringStart => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected string! Did you forget a semicolon?")));
                }
                effect = Some(parse_string(parser_utils)?)
            }
            TokenTypes::LineEnd | TokenTypes::ParenClose => break,
            TokenTypes::CodeEnd | TokenTypes::BlockEnd => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(),
                                                format!("Unexpected code end! Did you forget a semicolon?")));
                }
                return Ok(None)
            },
            TokenTypes::Variable => {
                let next = parser_utils.tokens.get(parser_utils.index).unwrap();
                if let TokenTypes::ParenOpen = next.token_type {
                    //Skip because ParenOpen handles this.
                } else if let TokenTypes::Operator = next.token_type {
                    //Skip if a generic method is being called next to preserve the last effect.
                    if next.to_string(parser_utils.buffer) == "<" &&
                        token.to_string(parser_utils.buffer).bytes().last().unwrap() != b' ' {
                        continue
                    } else {
                        effect = Some(
                            Effects::LoadVariable(token.to_string(parser_utils.buffer)))
                    }
                } else {
                    if effect.is_some() {
                        return Err(token.make_error(parser_utils.file.clone(),
                                                    format!("Unexpected value! Did you forget a semicolon?")));
                    }
                    effect = Some(
                        Effects::LoadVariable(token.to_string(parser_utils.buffer)))
                }
            },
            TokenTypes::Return => {
                expression_type = ExpressionType::Return
            },
            TokenTypes::New => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected new! Did you forget a semicolon?")));
                }
                effect = Some(parse_new(parser_utils)?);
            },
            TokenTypes::BlockStart => if ParseState::ControlVariable == state || ParseState::ControlOperator == state {
                break;
            } else {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected block! Did you forget a semicolon?")));
                }

                // Get the code in the next block.
                let (returning, body) = parse_code(parser_utils)?;
                // If the inner block returns/breaks, then the outer one should too
                if expression_type == ExpressionType::Line {
                    expression_type = returning;
                }
                effect = Some(Effects::CodeBody(body));
            },
            TokenTypes::Let => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected let! Did you forget a semicolon?")));
                }
                return Ok(Some(Expression::new(expression_type, parse_let(parser_utils)?)))
            },
            TokenTypes::If => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected if! Did you forget a semicolon?")));
                }

                let expression = parse_if(parser_utils)?;
                // If the if returns/breaks, the outer block should too
                if expression_type == ExpressionType::Line {
                    expression_type = expression.expression_type;
                }
                return Ok(Some(Expression::new(expression_type, expression.effect)));
            }
            TokenTypes::For => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected for! Did you forget a semicolon?")));
                }
                return Ok(Some(Expression::new(expression_type, parse_for(parser_utils)?)))
            },
            TokenTypes::While => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected for! Did you forget a semicolon?")));
                }
                return Ok(Some(Expression::new(expression_type, parse_while(parser_utils)?)))
            },
            TokenTypes::Do => {
                if effect.is_some() {
                    return Err(token.make_error(parser_utils.file.clone(), format!("Unexpected for! Did you forget a semicolon?")));
                }
                return Ok(Some(Expression::new(expression_type, parse_do_while(parser_utils)?)))
            },
            TokenTypes::Equals => {
                let other = parser_utils.tokens.get(parser_utils.index).unwrap().token_type.clone();
                // Check to make sure this isn't an operation like == or +=
                if effect.is_some() && other != TokenTypes::Operator && other != TokenTypes::Equals {
                    let value = parse_line(parser_utils, ParseState::None)?;
                    if let Some(value) = value {
                        effect = Some(Effects::Set(Box::new(effect.unwrap()), Box::new(value.effect)));
                    } else {
                        return Err(token.make_error(parser_utils.file.clone(), "Tried to assign a void value!".to_string()));
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
                if (last.token_type == TokenTypes::Variable || last.token_type == TokenTypes::CallingType) &&
                    token.to_string(parser_utils.buffer) == "<" &&
                    last.to_string(parser_utils.buffer).bytes().last().unwrap() != b' ' {
                    effect = Some(parse_generic_method(effect, parser_utils)?);
                } else {
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
            TokenTypes::ArgumentEnd => break,
            TokenTypes::CallingType => {
                let next: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
                if next.token_type == TokenTypes::ParenOpen ||
                    (next.token_type == TokenTypes::Operator && next.to_string(parser_utils.buffer) == "<" &&
                    token.to_string(parser_utils.buffer).bytes().last().unwrap() != b' ') {
                    // Ignored, ParenOpen or Operator handles this
                } else {
                    if effect.is_none() {
                        return Err(token.make_error(parser_utils.file.clone(), format!("Extra symbol!")));
                    }
                    effect = Some(Effects::Load(Box::new(effect.unwrap()),
                                                token.to_string(parser_utils.buffer)))
                }
            },
            TokenTypes::EOF => {
                return Ok(None);
            }
            TokenTypes::Else => return Err(token.make_error(parser_utils.file.clone(),
                                                            "Unexpected Else!".to_string())),
            TokenTypes::Period => if parser_utils.tokens[parser_utils.index].token_type == TokenTypes::Period {
                let operator = parse_operator(effect, parser_utils, &state)?;
                // Operators inside operators return immediately so operators can be combined
                // later on for operators like [].
                if ParseState::InOperator == state || ParseState::ControlOperator == state {
                    return Ok(Some(Expression::new(expression_type, operator)));
                } else {
                    effect = Some(operator);
                }
            },
            TokenTypes::Comment => {},
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return Ok(Some(Expression::new(expression_type, effect.unwrap_or(Effects::NOP()))));
}

fn parse_string(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let mut string = String::new();
    loop {
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
                let found = token.to_string(parser_utils.buffer);
                string += &found[0..found.len() - 1];
            }
            TokenTypes::StringStart => {}
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }
}

/// Parses a generic method call
fn parse_generic_method(effect: Option<Effects>, parser_utils: &mut ParserUtils)
    -> Result<Effects, ParsingError> {
    let name = parser_utils.tokens.get(parser_utils.index-2).unwrap().to_string(parser_utils.buffer);
    // Get the type being expressed. Should only be one type.
    let returning: Option<UnparsedType> = if let UnparsedType::Generic(_, bounds) = add_generics(String::new(), parser_utils).0 {
        if bounds.len() != 1 {
            parser_utils.tokens.get(parser_utils.index-1).unwrap().make_error(parser_utils.file.clone(),
            format!("Expected one generic argument!"));
        }
        let types: &UnparsedType = bounds.get(0).unwrap();
        Some(types.clone())
    } else {
        None
    };

    parser_utils.index += 1;
    let mut effects = Vec::new();
    // Parse the method call arguments
    if parser_utils.tokens.get(parser_utils.index).unwrap().token_type != TokenTypes::ParenClose {
        while let Some(expression) = parse_line(parser_utils, ParseState::None)? {
            effects.push(expression.effect);
            if parser_utils.tokens.get(parser_utils.index - 1).unwrap().token_type
                == TokenTypes::ArgumentEnd {} else {
                break;
            }
        }
    } else {
        parser_utils.index += 1;
    }

    return Ok(Effects::MethodCall(effect.map(|inner| Box::new(inner)),
                                      name.clone(), effects, returning));
}

fn parse_let(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let name;
    {
        let next = parser_utils.tokens.get(parser_utils.index).unwrap();
        if let TokenTypes::Variable = next.token_type {
            name = next.to_string(parser_utils.buffer);
        } else {
            return Err(next.make_error(parser_utils.file.clone(), "Unexpected token, expected variable name!".to_string()));
        }

        if let TokenTypes::Equals = parser_utils.tokens.get(parser_utils.index + 1).unwrap().token_type {} else {
            return Err(next.make_error(parser_utils.file.clone(), format!("Unexpected {:?}, expected equals!", next)));
        }
        parser_utils.index += 2;
    }

    // If the rest of the line doesn't exist, return an error because the value must be set to something.
    return match parse_line(parser_utils, ParseState::None)? {
        Some(line) => Ok(Effects::CreateVariable(name, Box::new(line.effect))),
        None => Err(parser_utils.tokens.get(parser_utils.index).unwrap()
            .make_error(parser_utils.file.clone(), "Expected value, found void!".to_string()))
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
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return Ok(Effects::CreateStruct(types.unwrap(), values));
}

fn parse_new_args(parser_utils: &mut ParserUtils) -> Result<Vec<(String, Effects)>, ParsingError> {
    let mut values = Vec::new();
    let mut name = String::new();
    loop {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => name = token.to_string(parser_utils.buffer),
            TokenTypes::Colon | TokenTypes::ArgumentEnd => {
                let effect = if let TokenTypes::Colon = token.token_type {
                    let token = token.clone();
                    match parse_line(parser_utils, ParseState::None)? {
                        Some(inner) => inner.effect,
                        None => return Err(token.make_error(parser_utils.file.clone(), format!("Expected effect!")))
                    }
                } else {
                    Effects::LoadVariable(name.clone())
                };
                values.push((name, effect));
                name = String::new();
            }
            TokenTypes::BlockEnd => break,
            TokenTypes::InvalidCharacters => {},
            TokenTypes::Comment => {},
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return Ok(values);
}