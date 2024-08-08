#![feature(get_mut_unchecked, async_closure)]

extern crate core;

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use async_recursion::async_recursion;
use data::tokens::Span;
use indexmap::IndexMap;

use crate::degeneric::degeneric_type_no_generic_types;
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::errors::ParsingError;
use syntax::program::code::FinalizedEffectType;
use syntax::program::syntax::Syntax;
use syntax::program::types::FinalizedTypes;
use syntax::SimpleVariableManager;

use crate::output::TypesChecker;

/// Checks code to perform internal linking and find any errors
pub mod check_code;
/// Checks functions
pub mod check_function;
/// Checks the impl call effect
pub mod check_impl_call;
/// Checks the method call effect
pub mod check_method_call;
/// Checks the operator effect
pub mod check_operator;
/// Checks structs
pub mod check_struct;
/// Degenerics types
pub mod degeneric;
/// Used to send data to be checked by the checker and then send the result to the compiler
pub mod output;

/// Finalizes an IndexMap of generics into FinalizedEffectType
pub async fn finalize_generics(
    syntax: &Arc<Mutex<Syntax>>,
    resolver: &Box<dyn NameResolver>,
    generics: &IndexMap<String, Vec<UnparsedType>>,
) -> Result<IndexMap<String, FinalizedTypes>, ParsingError> {
    let mut output = IndexMap::default();
    for (generic, bounds) in generics {
        let mut output_bounds = vec![];
        for bound in bounds {
            output_bounds.push(
                Syntax::parse_type(syntax.clone(), resolver.boxed_clone(), bound.clone(), vec![])
                    .await?
                    .finalize(syntax.clone())
                    .await,
            );
        }
        output.insert(generic.clone(), FinalizedTypes::Generic(generic.clone(), output_bounds));
    }
    return Ok(output);
}

/// Simple wrapper program for the types used in code verification
pub struct CodeVerifier<'a> {
    process_manager: &'a TypesChecker,
    resolver: Box<dyn NameResolver>,
    return_type: Option<FinalizedTypes>,
    syntax: Arc<Mutex<Syntax>>,
}

/// Gets the return type of the effect, requiring a variable manager to get
/// any variables from, or None if the effect has no return type.
#[async_recursion(Sync)]
pub async fn get_return(
    types: &FinalizedEffectType,
    variables: &SimpleVariableManager,
    syntax: &Arc<Mutex<Syntax>>,
) -> Option<FinalizedTypes> {
    return match types {
        FinalizedEffectType::MethodCall(calling, function, _, returning) => match function.return_type.as_ref().cloned() {
            Some(mut inner) => {
                if !returning.is_empty() {
                    let generics = function
                        .generics
                        .iter()
                        .zip(returning)
                        //TODO bounds check
                        .map(|((generic, _bounds), explicit_generic)| (generic.clone(), explicit_generic.0.clone()))
                        .collect::<HashMap<_, _>>();
                    degeneric_type_no_generic_types(&mut inner, &generics, syntax).await;
                } else if let Some(calling) = calling {
                    let other = get_return(&calling.types, variables, syntax).await;
                    if let Some(found) = other {
                        let mut generics = HashMap::new();
                        function
                            .parent
                            .as_ref()
                            .unwrap()
                            .resolve_generic(&found, syntax, &mut generics, Span::default())
                            .await
                            .unwrap();
                        degeneric_type_no_generic_types(&mut inner, &generics, syntax).await;
                    }
                }
                Some(FinalizedTypes::Reference(Box::new(inner)))
            }
            None => None,
        },
        FinalizedEffectType::GenericMethodCall(function, _, args)
        | FinalizedEffectType::VirtualCall(_, function, _, args)
        | FinalizedEffectType::GenericVirtualCall(_, _, function, args) => match function.return_type.as_ref().cloned() {
            Some(mut inner) => {
                if let Some(calling) = args.get(0) {
                    let other = get_return(&calling.types, variables, syntax).await;
                    if let Some(found) = other {
                        let mut generics = HashMap::new();
                        function
                            .parent
                            .as_ref()
                            .unwrap()
                            .resolve_generic(&found, syntax, &mut generics, Span::default())
                            .await
                            .unwrap();
                        degeneric_type_no_generic_types(&mut inner, &generics, syntax).await;
                    }
                }
                Some(FinalizedTypes::Reference(Box::new(inner)))
            }
            None => None,
        },
        // Stores just return their inner type.
        FinalizedEffectType::HeapStore(inner)
        | FinalizedEffectType::StackStore(inner)
        | FinalizedEffectType::Set(_, inner) => get_return(&inner.types, variables, syntax).await,
        // References return their inner type as well.
        FinalizedEffectType::ReferenceLoad(inner) => match get_return(&inner.types, variables, syntax).await.unwrap() {
            FinalizedTypes::Reference(inner) => Some(*inner),
            _ => panic!("Tried to load non-reference!"),
        },
        // Gets the type of the field in the program with that name.
        FinalizedEffectType::Load(effect, name, _) => get_return(&effect.types, variables, syntax)
            .await
            .unwrap()
            .inner_struct()
            .fields
            .iter()
            .find(|field| &field.field.name == name)
            .map(|field| field.field.field_type.clone()),
        _ => types.get_nongeneric_return(variables),
    };
}
