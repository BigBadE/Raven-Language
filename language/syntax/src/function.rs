use std::fmt::{Debug, Display};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};

use async_trait::async_trait;
use indexmap::IndexMap;

use crate::async_util::{AsyncDataGetter, HandleWrapper, NameResolver};
use crate::code::{
    Expression, FinalizedEffects, FinalizedExpression, FinalizedMemberField, MemberField,
};
use crate::types::FinalizedTypes;
use crate::{
    is_modifier, Attribute, DataType, Modifier, ParsingError, ParsingFuture, ProcessManager,
    SimpleVariableManager, Syntax, TopElement, TopElementManager, Types,
};

/// The static data of a function, which is set during parsing and immutable throughout the entire compilation process.
/// Generics will copy this and change the name and types, but never modify the original.
#[derive(Clone, Debug)]
pub struct FunctionData {
    pub attributes: Vec<Attribute>,
    pub modifiers: u8,
    pub name: String,
    pub poisoned: Vec<ParsingError>,
}

impl FunctionData {
    pub fn new(attributes: Vec<Attribute>, modifiers: u8, name: String) -> Self {
        return Self {
            attributes,
            modifiers,
            name,
            poisoned: Vec::default(),
        };
    }

    /// Creates an empty function data that errored while parsing.
    pub fn poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: Vec::default(),
            modifiers: 0,
            name,
            poisoned: vec![error],
        };
    }
}

/// Allows generic access to FunctionData.
#[async_trait]
impl TopElement for FunctionData {
    type Unfinalized = UnfinalizedFunction;
    type Finalized = CodelessFinalizedFunction;

    fn set_id(&mut self, _id: u64) {
        //Ignored. Funcs don't have IDs
    }

    fn poison(&mut self, error: ParsingError) {
        self.poisoned.push(error);
    }

    fn is_operator(&self) -> bool {
        return false;
    }

    fn is_trait(&self) -> bool {
        return is_modifier(self.modifiers, Modifier::Trait);
    }

    fn errors(&self) -> &Vec<ParsingError> {
        return &self.poisoned;
    }

    fn name(&self) -> &String {
        return &self.name;
    }

    fn new_poisoned(name: String, error: ParsingError) -> Self {
        return FunctionData::poisoned(name, error);
    }

    /// Verifies the function and adds it to the compiler after it finished verifying.
    async fn verify(
        handle: Arc<Mutex<HandleWrapper>>,
        current: UnfinalizedFunction,
        syntax: Arc<Mutex<Syntax>>,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    ) {
        let name = current.data.name.clone();
        // Get the codeless finalized function and the code from the function.
        let (codeless_function, code) = process_manager.verify_func(current, &syntax).await;
        // Finalize the code and combine it with the codeless finalized function.
        let finalized_function = process_manager
            .verify_code(codeless_function, code, resolver, &syntax)
            .await;
        let finalized_function = Arc::new(finalized_function);
        let mut locked = syntax.lock().unwrap();

        // Add the finalized code to the compiling list.
        locked
            .compiling
            .insert(name.clone(), finalized_function.clone());
        for waker in &locked.compiling_wakers {
            waker.wake_by_ref();
        }
        locked.compiling_wakers.clear();

        if finalized_function.data.name == locked.async_manager.target {
            if let Some(found) = locked.async_manager.target_waker.as_ref() {
                found.wake_by_ref();
            }
        }
        handle.lock().unwrap().finish_task(&name);
    }

    fn get_manager(syntax: &mut Syntax) -> &mut TopElementManager<Self> {
        return &mut syntax.functions;
    }
}

/// An unfinalized function is the unlinked function directly after parsing, with no code.
/// Code is finalizied separately and combined with this to make a FinalizedFunction.
pub struct UnfinalizedFunction {
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    pub fields: Vec<ParsingFuture<MemberField>>,
    pub code: CodeBody,
    pub return_type: Option<ParsingFuture<Types>>,
    pub data: Arc<FunctionData>,
}

/// Gives generic access to the function data.
impl DataType<FunctionData> for UnfinalizedFunction {
    fn data(&self) -> &Arc<FunctionData> {
        return &self.data;
    }
}

/// If the code is required to finalize the function, then recursive function calls will deadlock.
/// That's why this codeless variant exists, which allows the function data to be finalized before the code itself.
/// This is combined with the FinalizedCodeBody into a FinalizedFunction which is passed to the compiler.
/// (see add_code below)
#[derive(Clone, Debug)]
pub struct CodelessFinalizedFunction {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub arguments: Vec<FinalizedMemberField>,
    pub return_type: Option<FinalizedTypes>,
    pub data: Arc<FunctionData>,
}

