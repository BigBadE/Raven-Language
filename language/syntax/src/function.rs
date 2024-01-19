use std::fmt::{Debug, Display};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};

use async_trait::async_trait;
use data::tokens::Span;
use indexmap::IndexMap;

use crate::async_util::{AsyncDataGetter, HandleWrapper, NameResolver};
use crate::code::{Expression, FinalizedEffects, FinalizedExpression, FinalizedMemberField, MemberField};
use crate::types::FinalizedTypes;
use crate::{
    is_modifier, Attribute, DataType, Modifier, ParsingError, ParsingFuture, ProcessManager, SimpleVariableManager, Syntax,
    TopElement, TopElementManager, Types,
};

/// The static data of a function, which is set during parsing and immutable throughout the entire compilation process.
/// Generics will copy this and change the name and types, but never modify the original.
#[derive(Clone, Debug)]
pub struct FunctionData {
    /// The function's attributes
    pub attributes: Vec<Attribute>,
    /// The function's modifiers
    pub modifiers: u8,
    /// The function's name
    pub name: String,
    /// The function's span
    pub span: Span,
    /// The function's errors if it has been poison'd
    pub poisoned: Vec<ParsingError>,
}

impl FunctionData {
    /// Creates a new function
    pub fn new(attributes: Vec<Attribute>, modifiers: u8, name: String, span: Span) -> Self {
        return Self { attributes, modifiers, name, span, poisoned: Vec::default() };
    }

    /// Creates an empty function data that errored while parsing.
    pub fn poisoned(name: String, error: ParsingError) -> Self {
        return Self { attributes: Vec::default(), modifiers: 0, name, span: error.span.clone(), poisoned: vec![error] };
    }
}

/// Allows generic access to FunctionData.
#[async_trait]
impl TopElement for FunctionData {
    type Unfinalized = UnfinalizedFunction;
    type Finalized = CodelessFinalizedFunction;

    fn get_span(&self) -> &Span {
        return &self.span;
    }

    fn set_id(&mut self, _id: u64) {
        //Ignored. Funcs don't have IDs
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
        let mut finalized_function = process_manager.verify_code(codeless_function, code, resolver, &syntax).await;

        if finalized_function.generics.is_empty() {
            finalized_function.flatten(&syntax, &*process_manager).await.unwrap();
        }

        let finalized_function = Arc::new(finalized_function);
        let mut locked = syntax.lock().unwrap();

        // Add the finalized code to the compiling list.
        locked.add_compiling(finalized_function.clone());
        handle.lock().unwrap().finish_task(&name);
    }

    fn get_manager(syntax: &mut Syntax) -> &mut TopElementManager<Self> {
        return &mut syntax.functions;
    }
}

/// An unfinalized function is the unlinked function directly after parsing, with no code.
/// Code is finalizied separately and combined with this to make a FinalizedFunction.
pub struct UnfinalizedFunction {
    /// The ordered generics of the function
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    /// The function's fields
    pub fields: Vec<ParsingFuture<MemberField>>,
    /// The function's code
    pub code: CodeBody,
    /// The function's return type
    pub return_type: Option<ParsingFuture<Types>>,
    /// The function's data
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
    /// The function's generics
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    /// The function's arguments
    pub arguments: Vec<FinalizedMemberField>,
    /// The function's return type
    pub return_type: Option<FinalizedTypes>,
    /// The function's data
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

    pub async fn flatten(&self, syntax: &Arc<Mutex<Syntax>>) -> Result<Arc<CodelessFinalizedFunction>, ParsingError> {
        let mut output = CodelessFinalizedFunction::clone(self);
        if let Some(found) = &mut output.return_type {
            found.flatten(syntax).await?;
        }

        for field in &mut output.arguments {
            field.field.field_type.flatten(syntax).await?;
        }

        return Ok(Arc::new(output));
    }

