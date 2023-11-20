use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;

use crate::parser::code_parser::{parse_code, parse_line, ParseState};
use crate::{ParserUtils, TokenTypes};

/// Parses an if statement into a single expression.
pub fn parse_if(parser_utils: &mut ParserUtils) -> Result<Expression, ParsingError> {
    // Get the effect inside the if. The if token itself is already parsed, so next is whatever
    // is being checked.
    // ex:
    // if value == 2
    // This gets value == 2
    let effect = parse_line(parser_utils, ParseState::ControlVariable)?;
    if effect.is_none() {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Expected condition, found void".to_string(),
            ));
    }

    // Make sure the if statement ended with a bracket
    if parser_utils
        .tokens
        .get(parser_utils.index)
        .unwrap()
        .token_type
        != TokenTypes::BlockStart
    {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Expected body, found void".to_string(),
            ));
    }

    parser_utils.index += 1;

    // Get the code inside the if statement
    let (mut returning, body) = parse_code(parser_utils)?;
    let mut else_ifs = Vec::default();
    let mut else_body = None;

    // Loop over every else block
    while parser_utils
        .tokens
        .get(parser_utils.index)
        .unwrap()
        .token_type
        == TokenTypes::Else
    {
        // Else ifs get added to the else if
        if parser_utils
            .tokens
            .get(parser_utils.index + 1)
            .unwrap()
            .token_type
            == TokenTypes::If
        {
            parser_utils.index += 2;

            let effect = parse_line(parser_utils, ParseState::ControlVariable)?;
            if effect.is_none() {
                return Err(parser_utils
                    .tokens
                    .get(parser_utils.index)
                    .unwrap()
                    .make_error(
                        parser_utils.file.clone(),
                        "Expected condition, found void".to_string(),
                    ));
            }

            if parser_utils
                .tokens
                .get(parser_utils.index)
                .unwrap()
                .token_type
                != TokenTypes::BlockStart
            {
                return Err(parser_utils
                    .tokens
                    .get(parser_utils.index)
                    .unwrap()
                    .make_error(
                        parser_utils.file.clone(),
                        "Expected body, found void".to_string(),
                    ));
            }

            parser_utils.index += 1;

            // Get the body of the else if block
            let (other_returning, body) = parse_code(parser_utils)?;
            // An if statement is only the return of the block if every code path returns, so if they differ
            // this can't be a return block.
            if other_returning != returning {
                returning = ExpressionType::Line;
            }
            else_ifs.push((effect.unwrap().effect, body));
        } else if parser_utils
            .tokens
            .get(parser_utils.index + 1)
            .unwrap()
            .token_type
            == TokenTypes::BlockStart
        {
            parser_utils.index += 2;
            // Get the else body
            let (other_returning, body) = parse_code(parser_utils)?;
            // Check to make sure the else body returns if the other bodies do.
            if other_returning != returning {
                returning = ExpressionType::Line;
            }
            else_body = Some(body);
            break;
        } else {
            return Err(parser_utils
                .tokens
                .get(parser_utils.index)
                .unwrap()
                .make_error(parser_utils.file.clone(), "Expected block!".to_string()));
        }
    }

    // If there is no else, the if statement can't be the return.
    if else_body.is_none() {
        returning = ExpressionType::Line;
    }

    let adding = 1 + else_ifs.len() as u32 + else_body.is_some() as u32;
    parser_utils.imports.last_id += adding;
    return Ok(Expression::new(
        returning,
        create_if(
            effect.unwrap().effect,
            body,
            else_ifs,
            else_body,
            parser_utils.imports.last_id - adding,
        )?,
    ));
}

