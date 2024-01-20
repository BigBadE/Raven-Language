/// This file contains the representation of code in Raven and helper methods to transform that code.
use async_recursion::async_recursion;
use data::tokens::Span;
use std::mem;
use std::sync::Arc;
use std::sync::Mutex;

use crate::async_util::{AsyncDataGetter, UnparsedType};
use crate::function::{CodeBody, CodelessFinalizedFunction, FinalizedCodeBody, FunctionData};
use crate::r#struct::{FinalizedStruct, BOOL, CHAR, F64, STR, U64};
use crate::syntax::Syntax;
use crate::top_element_manager::ImplWaiter;
use crate::types::{FinalizedTypes, Types};
use crate::{Attribute, ParsingError, ProcessManager, SimpleVariableManager, VariableManager};

/// An expression is a single line of code, containing an effect and the type of expression.
#[derive(Clone, Debug)]
pub struct Expression {
    /// The expression type
    pub expression_type: ExpressionType,
    /// The contained code
    pub effect: Effects,
}

/// An expression that has been finalized.
#[derive(Clone, Debug)]
pub struct FinalizedExpression {
    /// The expression type
    pub expression_type: ExpressionType,
    /// The finalized code
    pub effect: FinalizedEffects,
}

/// the types of expressions: a normal line, a return, or a break (for inside control statements).
#[derive(Clone, Debug)]
pub enum ExpressionType {
    /// Breaks break out of a looping control statement like a for or while loop
    Break,
    /// Return returns out of the current function
    Return(Span),
    /// A line does nothing
    Line,
}

/// A field has a name and a type, see MemberField for the main use of fields.
#[derive(Clone, Debug)]
pub struct Field {
    /// The name of the field
    pub name: String,
    /// The field's type
    pub field_type: Types,
}

/// A finalized field.
#[derive(Clone, Debug)]
pub struct FinalizedField {
    /// The name of the field
    pub name: String,
    /// The field's type
    pub field_type: FinalizedTypes,
}

/// A field with modifiers and attributes, for example the arguments of a function or types of a struct.
#[derive(Clone, Debug)]
pub struct MemberField {
    /// The field's modifiers
    pub modifiers: u8,
    /// The field's attributes
    pub attributes: Vec<Attribute>,
    /// The field itself
    pub field: Field,
}

/// A finalized member field.
#[derive(Clone, Debug)]
pub struct FinalizedMemberField {
    /// The field's modifiers
    pub modifiers: u8,
    /// The field's attributes
    pub attributes: Vec<Attribute>,
    /// The field itself
    pub field: FinalizedField,
}

impl PartialEq for ExpressionType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ExpressionType::Return(_), ExpressionType::Return(_)) => true,
            (ExpressionType::Line, ExpressionType::Line) => true,
            (ExpressionType::Break, ExpressionType::Break) => true,
            _ => false,
        }
    }
}

impl MemberField {
    /// Creates a new field
    pub fn new(modifiers: u8, attributes: Vec<Attribute>, field: Field) -> Self {
        return Self { modifiers, attributes, field };
    }
}

impl Expression {
    /// Creates a new expression
    pub fn new(expression_type: ExpressionType, effect: Effects) -> Self {
        return Self { expression_type, effect };
    }
}

impl FinalizedExpression {
    /// Creates a new finalized expression
    pub fn new(expression_type: ExpressionType, effect: FinalizedEffects) -> Self {
        return Self { expression_type, effect };
    }
}

impl Field {
    /// Creates a new field
    pub fn new(name: String, field_type: Types) -> Self {
        return Self { name, field_type };
    }
}

/// Effects are single pieces of code which are strung together to make an expression.
/// For example, a single method call, creating a variable, setting a variable, etc... are all effects.
#[derive(Clone, Debug)]
pub struct Effects {
    /// The type of the effect
    pub types: EffectType,
    /// The span of the effect
    pub span: Span,
}

