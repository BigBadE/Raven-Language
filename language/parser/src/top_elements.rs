use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use anyhow::Error;
use syntax::function::{CodeBody, Function};
use syntax::{Attribute, get_modifier, ImplManager, is_modifier, Modifier, ParsingError};
use syntax::code::{Field, MemberField};
use syntax::r#struct::Struct;
use syntax::syntax::Syntax;
use syntax::type_resolver::TypeResolver;
use crate::imports::ImportManager;
use crate::literal::{parse_ident, parse_with_references};
use crate::parser::ParseInfo;
use crate::util::{find_if_first, parse_code_block, parse_fields, parse_generics, parse_generics_vec};

pub fn parse_top_elements(syntax: &Arc<Mutex<Syntax>>,
                          name: &String, mut parsing: ParseInfo) -> Result<(), Error> {
    let mut import_manager = ImportManager::new(name.clone());
    while let Some(_) = parsing.next_included() {
        parsing.index -= 1;
        let attributes = parse_attributes(&mut parsing, false);
        let modifiers = get_modifier(parse_modifiers(&mut parsing).as_slice());

        if parsing.matching("import") {
            let mut importing = parse_with_references(&mut parsing);
            import_manager.imports.insert(importing.split("::").last().unwrap().to_string(), importing);
            if !parsing.matching(";") {
                parsing.create_error("Missing semicolon!".to_string());
            }
        } else if parsing.matching("impl") {
            parse_impl(syntax, &mut import_manager, &mut parsing)?;
        } else if parsing.matching("trait") {
            parse_struct_type(syntax, &mut import_manager, modifiers, &mut parsing, true)?;
        } else if parsing.matching("struct") {
            parse_struct_type(syntax, &mut import_manager, modifiers, &mut parsing, false)?;
        } else if parsing.matching("fn") || is_modifier(modifiers, Modifier::Operation) {
            match parse_function(&mut import_manager, attributes, modifiers, &mut parsing, true) {
                Some(function) => {
                    syntax.add_function(function);
                }
                None => {}
            };
        } else {
            //Only error once for a big block of issues.
            if parsing.errors.last().is_none() || !parsing.errors.last().unwrap().error.starts_with("Unknown element") {
                let mut temp = parsing.clone();
                temp.skip_line();
                parsing.create_error(format!("Unknown element: {}",
                                             String::from_utf8_lossy(&parsing.buffer[parsing.index..temp.index - 1])));
            } else {
                parsing.skip_line();
            }
        }
    }
    return Ok(());
}

fn parse_impl(syntax: &Arc<Mutex<Syntax>>,
              import_manager: &mut ImportManager, parsing: &mut ParseInfo) -> Result<(), Error> {
    parsing.next_included();
    parsing.index -= 1;
    let mut last = parsing.loc();
    let implementing = match parsing.parse_to_space() {
        Some(found) => found,
        None => {
            syntax.lock().unwrap().structures.insert(import_manager.current.clone(),
                                                     Arc::new(Struct::new_poisoned("$file".to_string(),
                                                                                   ParsingError::new(last, parsing.loc(), "Unexpected EOF".to_string()))));
            return Ok(());
        }
    }.split("<").next().unwrap().to_string();
    if !parsing.matching("for") {
        parsing.create_error("Expected for in impl".to_string());
        return Ok(());
    }
    let base = match parsing.parse_to(b'{') {
        Some(found) => found,
        None => {
            parsing.create_error("Unexpected EOF".to_string());
            return Ok(());
        }
    }.split("<").next().unwrap().to_string();
    let mut functions = Vec::new();
    while match parsing.next_included() {
        Some(character) => character != b'}',
        None => {
            parsing.create_error("Unexpected EOF before end of impl!".to_string());
            return Ok(());
        }
    } {
        parsing.index -= 1;
        let attributes = parse_attributes(parsing, false);
        let modifiers = parse_modifiers(parsing);
        let mut import_manager = import_manager.clone();
        import_manager.current = implementing.clone();
        if parsing.matching("fn") {
            match parse_function(&mut import_manager, attributes,
                                 get_modifier(modifiers.as_slice()), parsing, true)? {
                Some(function) => {
                    functions.push(function);
                    continue;
                }
                None => {}
            }
        }
    }
    let locked = syntax.lock()?;
    locked.manager.handle().spawn(add_impl(syntax.clone(), base,
                                           implementing, functions,
                                           import_manager.clone()));
    return Ok(());
}