pub fn parse_for(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let name = parser_utils.tokens.get(parser_utils.index).unwrap();
    parser_utils.index += 1;
    // Gets the name of the for loop variable
    if name.token_type != TokenTypes::Variable {
        return Err(name.make_error(
            parser_utils.file.clone(),
            "Expected variable name!".to_string(),
        ));
    }

    // Checks for the "in" keyword
    if parser_utils
        .tokens
        .get(parser_utils.index)
        .unwrap()
        .token_type
        != TokenTypes::In
    {
        return Err(name.make_error(
            parser_utils.file.clone(),
            "Missing \"in\" in for loop.".to_string(),
        ));
    }
    parser_utils.index += 1;

    let name = name.to_string(parser_utils.buffer);

    // Gets the variable we're looping over
    let effect = parse_line(parser_utils, ParseState::ControlVariable)?;
    if effect.is_none() {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Expected iterator, found void".to_string(),
            ));
    }

    // Checks for the code start
    if parser_utils
        .tokens
        .get(parser_utils.index)
        .unwrap()
        .token_type
        != TokenTypes::BlockStart
    {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index - 1)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Missing code body for loop.".to_string(),
            ));
    }
    parser_utils.index += 1;

    // Parses the body of the for loop
    let body = parse_code(parser_utils)?.1;
    parser_utils.imports.last_id += 2;

    // Returns the finished for loop.
    return create_for(
        name,
        effect.unwrap().effect,
        body,
        parser_utils.imports.last_id - 2,
    );
}

pub fn parse_while(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let effect = parse_line(parser_utils, ParseState::ControlVariable)?;
    if effect.is_none() {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Expected condition, found void".to_string(),
            ));
    }

    if parser_utils
        .tokens
        .get(parser_utils.index)
        .unwrap()
        .token_type
        != TokenTypes::BlockStart
    {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Expected body, found void".to_string(),
            ));
    }

    parser_utils.index += 1;

    let (_returning, body) = parse_code(parser_utils)?;
    parser_utils.imports.last_id += 1;
    return create_while(
        effect.unwrap().effect,
        body,
        parser_utils.imports.last_id - 1,
    );
}

pub fn parse_do_while(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    if parser_utils
        .tokens
        .get(parser_utils.index)
        .unwrap()
        .token_type
        != TokenTypes::BlockStart
    {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Expected body, found void".to_string(),
            ));
    }
    parser_utils.index += 1;

    let (_returning, body) = parse_code(parser_utils)?;

    if parser_utils
        .tokens
        .get(parser_utils.index)
        .unwrap()
        .token_type
        != TokenTypes::While
    {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(parser_utils.file.clone(), "Expected while!".to_string()));
    }

    parser_utils.index += 1;

    let effect = parse_line(parser_utils, ParseState::ControlVariable)?;
    if effect.is_none() {
        return Err(parser_utils
            .tokens
            .get(parser_utils.index)
            .unwrap()
            .make_error(
                parser_utils.file.clone(),
                "Expected condition, found void".to_string(),
            ));
    }

    parser_utils.imports.last_id += 1;
    return create_do_while(
        effect.unwrap().effect,
        body,
        parser_utils.imports.last_id - 1,
    );
}

fn create_do_while(effect: Effects, mut body: CodeBody, id: u32) -> Result<Effects, ParsingError> {
    let mut top = Vec::default();

    let label = body.label.clone();
    body.expressions.push(Expression::new(
        ExpressionType::Line,
        Effects::Jump((id - 1).to_string() + "end"),
    ));
    top.push(Expression::new(
        ExpressionType::Line,
        Effects::CodeBody(body),
    ));
    top.push(Expression::new(
        ExpressionType::Line,
        Effects::CompareJump(Box::new(effect), label, id.to_string() + "end"),
    ));

    return Ok(Effects::CodeBody(CodeBody::new(top, id.to_string())));
}

fn create_while(effect: Effects, mut body: CodeBody, id: u32) -> Result<Effects, ParsingError> {
    let mut top = Vec::default();

    top.push(Expression::new(
        ExpressionType::Line,
        Effects::CompareJump(Box::new(effect), body.label.clone(), id.to_string() + "end"),
    ));
    body.expressions.push(Expression::new(
        ExpressionType::Line,
        Effects::Jump(id.to_string()),
    ));
    top.push(Expression::new(
        ExpressionType::Line,
        Effects::CodeBody(body),
    ));

    return Ok(Effects::CodeBody(CodeBody::new(top, id.to_string())));
}