impl Effects {
    /// Creates a new effect
    pub fn new(span: Span, types: EffectType) -> Self {
        return Self { types, span };
    }
}

/// The type of the effect, storing all per-effect data
#[derive(Clone, Debug)]
pub enum EffectType {
    /// A placeholder of no operation, which should be resolved before finalizing.
    NOP,
    /// An effect wrapped in parenthesis, just a wrapper around the effect to prevent issues with operator merging.
    Paren(Box<Effects>),
    /// Creates a variable with the given name and value.
    CreateVariable(String, Box<Effects>),
    /// Label of jumping to body
    Jump(String),
    /// Comparison effect, and label to jump to the first if true, second if false
    CompareJump(Box<Effects>, String, String),
    /// A block of code inside the block of code.
    CodeBody(CodeBody),
    /// Finds the implementation of the given trait for the given calling type, and calls the given method.
    /// Calling, trait to call, function name, args, and return type (if explicitly required)
    ImplementationCall(Box<Effects>, String, String, Vec<Effects>, Option<UnparsedType>),
    /// Finds the method with the name and calls it with those arguments.
    /// Calling, calling function, function arguments, and return type (if explicitly required)
    MethodCall(Option<Box<Effects>>, String, Vec<Effects>, Option<(UnparsedType, Span)>),
    /// Sets the variable to a value.
    Set(Box<Effects>, Box<Effects>),
    /// Loads variable with the given name.
    LoadVariable(String),
    /// Loads a field with the given name from the program.
    Load(Box<Effects>, String),
    /// An unresolved operation, sent to the checker to resolve, with the given arguments.
    Operation(String, Vec<Effects>),
    /// Struct to create and a tuple of the name of the field and the argument.
    CreateStruct(UnparsedType, Vec<(String, Effects)>),
    /// Creates an array of the given effects.
    CreateArray(Vec<Effects>),
    /// A float
    Float(f64),
    /// An integer
    Int(i64),
    /// An unsigned integer
    UInt(u64),
    /// A boolean
    Bool(bool),
    /// A character
    Char(char),
    /// A string
    String(String),
}

/// Effects that have been finalized and are ready for compilation
#[derive(Clone, Debug)]
pub struct FinalizedEffects {
    /// The type of the effect
    pub types: FinalizedEffectType,
    /// The span of the effect
    pub span: Span,
}

impl FinalizedEffects {
    /// Creates a new finalized effect
    pub fn new(span: Span, types: FinalizedEffectType) -> Self {
        return Self { types, span };
    }
}