impl CodelessFinalizedFunction {
    /// Combines the CodelessFinalizedFunction with a FinalizedCodeBody to get a FinalizedFunction.
    pub fn add_code(self, code: FinalizedCodeBody) -> FinalizedFunction {
        return FinalizedFunction {
            generics: self.generics,
            fields: self.arguments,
            code,
            return_type: self.return_type,
            data: self.data,
        };
    }

    /// Makes a copy of the CodelessFinalizedFunction with all the generics solidified into their actual type.
    /// Figures out the solidified types by comparing generics against the input effect types,
    /// then replaces all generic types with their solidified types.
    /// This can't always figure out return types, so an optional return type variable is passed as well
    /// for function calls that include them (see Effects::MethodCall)
    /// The VariableManager here is for the arguments to the function, and not for the function itself.
    pub async fn degeneric(
        method: Arc<CodelessFinalizedFunction>,
        mut manager: Box<dyn ProcessManager>,
        arguments: &Vec<FinalizedEffects>,
        syntax: &Arc<Mutex<Syntax>>,
        variables: &SimpleVariableManager,
        resolver: &dyn NameResolver,
        returning: Option<FinalizedTypes>,
    ) -> Result<Arc<CodelessFinalizedFunction>, ParsingError> {
        // Degenerics the return type if there is one and returning is some.
        if let Some(inner) = method.return_type.clone() {
            if let Some(returning) = returning {
                inner
                    .resolve_generic(
                        &returning,
                        syntax,
                        manager.mut_generics(),
                        placeholder_error("Invalid bounds!".to_string()),
                    )
                    .await?;
            }
        }

        //Degenerics the arguments to the method
        for i in 0..method.arguments.len() {
            let mut effect = arguments[i].get_return(variables).unwrap();
            effect.fix_generics(resolver, syntax).await?;
            match method.arguments[i]
                .field
                .field_type
                .resolve_generic(
                    &effect,
                    syntax,
                    manager.mut_generics(),
                    placeholder_error(format!("Invalid bounds! {:?}", arguments[i])),
                )
                .await
            {
                Ok(_) => {}
                Err(error) => {
                    println!("error: {}", error);
                    return Err(error);
                }
            }
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
            let data = syntax
                .lock()
                .unwrap()
                .functions
                .types
                .get(&name)
                .unwrap()
                .clone();
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
            for arguments in &mut new_method.arguments {
                arguments
                    .field
                    .field_type
                    .degeneric(
                        &manager.generics(),
                        syntax,
                        placeholder_error(format!("No generic in {}", name)),
                        placeholder_error("Invalid bounds!".to_string()),
                    )
                    .await?;
            }

            // Degeneric the return type if there is one.
            if let Some(returning) = &mut new_method.return_type {
                returning
                    .degeneric(
                        &manager.generics(),
                        syntax,
                        placeholder_error(format!("No generic in {}", name)),
                        placeholder_error("Invalid bounds!".to_string()),
                    )
                    .await?;
            }

            // Add the new degenericed static data to the locked function.
            let original = method;
            let new_method = Arc::new(new_method);
            let mut locked = syntax.lock().unwrap();
            locked.functions.types.insert(name, new_method.data.clone());
            locked
                .functions
                .data
                .insert(new_method.data.clone(), new_method.clone());

            if let Some(wakers) = locked.functions.wakers.get(&new_method.data.name) {
                for waker in wakers {
                    waker.wake_by_ref();
                }
            }
            locked.functions.wakers.remove(&new_method.data.name);

            // Spawn a thread to asynchronously degeneric the code inside the function.
            let handle = manager.handle().clone();
            handle.lock().unwrap().spawn(
                new_method.data.name.clone(),
                degeneric_code(
                    syntax.clone(),
                    original,
                    resolver.boxed_clone(),
                    new_method.clone(),
                    manager,
                ),
            );

            return Ok(new_method);
        };
    }
}

/// A placeholder error until the actual tokens are passed.
fn placeholder_error(error: String) -> ParsingError {
    return ParsingError::new(String::default(), (0, 0), 0, (0, 0), 0, error);
}

struct GenericWaiter {
    syntax: Arc<Mutex<Syntax>>,
    name: String,
}

impl Future for GenericWaiter {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        return if self
            .syntax
            .lock()
            .unwrap()
            .compiling
            .contains_key(&self.name)
        {
            Poll::Ready(())
        } else {
            self.syntax
                .lock()
                .unwrap()
                .compiling_wakers
                .push(cx.waker().clone());
            Poll::Pending
        };
    }
}