async fn add_impl(syntax: Arc<Mutex<Syntax>>, base: String,
                  implementing: String, functions: Vec<Arc<Function>>,
                  import_manager: ImportManager) {
    let import_manager = Box::new(import_manager);
    let mut base = Syntax::get_struct(syntax.clone(), base, import_manager.clone()).await;
    let implementing = Syntax::get_struct(syntax, implementing, import_manager).await;

    //This is safe because it satisfies both requirements of get_mut_unchecked:
    //1: No borrows of another type. Types don't ever return references to inner types.
    //2: No dereferenced borrows. No borrows are dereferenced until after everything is parsed.
    let mutable = unsafe { Arc::get_mut_unchecked(&mut base) };
    mutable.traits.push(implementing);
    for func in functions {
        mutable.functions.push(func);
    }
}

fn parse_struct_type(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager,
                     mut modifiers: u8, parsing: &mut ParseInfo, is_trait: bool) -> Result<(), Error> {
    let mut fn_name = String::new();
    let mut generics = Vec::new();

    if let Some(temp_name) = find_if_first(parsing, b'<', b'{') {
        fn_name = temp_name;

        parsing.matching("<");
        parse_generics_vec(parsing, &mut generics);
    }

    let mut parent_types = Vec::new();
    if let Some(temp_name) = find_if_first(parsing, b':', b'{') {
        fn_name = temp_name;
        let subtypes = match parsing.parse_to(b'{') {
            Some(found) => found,
            None => {
                parsing.create_error("Expected bracket!".to_string());
                return Ok(());
            }
        };
        let subtypes: Vec<String> = subtypes.split("+").map(
            |found| found.replace(" ", "").to_string()).collect();

        if subtypes.len() > 1 && !is_trait {
            parsing.create_error("Can't have multiple supertypes on a structure. Implement traits using the impl keyword!".to_string());
            return Ok(());
        }

        parent_types = subtypes;
    } else if fn_name.is_empty() {
        fn_name = match parsing.parse_to(b'{') {
            Some(name) => name.clone(),
            None => {
                parsing.create_error("Expected bracket!".to_string());
                return Ok(());
            }
        };
    } else {
        parsing.matching("{");
    }

    if !is_modifier(modifiers, Modifier::Internal) {
        fn_name = name.clone() + "::" + fn_name.as_str();
    }

    let mut functions = Vec::new();
    let mut fields = Vec::new();
    while match parsing.next_included() {
        Some(character) => character != b'}',
        None => {
            parsing.create_error("Unexpected EOF before end of struct!".to_string());
            return Ok(());
        }
    } {
        parsing.index -= 1;
        let attributes = parse_attributes(parsing, false);
        let modifiers = parse_modifiers(parsing);

        if parsing.matching("fn") {
            let mut import_manager = Box::new(import_manager.clone());
            import_manager.current = fn_name.clone();
            functions.push(parse_function(&mut import_manager, attributes, get_modifier(modifiers.as_slice()),
                                 parsing, !is_trait)?);
        }

        let field_name = match parsing.parse_to(b':') {
            Some(field_name) => field_name,
            None => {
                parsing.create_error("Expected field name!".to_string());
                return Ok(());
            }
        };

        let field_type = match parsing.parse_to(b';') {
            Some(field_type) => field_type,
            None => {
                parsing.create_error("Expected field type!".to_string());
                return Ok(());
            }
        };

        fields.push((get_modifier(modifiers.as_slice()), field_name, field_type));
    }

    if is_trait {
        if (modifiers & Modifier::Trait as u8) != 0 {
            panic!("Traits can't have internal or external modifiers!");
        }
        modifiers = modifiers + Modifier::Trait as u8;
    } else {
        if !parent_types.is_empty() && !fields.is_empty() {
            panic!("Subtypes can't declare new fields!");
        }
    }
    syntax.lock()?.manager.handle().spawn(add_struct(syntax.clone(), fields, generics,
                                                     functions, modifiers, fn_name, Box::new(import_manager.clone())));
    return Ok(());
}

async fn add_struct(syntax: Arc<Mutex<Syntax>>, fields: Vec<(u8, String, String)>, generics: Vec<(String, Vec<String>)>,
                    functions: Vec<impl Future<Output=Function>>, modifiers: u8, fn_name: String, import_manager: Box<ImportManager>) {
    let fields = fields.iter().map(|(modifiers, name, field_type)|
        MemberField::new(modifiers, Field::new(name,
                                               Syntax::get_struct(syntax.clone(),
                                                                  field_type, import_manager.clone()))))
        .collect();
    let generics = generics.iter().map(|(name, bounds)| bounds.iter().map(|bound|
        Syntax::get_struct(syntax.clone(), bound, import_manager.clone())).collect()).collect();

    let mut out_functions = Vec::new();
    for mut function in functions {
        let mut function = function.await;
        
        if function.fields.iter().any(|field| field.name == "self") {
            for generic in generics {
                function.generics.push(generic);
            }
            out_functions.push(Arc::new(function));
        } else {
            syntax.lock().unwrap().add_function(Arc::new(function));
        }
    }
    syntax.lock().unwrap().add_struct(Arc::new(Struct::new(fields, generics, out_functions, modifiers, fn_name)));
}