/// Effects that have been finalized and are ready for compilation
#[derive(Clone, Debug)]
pub enum FinalizedEffectType {
    ///  Exclusively used for void returns. Will make the compiler panic.
    NOP,
    ///  Creates a variable.
    CreateVariable(String, Box<FinalizedEffects>, FinalizedTypes),
    /// Jumps to the given label.
    Jump(String),
    /// Comparison effect, jumps to the given first label if true, or second label if false
    CompareJump(Box<FinalizedEffects>, String, String),
    /// Nested code body.
    CodeBody(FinalizedCodeBody),
    /// Calls the function on the given value (if any) with the given arguments.
    MethodCall(Option<Box<FinalizedEffects>>, Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    /// Calls the trait's function with the given arguments.
    GenericMethodCall(Arc<CodelessFinalizedFunction>, FinalizedTypes, Vec<FinalizedEffects>),
    /// Sets given reference to given value.
    Set(Box<FinalizedEffects>, Box<FinalizedEffects>),
    /// Loads variable with the given name.
    LoadVariable(String),
    /// Loads a field reference from the given struct with the given type.
    Load(Box<FinalizedEffects>, String, Arc<FinalizedStruct>),
    /// Creates a struct at the given reference, of the given type with a tuple of the index of the argument and the argument.
    CreateStruct(Option<Box<FinalizedEffects>>, FinalizedTypes, Vec<(usize, FinalizedEffects)>),
    /// Create an array with the type and values
    CreateArray(Option<FinalizedTypes>, Vec<FinalizedEffects>),
    /// Creates a float
    Float(f64),
    /// Creates an unsigned int
    UInt(u64),
    /// Creates a boolean
    Bool(bool),
    /// Creates a string
    String(String),
    /// Creates a character
    Char(char),
    /// Calls a virtual method, usually a downcasted trait, with the given function index, function,
    /// and on the given arguments (first argument must be the downcased trait).
    VirtualCall(usize, Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    /// Calls a virtual method on a generic type. Same as above, but must degeneric like check_code on EffectType::ImplementationCall
    GenericVirtualCall(usize, Arc<FunctionData>, Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    /// Downcasts a program into its trait (with the given functions), which can only be used in a VirtualCall.
    Downcast(Box<FinalizedEffects>, FinalizedTypes, Vec<Arc<FunctionData>>),
    /// Internally used by low-level verifier to store a type on the heap.
    HeapStore(Box<FinalizedEffects>),
    /// Allocates space on the heap.
    HeapAllocate(FinalizedTypes),
    /// Loads from the given reference.
    ReferenceLoad(Box<FinalizedEffects>),
    /// Stores an effect on the stack.
    StackStore(Box<FinalizedEffects>),
}

impl FinalizedEffectType {
    /// Flattens a type, which is the final step before compilation that gets rid of all generics in the type
    #[async_recursion]
    // skipcq: RS-R1000 Match statements have complexity calculated incorrectly
    pub async fn flatten(
        &mut self,
        syntax: &Arc<Mutex<Syntax>>,
        process_manager: &dyn ProcessManager,
        variables: &mut SimpleVariableManager,
    ) -> Result<(), ParsingError> {
        match self {
            Self::CreateVariable(name, value, types) => {
                value.types.flatten(syntax, process_manager, variables).await?;
                types.flatten(syntax).await?;
                variables.variables.insert(name.clone(), types.clone());
            }
            Self::CompareJump(effect, _, _) => effect.types.flatten(syntax, process_manager, variables).await?,
            Self::CodeBody(body) => body.flatten(syntax, process_manager, variables).await?,
            Self::MethodCall(calling, function, arguments) => {
                if let Some(found) = calling {
                    found.types.flatten(syntax, process_manager, variables).await?;
                }
                *function = function.flatten(syntax).await?;
                for argument in arguments {
                    argument.types.flatten(syntax, process_manager, variables).await?;
                }
            }
            Self::GenericMethodCall(function, types, arguments) => {
                types.flatten(syntax).await?;
                *function = function.flatten(syntax).await?;
                for argument in arguments {
                    argument.types.flatten(syntax, process_manager, variables).await?;
                }
            }
            Self::Set(base, value) => {
                base.types.flatten(syntax, process_manager, variables).await?;
                value.types.flatten(syntax, process_manager, variables).await?;
            }
            Self::Load(base, _, _) => {
                base.types.flatten(syntax, process_manager, variables).await?;
            }
            Self::CreateStruct(storing, types, effects) => {
                if let Some(found) = storing {
                    found.types.flatten(syntax, process_manager, variables).await?;
                }
                types.flatten(syntax).await?;
                for (_, found) in effects {
                    found.types.flatten(syntax, process_manager, variables).await?;
                }
            }
            Self::CreateArray(types, effects) => {
                if let Some(found) = types {
                    found.flatten(syntax).await?;
                }
                for effect in effects {
                    effect.types.flatten(syntax, process_manager, variables).await?;
                }
            }
            Self::VirtualCall(_, function, effects) => {
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
                    effect.types.flatten(syntax, process_manager, variables).await?;
                }
            }
            Self::GenericVirtualCall(_, _, _, effects) => {
                for effect in effects {
                    effect.types.flatten(syntax, process_manager, variables).await?;
                }
            }
            Self::Downcast(base, target, _) => {
                base.types.flatten(syntax, process_manager, variables).await?;
                target.flatten(syntax).await?;
            }
            Self::HeapStore(storing) => storing.types.flatten(syntax, process_manager, variables).await?,
            Self::HeapAllocate(types) => types.flatten(syntax).await?,
            Self::ReferenceLoad(base) => base.types.flatten(syntax, process_manager, variables).await?,
            Self::StackStore(storing) => storing.types.flatten(syntax, process_manager, variables).await?,
            _ => {}
        }
        return Ok(());
    }

    /// Gets the return type of the effect, requiring a variable manager to get
    /// any variables from, or None if the effect has no return type.
    pub fn get_return(&self, variables: &dyn VariableManager) -> Option<FinalizedTypes> {
        let temp = match self {
            Self::NOP | Self::Jump(_) | Self::CompareJump(_, _, _) | Self::CodeBody(_) => None,
            // Downcasts simply return the downcasting target.
            Self::CreateVariable(_, _, types) | Self::Downcast(_, types, _) => Some(types.clone()),
            Self::MethodCall(_, function, _)
            | Self::GenericMethodCall(function, _, _)
            | Self::VirtualCall(_, function, _)
            | Self::GenericVirtualCall(_, _, function, _) => {
                function.return_type.as_ref().map(|inner| FinalizedTypes::Reference(Box::new(inner.clone())))
            }
            Self::LoadVariable(name) => {
                let variable = variables.get_variable(name);
                if variable.is_some() {
                    return variable;
                }
                // Failed to find a variable with that name.
                panic!("Unresolved variable {} from {:?}", name, variables);
            }
            // Gets the type of the field in the program with that name.
            Self::Load(_, name, loading) => {
                loading.fields.iter().find(|field| &field.field.name == name).map(|field| field.field.field_type.clone())
            }
            // Returns the program type.
            Self::CreateStruct(_, types, _) => Some(FinalizedTypes::Reference(Box::new(types.clone()))),
            // Returns the internal constant type.
            Self::Float(_) => Some(FinalizedTypes::Struct(F64.clone())),
            Self::UInt(_) => Some(FinalizedTypes::Struct(U64.clone())),
            Self::Bool(_) => Some(FinalizedTypes::Struct(BOOL.clone())),
            Self::String(_) => Some(FinalizedTypes::Struct(STR.clone())),
            Self::Char(_) => Some(FinalizedTypes::Struct(CHAR.clone())),
            // Stores just return their inner type.
            Self::HeapStore(inner) | Self::StackStore(inner) | Self::Set(_, inner) => inner.types.get_return(variables),
            // References return their inner type as well.
            Self::ReferenceLoad(inner) => match inner.types.get_return(variables).unwrap() {
                FinalizedTypes::Reference(inner) => Some(*inner),
                _ => panic!("Tried to load non-reference!"),
            },
            // Heap allocations shouldn't get return type checked, even though they have a type.
            Self::HeapAllocate(_) => panic!("Tried to return type a heap allocation!"),
            // Returns the target type as an array type.
            Self::CreateArray(types, _) => types.clone().map(|inner| FinalizedTypes::Array(Box::new(inner))),
        };
        return temp;
    }

    /// Degenericing replaces every instance of a generic function with its actual type.
    /// This mostly targets FinalizedTypes or function calls and calls the degeneric function on them.
    #[async_recursion]
    // skipcq: RS-R1000 Match statements have complexity calculated incorrectly
    pub async fn degeneric(
        &mut self,
        process_manager: &dyn ProcessManager,
        variables: &mut SimpleVariableManager,
        syntax: &Arc<Mutex<Syntax>>,
        span: &Span,
    ) -> Result<(), ParsingError> {
        match self {
            // Recursively searches nested effects for method calls.
            Self::NOP
            | Self::Jump(_)
            | Self::LoadVariable(_)
            | Self::Float(_)
            | Self::UInt(_)
            | Self::Bool(_)
            | Self::String(_)
            | Self::Char(_) => {}
            Self::CreateVariable(_, first, other) => {
                first.types.degeneric(process_manager, variables, syntax, span).await?;
                other.degeneric(process_manager.generics(), syntax).await;
            }
            Self::CompareJump(effect, _, _)
            | Self::Load(effect, _, _)
            | Self::HeapStore(effect)
            | Self::ReferenceLoad(effect)
            | Self::StackStore(effect) => effect.types.degeneric(process_manager, variables, syntax, span).await?,
            Self::CodeBody(body) => {
                for statement in &mut body.expressions {
                    statement.effect.types.degeneric(process_manager, variables, syntax, span).await?;
                }
            }
            Self::MethodCall(calling, method, effects) => {
                if let Some(inner) = calling {
                    inner.types.degeneric(process_manager, variables, syntax, span).await?;
                }
                for effect in &mut *effects {
                    effect.types.degeneric(process_manager, variables, syntax, span).await?;
                }
                let manager: Box<dyn ProcessManager> = process_manager.cloned();
                // Calls the degeneric method on the method.
                *method =
                    CodelessFinalizedFunction::degeneric(method.clone(), manager, effects, syntax, variables, None).await?;
            }
            Self::GenericMethodCall(function, found_trait, effects) => {
                let mut calling = effects.remove(0);
                calling.types.degeneric(process_manager, variables, syntax, span).await?;

                let implementor = calling.types.get_return(variables).unwrap();
                let implementation = ImplWaiter {
                    syntax: syntax.clone(),
                    return_type: implementor.clone(),
                    data: found_trait.clone(),
                    error: ParsingError::new(
                        Span::default(),
                        "You shouldn't see this! Report this please! Location: Degeneric generic method call",
                    ),
                }
                .await?;

                let name = function.data.name.split("::").last().unwrap();
                let function = implementation.iter().find(|inner| inner.name.ends_with(&name)).unwrap();

                for effect in &mut *effects {
                    effect.types.degeneric(process_manager, variables, syntax, span).await?;
                }
                let mut effects = effects.clone();
                effects.insert(0, calling.clone());
                let function = AsyncDataGetter::new(syntax.clone(), function.clone()).await;
                let function = CodelessFinalizedFunction::degeneric(
                    function.clone(),
                    process_manager.cloned(),
                    &effects,
                    syntax,
                    variables,
                    None,
                )
                .await?;
                *self = Self::MethodCall(None, function, effects.clone());
            }
            // Virtual calls can't be generic because virtual calls aren't direct calls which can be degenericed.
            Self::VirtualCall(_, _, effects) => {
                for effect in &mut *effects {
                    effect.types.degeneric(process_manager, variables, syntax, span).await?;
                }
            }
            Self::Set(setting, value) => {
                setting.types.degeneric(process_manager, variables, syntax, span).await?;
                value.types.degeneric(process_manager, variables, syntax, span).await?;
            }
            Self::CreateStruct(target, types, effects) => {
                if let Some(found) = target {
                    found.types.degeneric(process_manager, variables, syntax, span).await?;
                }
                types.degeneric(process_manager.generics(), syntax).await;
                for (_, effect) in effects {
                    effect.types.degeneric(process_manager, variables, syntax, span).await?;
                }
            }
            Self::CreateArray(other, effects) => {
                if let Some(inner) = other {
                    inner.degeneric(process_manager.generics(), syntax).await;
                }
                for effect in effects {
                    effect.types.degeneric(process_manager, variables, syntax, span).await?;
                }
            }
            Self::HeapAllocate(target) | Self::Downcast(_, target, _) => {
                target.degeneric(process_manager.generics(), syntax).await
            }
            Self::GenericVirtualCall(index, target, found, effects) => {
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
                let output =
                    CodelessFinalizedFunction::degeneric(output, process_manager.cloned(), &temp, &syntax, variables, None)
                        .await?;
                *self = Self::VirtualCall(*index, output, temp);
            }
        }
        return Ok(());
    }
}

/// Degeneric's a function header
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

    let return_type = arguments[0].types.get_return(&variables).unwrap();
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