/// Degenerics the code body of the method.
async fn degeneric_code(
    syntax: Arc<Mutex<Syntax>>,
    original: Arc<CodelessFinalizedFunction>,
    resolver: Box<dyn NameResolver>,
    degenericed_method: Arc<CodelessFinalizedFunction>,
    manager: Box<dyn ProcessManager>,
) {
    // This has to wait until the original is ready to be compiled.
    GenericWaiter {
        syntax: syntax.clone(),
        name: original.data.name.clone(),
    }
    .await;

    // Gets a clone of the code of the original.
    let code = syntax
        .lock()
        .unwrap()
        .compiling
        .get(&original.data.name)
        .unwrap()
        .code
        .clone();

    let mut variables = SimpleVariableManager::for_function(degenericed_method.deref());
    // Degenerics the code body.
    let code = match code
        .degeneric(&*manager, &*resolver, &mut variables, &syntax)
        .await
    {
        Ok(inner) => inner,
        Err(error) => panic!("Error degenericing code: {}", error),
    };

    // Combines the degenericed function with the degenericed code to finalize it.
    let output = CodelessFinalizedFunction::clone(degenericed_method.deref()).add_code(code);

    // Sends the finalized function to be compiled.
    let mut locked = syntax.lock().unwrap();
    locked
        .compiling
        .insert(output.data.name.clone(), Arc::new(output));
    for waker in &locked.compiling_wakers {
        waker.wake_by_ref();
    }
    locked.compiling_wakers.clear();
}

/// A finalized function, which is ready to be compiled and has been checked of any errors.
#[derive(Clone, Debug)]
pub struct FinalizedFunction {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub fields: Vec<FinalizedMemberField>,
    pub code: FinalizedCodeBody,
    pub return_type: Option<FinalizedTypes>,
    pub data: Arc<FunctionData>,
}

impl FinalizedFunction {
    /// Recreates the CodelessFinalizedFunction
    pub fn to_codeless(&self) -> CodelessFinalizedFunction {
        return CodelessFinalizedFunction {
            generics: self.generics.clone(),
            arguments: self.fields.clone(),
            return_type: self.return_type.clone(),
            data: self.data.clone(),
        };
    }
}

/// A body of code, each body must have a label for jump effects to jump to.
/// ! Each nested CodeBody MUST have a jump or return or else the compiler will error !
#[derive(Clone, Default, Debug)]
pub struct CodeBody {
    pub label: String,
    pub expressions: Vec<Expression>,
}

/// A finalized body of code.
#[derive(Clone, Default, Debug)]
pub struct FinalizedCodeBody {
    pub label: String,
    pub expressions: Vec<FinalizedExpression>,
    pub returns: bool,
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>, label: String) -> Self {
        return Self { label, expressions };
    }
}

impl FinalizedCodeBody {
    pub fn new(expressions: Vec<FinalizedExpression>, label: String, returns: bool) -> Self {
        return Self {
            label,
            expressions,
            returns,
        };
    }

    /// Degenerics every effect inside the body of code.
    pub async fn degeneric(
        mut self,
        process_manager: &dyn ProcessManager,
        resolver: &dyn NameResolver,
        variables: &mut SimpleVariableManager,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> Result<FinalizedCodeBody, ParsingError> {
        for expression in &mut self.expressions {
            expression
                .effect
                .degeneric(process_manager, variables, resolver, syntax)
                .await?;
        }

        return Ok(self);
    }
}

/// Helper functions to display types.
pub fn display<T>(input: &Vec<T>, deliminator: &str) -> String
where
    T: Display,
{
    if input.is_empty() {
        return "()".to_string();
    }

    let mut output = String::default();
    for element in input {
        output += &*format!("{}{}", element, deliminator);
    }

    return format!(
        "({})",
        (&output[..output.len() - deliminator.len()]).to_string()
    );
}

pub fn display_parenless<T>(input: &Vec<T>, deliminator: &str) -> String
where
    T: Display,
{
    if input.is_empty() {
        return String::default();
    }

    let mut output = String::default();
    for element in input {
        output += &*format!("{}{}", element, deliminator);
    }

    return (&output[..output.len() - deliminator.len()]).to_string();
}

pub fn debug_parenless<T>(input: &Vec<T>, deliminator: &str) -> String
where
    T: Debug,
{
    if input.is_empty() {
        return String::default();
    }

    let mut output = String::default();
    for element in input {
        output += &*format!("{:?}{}", element, deliminator);
    }

    return (&output[..output.len() - deliminator.len()]).to_string();
}

impl Hash for FunctionData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for FunctionData {
    fn eq(&self, other: &Self) -> bool {
        return self.name == other.name;
    }
}

impl Eq for FunctionData {}
