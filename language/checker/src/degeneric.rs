use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use async_recursion::async_recursion;
use data::tokens::Span;
use data::ParsingError;
use syntax::async_util::AsyncDataGetter;
use syntax::code::{FinalizedEffectType, FinalizedEffects};
use syntax::function::{display_parenless, CodelessFinalizedFunction, FinalizedCodeBody, FunctionData};
use syntax::r#struct::{FinalizedStruct, StructData};
use syntax::syntax::Syntax;
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
            variables.variables.insert(name.clone(), types.clone());
            degeneric_effect(&mut value.types, syntax, process_manager, variables, span).await?;
            degeneric_type(types, process_manager.generics(), syntax).await?;
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
            *function = function.flatten(syntax).await?;
            for argument in arguments {
                degeneric_effect(&mut argument.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::GenericMethodCall(function, types, arguments) => {
            types.flatten(syntax).await?;
            *function = function.flatten(syntax).await?;
            *function = CodelessFinalizedFunction::degeneric(
                function.clone(),
                process_manager.cloned(),
                arguments,
                syntax,
                variables,
                None,
            )
            .await?;
            for argument in arguments {
                degeneric_effect(&mut argument.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::Set(base, value) => {
            degeneric_effect(&mut base.types, syntax, process_manager, variables, span).await?;
            degeneric_effect(&mut value.types, syntax, process_manager, variables, span).await?;
        }
        FinalizedEffectType::Load(base, _, types) => {
            degeneric_effect(&mut base.types, syntax, process_manager, variables, span).await?;
            *types = degeneric_struct(FinalizedStruct::clone(types), process_manager.generics(), syntax).await;
        }
        FinalizedEffectType::CreateStruct(storing, types, effects) => {
            if let Some(found) = storing {
                degeneric_effect(&mut found.types, syntax, process_manager, variables, span).await?;
            }
            types.flatten(syntax).await?;
            for (_, found) in effects {
                degeneric_effect(&mut found.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::CreateArray(types, effects) => {
            if let Some(found) = types {
                found.flatten(syntax).await?;
            }
            for effect in effects {
                degeneric_effect(&mut effect.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::VirtualCall(_, function, effects) => {
            *function = CodelessFinalizedFunction::degeneric(
                function.clone(),
                process_manager.cloned(),
                effects,
                syntax,
                variables,
                None,
            )
            .await?;
            *function = function.flatten(syntax).await?;
            for effect in effects {
                degeneric_effect(&mut effect.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::GenericVirtualCall(_, _, _, effects) => {
            for effect in effects {
                degeneric_effect(&mut effect.types, syntax, process_manager, variables, span).await?;
            }
        }
        FinalizedEffectType::Downcast(base, target, _) => {
            degeneric_effect(&mut base.types, syntax, process_manager, variables, span).await?;
            target.flatten(syntax).await?;
        }
        FinalizedEffectType::HeapStore(storing) => {
            degeneric_effect(&mut storing.types, syntax, process_manager, variables, span).await?
        }
        FinalizedEffectType::HeapAllocate(types) => types.flatten(syntax).await?,
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
        let mut effect = arguments[i].types.get_return(variables, syntax).await.unwrap();

        effect.fix_generics(&*manager, syntax).await?;
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

    if manager.generics().is_empty() {
        return Ok(method);
    }

    // Now all the generic types have been resolved, it's time to replace them with
    // their solidified versions.
    // Degenericed function names have a $ seperating the name and the generics.
    let name = format!(
        "{}${}",
        method.data.name.split("$").next().unwrap(),
        display_parenless(&manager.generics().values().collect(), "_")
    );
    // If this function has already been degenericed, use the previous one.
    if syntax.lock().unwrap().functions.types.contains_key(&name) {
        let data = syntax.lock().unwrap().functions.types.get(&name).unwrap().clone();
        return Ok(AsyncDataGetter::new(syntax.clone(), data).await);
    } else {
        // Copy the method and degeneric every type inside of it.
        let mut new_method = CodelessFinalizedFunction::clone(&method);
        // Delete the generics because now they are all solidified.
        new_method.generics.clear();
        let mut method_data = FunctionData::clone(&method.data);
        method_data.name.clone_from(&name);
        new_method.data = Arc::new(method_data);
        // Degeneric the arguments.
        for argument in &mut new_method.arguments {
            argument.field.field_type.degeneric(&manager.generics(), syntax).await;
        }

        // Degeneric the return type if there is one.
        if let Some(method_returning) = &mut new_method.return_type {
            method_returning.degeneric(&manager.generics(), syntax).await;
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
    };
}

/// Degenerics the code body of the method.
async fn degeneric_code(
    syntax: Arc<Mutex<Syntax>>,
    original: Arc<CodelessFinalizedFunction>,
    degenericed_method: Arc<CodelessFinalizedFunction>,
    manager: Box<dyn ProcessManager>,
) {
    // This has to wait until the original is ready to be compiled.
    GenericWaiter { syntax: syntax.clone(), data: original.data.clone() }.await;

    // Gets a clone of the code of the original.
    let code = syntax.lock().unwrap().compiling.get(&original.data.name).unwrap().code.clone();

    let mut variables = SimpleVariableManager::for_function(degenericed_method.deref());
    // Degenerics the code body.
    let code = match code.degeneric(&*manager, &mut variables, &syntax).await {
        Ok(inner) => inner,
        Err(error) => panic!("Error degenericing code: {}", error.message),
    };

    // Combines the degenericed function with the degenericed code to finalize it.
    let mut output = CodelessFinalizedFunction::clone(degenericed_method.deref()).add_code(code);
    output.flatten(&syntax, &*manager).await.unwrap();

    // Sends the finalized function to be compiled.
    let mut locked = syntax.lock().unwrap();
    locked.add_compiling(Arc::new(output));
}

/// A waiter used by generics trying to degeneric a function that returns when the target function's
/// code is in the compiling list
struct GenericWaiter {
    /// The program
    syntax: Arc<Mutex<Syntax>>,
    /// Name of the function to wait for
    data: Arc<FunctionData>,
}

impl Future for GenericWaiter {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        return if self.syntax.lock().unwrap().compiling.contains_key(&self.data.name) {
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
        expression.effect.types.degeneric(process_manager, variables, syntax, &expression.effect.span).await?;
        println!("Degeneric'd {:?}", expression.effect);
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
            base.degeneric(generics, syntax).await;

            for bound in &mut *bounds {
                bound.degeneric(generics, syntax).await;
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
                    field.field.field_type.flatten(syntax).await?;
                }

                let data = Arc::new(data);
                // Add the flattened type to the syntax
                let mut locked = syntax.lock().unwrap();
                locked.structures.add_data(arc_other, data.clone());
                *types = FinalizedTypes::Struct(data.clone());
            }
        }
        FinalizedTypes::Reference(inner) => inner.degeneric(generics, syntax).await,
        FinalizedTypes::Array(inner) => inner.degeneric(generics, syntax).await,
        FinalizedTypes::Struct(inner) => {
            let mut temp = FinalizedStruct::clone(inner);
            for field in &mut temp.fields {
                degeneric_type(&mut field.field.field_type, generics, syntax).await?;
            }
            *inner = temp.degeneric(generics, syntax).await;
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

    let return_type = arguments[0].types.get_return(&variables, &syntax).await.unwrap();
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
        arguments.field.field_type.degeneric(&manager.generics(), &syntax).await;
    }

    // Degeneric the return type if there is one.
    if let Some(returning) = &mut new_method.return_type {
        returning.degeneric(&manager.generics(), &syntax).await;
    }

    let new_method = Arc::new(new_method);

    let mut code =
        CodelessFinalizedFunction::clone(&new_method).add_code(FinalizedCodeBody::new(vec![], "empty".to_string(), true));
    code.flatten(&syntax, &*manager).await?;

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
        field.field.field_type.degeneric(generics, syntax).await;
    }

    let mut locked = syntax.lock().unwrap();
    locked.structures.set_id(&mut data);
    structure.data = Arc::new(data);
    let output = Arc::new(structure);

    locked.structures.add_type(output.data.clone());
    locked.structures.add_data(output.data.clone(), output.clone());
    return output;
}