    /// Makes a copy of the CodelessFinalizedFunction with all the generics solidified into their actual type.
    /// Figures out the solidified types by comparing generics against the input effect types,
    /// then replaces all generic types with their solidified types.
    /// This can't always figure out return types, so an optional return type variable is passed as well
    /// for function calls that include them (see EffectType::MethodCall)
    /// The VariableManager here is for the arguments to the function, and not for the function itself.
    pub async fn degeneric(
        method: Arc<CodelessFinalizedFunction>,
        mut manager: Box<dyn ProcessManager>,
        arguments: &Vec<FinalizedEffects>,
        syntax: &Arc<Mutex<Syntax>>,
        variables: &SimpleVariableManager,
        returning: Option<(FinalizedTypes, Span)>,
    ) -> Result<Arc<CodelessFinalizedFunction>, ParsingError> {
        // Degenerics the return type if there is one and returning is some.
        if let Some(inner) = method.return_type.clone() {
            if let Some((returning, span)) = returning {
                inner
                    .resolve_generic(&returning, syntax, manager.mut_generics(), span.make_error("Invalid bounds!"))
                    .await?;
            }
        }

        //Degenerics the arguments to the method
        for i in 0..method.arguments.len() {
            let mut effect = arguments[i].types.get_return(variables).unwrap();

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

/// A finalized function, which is ready to be compiled and has been checked of any errors.
#[derive(Clone, Debug)]
pub struct FinalizedFunction {
    /// The function's generics
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    /// The function's fields
    pub fields: Vec<FinalizedMemberField>,
    /// The function's code
    pub code: FinalizedCodeBody,
    /// The function's return type
    pub return_type: Option<FinalizedTypes>,
    /// The function's data
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

    pub async fn flatten(
        &mut self,
        syntax: &Arc<Mutex<Syntax>>,
        process_manager: &dyn ProcessManager,
    ) -> Result<(), ParsingError> {
        let mut variables = SimpleVariableManager::for_final_function(self);
        self.code.flatten(syntax, process_manager, &mut variables).await?;
        for field in &mut self.fields {
            field.field.field_type.flatten(&syntax).await?;
        }
        return Ok(());
    }
}

/// A body of code, each body must have a label for jump effects to jump to.
/// ! Each nested CodeBody MUST have a jump or return or else the compiler will error !
#[derive(Clone, Default, Debug)]
pub struct CodeBody {
    /// A unique label for this code body, never shown to the user but used by the compiler for jumps
    pub label: String,
    /// The code in this code body
    pub expressions: Vec<Expression>,
}

/// A finalized body of code.
#[derive(Clone, Default, Debug)]
pub struct FinalizedCodeBody {
    /// A unique label for this code body, never shown to the user but used by the compiler for jumps
    pub label: String,
    /// The code in this code body
    pub expressions: Vec<FinalizedExpression>,
    /// Whether every code path in this code body returns
    pub returns: bool,
}

impl CodeBody {
    /// Creates a new code body
    pub fn new(expressions: Vec<Expression>, label: String) -> Self {
        return Self { label, expressions };
    }
}

impl FinalizedCodeBody {
    /// Creates a new code body
    pub fn new(expressions: Vec<FinalizedExpression>, label: String, returns: bool) -> Self {
        return Self { label, expressions, returns };
    }

    pub async fn flatten(
        &mut self,
        syntax: &Arc<Mutex<Syntax>>,
        process_manager: &dyn ProcessManager,
        variables: &mut SimpleVariableManager,
    ) -> Result<(), ParsingError> {
        for line in &mut self.expressions {
            line.effect.types.flatten(syntax, process_manager, variables).await?;
        }
        return Ok(());
    }

    /// Degenerics every effect inside the body of code.
    pub async fn degeneric(
        mut self,
        process_manager: &dyn ProcessManager,
        variables: &mut SimpleVariableManager,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> Result<FinalizedCodeBody, ParsingError> {
        for expression in &mut self.expressions {
            expression.effect.types.degeneric(process_manager, variables, syntax, &expression.effect.span).await?;
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

    return format!("({})", (&output[..output.len() - deliminator.len()]).to_string());
}

/// Helper functions to display types without parenthesis.
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