fn parse_function(import_manager: &mut ImportManager, attributes: HashMap<String, Attribute>, modifiers: u8,
                  parsing: &mut ParseInfo, parse_body: bool) -> Result<impl Future<Output=Function>, ParsingError> {
    let fn_name;
    let mut generics = HashMap::new();

    if let Some(found_name) = find_if_first(parsing, b'<', b'(') {
        fn_name = name.clone() + "::" + found_name.as_str();

        parse_generics(parsing, &mut generics);

        if parsing.next_included().is_none() {
            panic!("Expected function parameters!");
        }
    } else {
        fn_name = name.clone() + "::" + match parsing.parse_to(b'(') {
            Some(name) => name.clone(),
            None => {
                parsing.create_error("Expected string name".to_string());
                return Ok(None);
            }
        }.as_str();
    }

    let fields = match parse_fields(parent, parsing) {
        Some(fields) => fields,
        None => None
    };

    let return_type = if parsing.matching("->") {
        if parse_body {
            match parsing.parse_to(b'{') {
                Some(found) => {
                    parsing.index -= 1;
                    Some(ResolvableTypes::Resolving(found))
                }
                None => {
                    parsing.create_error("Expected code body".to_string());
                    return Ok(None);
                }
            }
        } else {
            match parsing.parse_to(b';') {
                Some(found) => {
                    parsing.index -= 1;
                    Some(ResolvableTypes::Resolving(found))
                }
                None => {
                    parsing.create_error("Expected no body on trait function".to_string());
                    return Ok(None);
                }
            }
        }
    } else {
        None
    };

    if !parse_body {
        if !parsing.matching(";") {
            parsing.create_error("Unexpected body on function!".to_string());
        }
        import_manager.code_block_id += 1;
        return Ok(Some(Arc::new(Function::new(attributes, modifiers, fields, generics,
                                              CodeBody::new(Vec::new(),
                                                            import_manager.code_block_id.to_string()),
                                              return_type, fn_name))));
    }

    let code = if !is_modifier(modifiers, Modifier::Internal) && !is_modifier(modifiers, Modifier::Extern) {
        match parse_code_block(syntax, import_manager, parsing) {
            Some(code) => code,
            None => return Ok(None)
        }
    } else {
        parsing.find_end();
        import_manager.code_block_id += 1;
        CodeBody::new(Vec::new(), import_manager.code_block_id.to_string())
    };

    return Ok(Some(Arc::new(Function::new(attributes, modifiers, fields, generics, code, return_type, fn_name))));
}

fn parse_modifiers(parsing: &mut ParseInfo) -> Vec<Modifier> {
    let mut modifiers = Vec::new();
    while let Some(modifier) = parse_modifier(parsing) {
        modifiers.push(modifier);
    }
    return modifiers;
}

fn parse_modifier(parsing: &mut ParseInfo) -> Option<Modifier> {
    if parsing.matching("pub") {
        return Some(Modifier::Public);
    } else if parsing.matching("pub(proj)") {
        return Some(Modifier::Protected);
    } else if parsing.matching("extern") {
        return Some(Modifier::Extern);
    } else if parsing.matching("internal") {
        return Some(Modifier::Internal);
    } else if parsing.matching("operation") {
        return Some(Modifier::Operation);
    }
    return None;
}

fn parse_attributes(parsing: &mut ParseInfo, global: bool) -> HashMap<String, Attribute> {
    let mut output = HashMap::new();
    while parsing.matching("#") {
        if global {
            todo!()
        } else {
            if !parsing.matching("[") {
                parsing.create_error("Expected attribute!".to_string());
                continue;
            }
            let name = parse_ident(parsing);
            match parsing.next_included() {
                Some(value) => match value {
                    b'(' => {
                        match parsing.parse_to(b')') {
                            Some(value) =>
                                if !parsing.matching("]") {
                                    parsing.create_error("Expected closing brace!".to_string());
                                } else {
                                    output.insert(name, Attribute::new(value));
                                },
                            None => parsing.create_error("Unexpected EOF".to_string())
                        }
                    }
                    b']' => {}
                    _val => {
                        parsing.create_error("Expected value or end of attribute".to_string());
                    }
                }
                None => parsing.create_error("Unexpected EOF".to_string())
            }
        }
    }
    return output;
}