fn create_if(
    effect: Effects,
    body: CodeBody,
    mut else_ifs: Vec<(Effects, CodeBody)>,
    else_body: Option<CodeBody>,
    id: u32,
) -> Result<Effects, ParsingError> {
    let mut body = body;

    // Maps the else body, if there is an else_if there needs to be an empty else to put the else if into.
    let mut else_body = if let Some(mut body) = else_body {
        body.expressions.push(Expression::new(
            ExpressionType::Line,
            Effects::Jump(id.to_string() + "end"),
        ));
        Some(body)
    } else if !else_ifs.is_empty() {
        Some(CodeBody::new(Vec::default(), id.to_string()))
    } else {
        None
    };

    let mut temp = id + 1;
    // Add every else if statement
    while !else_ifs.is_empty() {
        let (effect, mut body) = else_ifs.remove(0);
        body.expressions.push(Expression::new(
            ExpressionType::Line,
            Effects::Jump(id.to_string() + "end"),
        ));
        // Creates the body of the else if by adding another if statement to the top of the else.
        let inner = CodeBody::new(
            vec![
                Expression::new(
                    ExpressionType::Line,
                    Effects::CompareJump(
                        Box::new(effect),
                        body.label.clone(),
                        else_body.as_ref().unwrap().label.clone(),
                    ),
                ),
                Expression::new(ExpressionType::Line, Effects::CodeBody(body)),
                Expression::new(ExpressionType::Line, Effects::CodeBody(else_body.unwrap())),
            ],
            temp.to_string(),
        );
        else_body = Some(inner);
        temp += 1;
    }

    // Where we jump if the if fails
    let if_jumping = if let Some(body) = &mut else_body {
        body.expressions.push(Expression::new(
            ExpressionType::Line,
            Effects::Jump(body.label.clone()),
        ));
        body.label.clone()
    } else {
        id.to_string() + "end"
    };

    body.expressions.push(Expression::new(
        ExpressionType::Line,
        Effects::Jump(id.to_string() + "end"),
    ));

    // The CodeBody before the if statement that controls the control flow
    let mut top = CodeBody::new(
        vec![
            Expression::new(
                ExpressionType::Line,
                Effects::CompareJump(Box::new(effect), body.label.clone(), if_jumping),
            ),
            Expression::new(ExpressionType::Line, Effects::CodeBody(body)),
        ],
        id.to_string(),
    );

    // Add the else body.
    if let Some(body) = else_body {
        top.expressions.push(Expression::new(
            ExpressionType::Line,
            Effects::CodeBody(body),
        ));
    }

    return Ok(Effects::CodeBody(top));
}

fn create_for(
    name: String,
    effect: Effects,
    mut body: CodeBody,
    id: u32,
) -> Result<Effects, ParsingError> {
    let mut top = Vec::default();
    let variable = format!("$iter{}", id);
    top.insert(
        0,
        Expression::new(
            ExpressionType::Line,
            Effects::CreateVariable(variable.clone(), Box::new(effect)),
        ),
    );
    top.push(Expression::new(
        ExpressionType::Line,
        Effects::Jump((id + 1).to_string()),
    ));
    // Adds a call to the Iter::next function at the top of the for loop.
    body.expressions.insert(
        0,
        Expression::new(
            ExpressionType::Line,
            Effects::CreateVariable(
                name.clone(),
                Box::new(Effects::ImplementationCall(
                    Box::new(Effects::LoadVariable(variable.clone())),
                    "iter::Iter".to_string(),
                    "next".to_string(),
                    vec![],
                    None,
                )),
            ),
        ),
    );

    // Jumps to the header of the for loop after each loop
    body.expressions.push(Expression::new(
        ExpressionType::Line,
        Effects::Jump((id + 1).to_string()),
    ));

    let for_check = CodeBody::new(
        vec![Expression::new(
            ExpressionType::Line,
            Effects::CompareJump(
                Box::new(Effects::ImplementationCall(
                    Box::new(Effects::LoadVariable(variable.clone())),
                    "iter::Iter".to_string(),
                    "has_next".to_string(),
                    vec![],
                    None,
                )),
                body.label.clone(),
                id.to_string() + "end",
            ),
        )],
        (id + 1).to_string(),
    );
    // Checks if the end is reached, and if so jumps to the end of the block.
    // The block after is named id + end so it can be named before it exists.
    top.push(Expression::new(
        ExpressionType::Line,
        Effects::CodeBody(for_check),
    ));
    top.push(Expression::new(
        ExpressionType::Line,
        Effects::CodeBody(body),
    ));

    return Ok(Effects::CodeBody(CodeBody::new(top, id.to_string())));
}
