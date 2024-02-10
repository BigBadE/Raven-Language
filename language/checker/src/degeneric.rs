use std::collections::HashMap;
use std::future::Future;
use std::mem;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::get_return;
use async_recursion::async_recursion;
use data::tokens::Span;
use data::ParsingError;
use syntax::async_util::AsyncDataGetter;
use syntax::code::{FinalizedEffectType, FinalizedEffects};
use syntax::function::{display_parenless, CodelessFinalizedFunction, FinalizedCodeBody, FunctionData};
use syntax::r#struct::{FinalizedStruct, StructData};
use syntax::syntax::Syntax;
use syntax::top_element_manager::ImplWaiter;
use syntax::types::FinalizedTypes;
use syntax::{ProcessManager, SimpleVariableManager};

/// Flattens a type, which is the final step before compilation that gets rid of all generics in the type
#[async_recursion]
// skipcq: RS-R1000 Match statements have complexity calculated incorrectly
pub async fn degeneric_effect(
    effect: &mut FinalizedEffectType,
    syntax: &Arc<Mutex<Syntax>>,
    process_manager: &dyn ProcessManager,
    variables: &mut SimpleVariableManager,
    span: &Span,
) -> Result<(), ParsingError> {
    match effect {
        FinalizedEffectType::CreateVariable(name, value, types) => {
            *types = get_return(&value.types, variables, syntax).await.unwrap();
            variables.variables.insert(name.clone(), types.clone());
            degeneric_effect(&mut value.types, syntax, process_manager, variables, span).await?;
            degeneric_type(types, process_manager.generics(), syntax).await;
        }
        FinalizedEffectType::CompareJump(effect, _, _) => {
            degeneric_effect(&mut effect.types, syntax, process_manager, variables, span).await?
        }
        FinalizedEffectType::CodeBody(body) => degeneric_code_body(body, process_manager, variables, syntax).await?,
        FinalizedEffectType::MethodCall(calling, function, arguments, _return_type) => {
            if let Some(found) = calling {
                degeneric_effect(&mut found.types, syntax, process_manager, variables, span).await?;
            }

            *function =
                degeneric_function(function.clone(), process_manager.cloned(), arguments, syntax, variables, None).await?;

            for argument in arguments {
                degeneric_effect(&mut argument.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::GenericMethodCall(function, types, arguments) => {
            let mut calling = arguments.remove(0);
            degeneric_effect(&mut calling.types, syntax, process_manager, variables, span).await?;

            let implementor = get_return(&calling.types, variables, syntax).await.unwrap();
            let implementation = ImplWaiter {
                syntax: syntax.clone(),
                base_type: implementor.clone(),
                trait_type: types.clone(),
                error: ParsingError::new(
                    Span::default(),
                    "You shouldn't see this! Report this please! Location: Degeneric generic method call",
                ),
            }
            .await?;

            let name = function.data.name.split("::").last().unwrap();
            let function = implementation.iter().find(|inner| inner.name.ends_with(&name)).unwrap();

            for argument in &mut *arguments {
                degeneric_effect(&mut argument.types, syntax, process_manager, variables, span).await?;
            }
            arguments.insert(0, calling.clone());
            let function = AsyncDataGetter::new(syntax.clone(), function.clone()).await;
            let function =
                degeneric_function(function.clone(), process_manager.cloned(), &arguments, syntax, variables, None).await?;
            *effect = FinalizedEffectType::MethodCall(None, function, arguments.clone(), None);
        }
        FinalizedEffectType::Set(base, value) => {
            degeneric_effect(&mut base.types, syntax, process_manager, variables, span).await?;
            degeneric_effect(&mut value.types, syntax, process_manager, variables, span).await?;
        }
        FinalizedEffectType::Load(base, _, types) => {
            degeneric_effect(&mut base.types, syntax, process_manager, variables, span).await?;
            degeneric_type(types, process_manager.generics(), syntax).await;
        }
        FinalizedEffectType::CreateStruct(storing, types, effects) => {
            if let Some(found) = storing {
                degeneric_effect(&mut found.types, syntax, process_manager, variables, span).await?;
            }
            let fields = types.get_fields();
            let mut type_generics = process_manager.generics().clone();
            for i in 0..fields.len() {
                let found = &mut effects[i].1;
                found
                    .types
                    .get_nongeneric_return(variables)
                    .unwrap()
                    .resolve_generic(
                        &fields[i].field.field_type,
                        syntax,
                        &mut type_generics,
                        span.make_error("Type doesn't match struct field bounds!"),
                    )
                    .await?;
                degeneric_effect(&mut found.types, syntax, process_manager, variables, span).await?;
            }
            degeneric_type(types, &type_generics, syntax).await;
        }
        FinalizedEffectType::CreateArray(types, effects) => {
            if let Some(found) = types {
                degeneric_type(found, process_manager.generics(), syntax).await;
            }
            for effect in effects {
                degeneric_effect(&mut effect.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::VirtualCall(_, function, effects) => {
            *function =
                degeneric_function(function.clone(), process_manager.cloned(), effects, syntax, variables, None).await?;
            for effect in effects {
                degeneric_effect(&mut effect.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::GenericVirtualCall(index, target, found, effects) => {
            syntax.lock().unwrap().process_manager.handle().lock().unwrap().spawn(
                target.name.clone(),
                degeneric_header(
                    target.clone(),
                    found.data.clone(),
                    syntax.clone(),
                    process_manager.cloned(),
                    effects.clone(),
                    variables.clone(),
                    span.clone(),
                ),
            );

            let output = AsyncDataGetter::new(syntax.clone(), target.clone()).await;
            let mut temp = vec![];
            mem::swap(&mut temp, effects);
            let output = degeneric_function(output, process_manager.cloned(), &temp, &syntax, variables, None).await?;
            for effect in &mut *effects {
                degeneric_effect(&mut effect.types, syntax, process_manager, variables, span).await?;
            }
            *effect = FinalizedEffectType::VirtualCall(*index, output, temp);
        }
        FinalizedEffectType::Downcast(base, target, functions) => {
            let impl_functions = ImplWaiter {
                syntax: syntax.clone(),
                trait_type: target.clone(),
                base_type: get_return(&base.types, variables, syntax).await.unwrap(),
                error: ParsingError::new(
                    Span::default(),
                    "You shouldn't see this! Report this please! Location: Return type check",
                ),
            }
            .await?;
            degeneric_effect(&mut base.types, syntax, process_manager, variables, span).await?;
            degeneric_type(target, process_manager.generics(), syntax).await;

            for function in impl_functions {
                let function = AsyncDataGetter::new(syntax.clone(), function).await;
                functions
                    .push(degeneric_function(function, process_manager.cloned(), &vec![], syntax, variables, None).await?)
            }
        }
        FinalizedEffectType::HeapStore(storing) => {
            degeneric_effect(&mut storing.types, syntax, process_manager, variables, span).await?
        }
        FinalizedEffectType::HeapAllocate(types) => degeneric_type(types, process_manager.generics(), syntax).await,
        FinalizedEffectType::ReferenceLoad(base) => {
            degeneric_effect(&mut base.types, syntax, process_manager, variables, span).await?
        }
        FinalizedEffectType::StackStore(storing) => {
            degeneric_effect(&mut storing.types, syntax, process_manager, variables, span).await?
        }
        _ => {}
    }
    return Ok(());
}

/// Makes a copy of the CodelessFinalizedFunction with all the generics solidified into their actual type.
/// Figures out the solidified types by comparing generics against the input effect types,
/// then replaces all generic types with their solidified types.
/// This can't always figure out return types, so an optional return type variable is passed as well
/// for function calls that include them (see EffectType::MethodCall)
/// The VariableManager here is for the arguments to the function, and not for the function itself.
pub async fn degeneric_function(
    method: Arc<CodelessFinalizedFunction>,
    mut manager: Box<dyn ProcessManager>,
    arguments: &Vec<FinalizedEffects>,
    syntax: &Arc<Mutex<Syntax>>,
    variables: &SimpleVariableManager,
    returning: Option<(FinalizedTypes, Span)>,
) -> Result<Arc<CodelessFinalizedFunction>, ParsingError> {
    *manager.mut_generics() = method
        .generics
        .clone()
        .into_iter()
        .map(|(name, types)| (name.clone(), FinalizedTypes::Generic(name, types)))
        .collect::<HashMap<_, _>>();

    // Degenerics the return type if there is one and returning is some.
    if let Some(inner) = method.return_type.clone() {
        if let Some((returning, span)) = returning {
            inner.resolve_generic(&returning, syntax, manager.mut_generics(), span.make_error("Invalid bounds!")).await?;
        }
    }

    //Degenerics the arguments to the method
    for i in 0..method.arguments.len() {
        if method.arguments.len() != 0 && arguments.len() == 0 {
            break;
        }
        let effect = get_return(&arguments[i].types, variables, syntax).await.unwrap();

        match method.arguments[i]
            .field
            .field_type
            .resolve_generic(&effect, syntax, manager.mut_generics(), arguments[i].span.make_error("Invalid bounds!"))
            .await
        {
            Ok(_) => {}
            Err(error) => {
                return Err(error);
            }
        }
    }

    // Now all the generic types have been resolved, it's time to replace them with
    // their solidified versions.
    // Degenericed function names have a $ separating the name and the generics.
    let name = if manager.generics().is_empty() {
        method.data.name.split("$").next().unwrap().to_string()
    } else {
        format!(
            "{}${}",
            method.data.name.split("$").next().unwrap(),
            display_parenless(&manager.generics().values().collect(), "_")
        )
    };

    // If this function has already been degenericed, use the previous one.
    if syntax.lock().unwrap().compiling.contains_key(&name) {
        let data = syntax.lock().unwrap().functions.types.get(&name).unwrap().clone();
        return Ok(AsyncDataGetter::new(syntax.clone(), data).await);
    }

    // Copy the method and degeneric every type inside of it.
    let mut new_method = CodelessFinalizedFunction::clone(&method);
    // Delete the generics because now they are all solidified.
    new_method.generics.clear();
    let mut method_data = FunctionData::clone(&method.data);
    method_data.name.clone_from(&name);
    new_method.data = Arc::new(method_data);
    // Degeneric the arguments.
    for argument in &mut new_method.arguments {
        degeneric_type(&mut argument.field.field_type, &manager.generics(), syntax).await;
    }

    // Degeneric the return type if there is one.
    if let Some(method_returning) = &mut new_method.return_type {
        degeneric_type(method_returning, &manager.generics(), syntax).await;
    }

    // Add the new degenericed static data to the locked function.
    let original = method;
    let new_method = Arc::new(new_method);
    let mut locked = syntax.lock().unwrap();
    // Since Syntax can't be locked this whole time, sometimes someone else can beat this method to the punch.
    // It's super rare to happen, but if it does just give up
    // TODO figure out of this is required
    /*if syntax.lock().unwrap().functions.types.contains_key(&name) {
        return Ok(new_method);
    }*/
    locked.functions.add_type(new_method.data.clone());
    locked.functions.add_data(new_method.data.clone(), new_method.clone());

    // Spawn a thread to asynchronously degeneric the code inside the function.
    let handle = manager.handle().clone();
    handle
        .lock()
        .unwrap()
        .spawn(new_method.data.name.clone(), degeneric_code(syntax.clone(), original, new_method.clone(), manager));

    return Ok(new_method);
}

/// Degenerics the code body of the method.
async fn degeneric_code(
    syntax: Arc<Mutex<Syntax>>,
    original: Arc<CodelessFinalizedFunction>,
    degenericed_method: Arc<CodelessFinalizedFunction>,
    manager: Box<dyn ProcessManager>,
) {
    // This has to wait until the original is ready to be compiled.
    FunctionWaiter { syntax: syntax.clone(), data: original.data.clone() }.await;

    // Gets a clone of the code of the original.
    let mut code = syntax.lock().unwrap().generics.get(&original.data.name).unwrap().code.clone();

    let mut variables = SimpleVariableManager::for_function(degenericed_method.deref());

    // Degenerics the code body.
    match degeneric_code_body(&mut code, &*manager, &mut variables, &syntax).await {
        Ok(inner) => inner,
        Err(error) => panic!("Error degenericing code: {} for {}", error.message, degenericed_method.data.name),
    };

    // Combines the degenericed function with the degenericed code to finalize it.
    let output = CodelessFinalizedFunction::clone(degenericed_method.deref()).add_code(code);

    let handle = manager.handle().clone();

    // Sends the finalized function to be compiled.
    Syntax::add_compiling(manager, Arc::new(output), &syntax, false).await;

    handle.lock().unwrap().finish_task(&degenericed_method.data.name);
}

/// A waiter used by generics trying to degeneric a function that returns when the target function's
/// code is in the compiling list
struct FunctionWaiter {
    /// The program
    syntax: Arc<Mutex<Syntax>>,
    /// Name of the function to wait for
    data: Arc<FunctionData>,
}

impl Future for FunctionWaiter {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        return if self.syntax.lock().unwrap().generics.contains_key(&self.data.name) {
            Poll::Ready(())
        } else {
            self.syntax
                .lock()
                .unwrap()
                .compiling_wakers
                .entry(self.data.name.clone())
                .or_insert(vec![])
                .push(cx.waker().clone());
            Poll::Pending
        };
    }
}

/// Degenerics every effect inside the body of code.
pub async fn degeneric_code_body(
    code: &mut FinalizedCodeBody,
    process_manager: &dyn ProcessManager,
    variables: &mut SimpleVariableManager,
    syntax: &Arc<Mutex<Syntax>>,
) -> Result<(), ParsingError> {
    for expression in &mut code.expressions {
        degeneric_effect(&mut expression.effect.types, syntax, process_manager, variables, &expression.effect.span).await?;
    }

    return Ok(());
}

/// Degenerics the type by replacing all generics with their solidified value.
#[async_recursion]
pub async fn degeneric_type(
    types: &mut FinalizedTypes,
    generics: &HashMap<String, FinalizedTypes>,
    syntax: &Arc<Mutex<Syntax>>,
) {
    return match types {
        FinalizedTypes::Generic(name, _) => {
            if let Some(found) = generics.get(name) {
                types.clone_from(found);
            }
        }
        FinalizedTypes::GenericType(base, bounds) => {
            degeneric_type(base, generics, syntax).await;

            for bound in &mut *bounds {
                degeneric_type(bound, generics, syntax).await;
            }

            let base = base.inner_struct();
            if bounds.is_empty() {
                *types = FinalizedTypes::Struct(base.clone());
                // If there are no bounds, we're good.
                return;
            }
            let name = format!("{}<{}>", base.data.name, display_parenless(&bounds, ", "));
            // If this type has already been flattened with these args, return that.
            if syntax.lock().unwrap().structures.types.contains_key(&name) {
                let data;
                {
                    let locked = syntax.lock().unwrap();
                    // skipcq: RS-W1070 Initialization of a value can't use clone_from
                    data = locked.structures.types.get(&name).unwrap().clone();
                }
                let base = AsyncDataGetter::new(syntax.clone(), data).await;
                *types = FinalizedTypes::Struct(base.clone());
            } else {
                // Clone the type and add the new type to the structures.
                let mut other = StructData::clone(&base.data);
                other.name.clone_from(&name);

                // Update the program's functions
                for function in &mut other.functions {
                    let mut temp = FunctionData::clone(function);
                    temp.name = format!("{}::{}", name, temp.name.split("::").last().unwrap());
                    let temp = Arc::new(temp);
                    *function = temp;
                }

                let arc_other;
                {
                    let mut locked = syntax.lock().unwrap();
                    locked.structures.set_id(&mut other);
                    arc_other = Arc::new(other);
                }

                // Get the FinalizedStruct and degeneric it.
                let mut data = FinalizedStruct::clone(AsyncDataGetter::new(syntax.clone(), base.data.clone()).await.deref());
                data.data.clone_from(&arc_other);

                // Update the program's fields
                for field in &mut data.fields {
                    degeneric_type(&mut field.field.field_type, generics, syntax).await;
                }

                let data = Arc::new(data);
                // Add the flattened type to the syntax
                let mut locked = syntax.lock().unwrap();
                locked.structures.add_data(arc_other, data.clone());
                *types = FinalizedTypes::Struct(data.clone());
            }
        }
        FinalizedTypes::Reference(inner) => degeneric_type(inner, generics, syntax).await,
        FinalizedTypes::Array(inner) => degeneric_type(inner, generics, syntax).await,
        FinalizedTypes::Struct(inner) => {
            let mut temp = FinalizedStruct::clone(inner);
            for field in &mut temp.fields {
                degeneric_type(&mut field.field.field_type, generics, syntax).await;
            }
            *inner = degeneric_struct(temp, generics, syntax).await;
        }
    };
}

/// Degenerics the type by replacing all generics with their solidified value.
/// Ignores generic types
#[async_recursion]
pub async fn degeneric_type_no_generic_types(
    types: &mut FinalizedTypes,
    generics: &HashMap<String, FinalizedTypes>,
    syntax: &Arc<Mutex<Syntax>>,
) {
    return match types {
        FinalizedTypes::Generic(name, _) => {
            if let Some(found) = generics.get(name) {
                types.clone_from(found);
            }
        }
        FinalizedTypes::GenericType(base, bounds) => {
            degeneric_type_no_generic_types(base, generics, syntax).await;

            for bound in &mut *bounds {
                degeneric_type_no_generic_types(bound, generics, syntax).await;
            }
        }
        FinalizedTypes::Reference(inner) => degeneric_type_no_generic_types(inner, generics, syntax).await,
        FinalizedTypes::Array(inner) => degeneric_type_no_generic_types(inner, generics, syntax).await,
        FinalizedTypes::Struct(inner) => {
            let mut temp = FinalizedStruct::clone(inner);
            for field in &mut temp.fields {
                degeneric_type_no_generic_types(&mut field.field.field_type, generics, syntax).await;
            }
            *inner = degeneric_struct(temp, generics, syntax).await;
        }
    };
}

/// Degenerics the type's fields
#[async_recursion]
pub async fn degeneric_type_fields(
    types: &mut FinalizedTypes,
    generics: &HashMap<String, FinalizedTypes>,
    syntax: &Arc<Mutex<Syntax>>,
) {
    return match types {
        FinalizedTypes::Generic(name, _) => {
            if let Some(found) = generics.get(name) {
                types.clone_from(found);
            }
        }
        FinalizedTypes::GenericType(base, bounds) => {
            let mut i = 0;
            let mut base_generics = HashMap::new();
            for (generic, _bound) in &base.inner_struct().generics {
                base_generics.insert(generic.clone(), bounds[i].clone());
                i += 1;
            }

            degeneric_type_fields(base, &base_generics, syntax).await;
            degeneric_type_fields(base, generics, syntax).await;

            for bound in &mut *bounds {
                degeneric_type_fields(bound, generics, syntax).await;
            }
        }
        FinalizedTypes::Reference(inner) => degeneric_type_fields(inner, generics, syntax).await,
        FinalizedTypes::Array(inner) => degeneric_type_fields(inner, generics, syntax).await,
        FinalizedTypes::Struct(inner) => {
            let mut temp = FinalizedStruct::clone(inner);
            for field in &mut temp.fields {
                degeneric_type_fields(&mut field.field.field_type, generics, syntax).await;
            }
            *inner = degeneric_struct(temp, generics, syntax).await;
        }
    };
}

/// Degenerics a function header, for virtual function calls
pub async fn degeneric_header(
    degenericed: Arc<FunctionData>,
    base: Arc<FunctionData>,
    syntax: Arc<Mutex<Syntax>>,
    mut manager: Box<dyn ProcessManager>,
    arguments: Vec<FinalizedEffects>,
    variables: SimpleVariableManager,
    span: Span,
) -> Result<(), ParsingError> {
    let function: Arc<CodelessFinalizedFunction> = AsyncDataGetter { getting: base, syntax: syntax.clone() }.await;

    let return_type = arguments[0].types.get_nongeneric_return(&variables).unwrap();
    let (_, generics) = return_type.inner_generic_type().unwrap();
    assert_eq!(function.generics.len(), generics.len());

    let mut iterator = function.generics.iter();
    for generic in generics {
        let (name, bounds) = iterator.next().unwrap();
        for bound in bounds {
            if !generic.of_type(bound, syntax.clone()).await {
                return Err(span.make_error("Failed bounds sanity check!"));
            }
        }
        manager.mut_generics().insert(name.clone(), generic.clone());
    }

    // Copy the method and degeneric every type inside of it.
    let mut new_method = CodelessFinalizedFunction::clone(&function);
    // Delete the generics because now they are all solidified.
    new_method.generics.clear();
    new_method.data = degenericed;

    // Degeneric the arguments.
    for arguments in &mut new_method.arguments {
        degeneric_type(&mut arguments.field.field_type, &manager.generics(), &syntax).await;
    }

    // Degeneric the return type if there is one.
    if let Some(returning) = &mut new_method.return_type {
        degeneric_type(returning, &manager.generics(), &syntax).await;
    }

    let new_method = Arc::new(new_method);

    //let mut code =
    //    CodelessFinalizedFunction::clone(&new_method).add_code(FinalizedCodeBody::new(vec![], "empty".to_string(), true));

    let mut locked = syntax.lock().unwrap();
    locked.functions.add_type(new_method.data.clone());
    locked.functions.add_data(new_method.data.clone(), new_method.clone());

    // Give the compiler the empty body
    return Ok(());
}

/// Degenerics a finalized struct
pub async fn degeneric_struct(
    mut structure: FinalizedStruct,
    generics: &HashMap<String, FinalizedTypes>,
    syntax: &Arc<Mutex<Syntax>>,
) -> Arc<FinalizedStruct> {
    let targets: Vec<_> =
        generics.iter().filter(|(key, _)| structure.generics.contains_key(*key)).map(|(_, value)| value).collect();
    if targets.is_empty() {
        return Arc::new(structure);
    }
    let mut data = StructData::clone(&structure.data);
    let name = format!("{}${}", data.name, display_parenless(&targets, "_"));
    data.name.clone_from(&name);

    // TODO check if this is safe, handle generics calling generics
    structure.generics.clear();
    for field in &mut structure.fields {
        degeneric_type(&mut field.field.field_type, generics, syntax).await;
    }

    let mut locked = syntax.lock().unwrap();
    locked.structures.set_id(&mut data);
    structure.data = Arc::new(data);
    let output = Arc::new(structure);

    locked.structures.add_type(output.data.clone());
    locked.structures.add_data(output.data.clone(), output.clone());
    return output;
}
