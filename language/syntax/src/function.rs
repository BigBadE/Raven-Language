use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;
use std::thread;
use no_deadlocks::Mutex;

use async_trait::async_trait;
use indexmap::IndexMap;

use crate::{Attribute, DisplayIndented, ParsingError, TopElement, Types, ProcessManager, Syntax, AsyncGetter, is_modifier, Modifier, ParsingFuture, DataType, CheckerVariableManager};
use crate::async_util::{AsyncDataGetter, NameResolver};
use crate::code::{Expression, FinalizedEffects, FinalizedExpression, FinalizedMemberField, MemberField};
use crate::types::FinalizedTypes;

#[derive(Clone, Debug)]
pub struct FunctionData {
    pub attributes: Vec<Attribute>,
    pub modifiers: u8,
    pub name: String,
    pub poisoned: Vec<ParsingError>,
}

pub struct UnfinalizedFunction {
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    pub fields: Vec<ParsingFuture<MemberField>>,
    pub code: CodeBody,
    pub return_type: Option<ParsingFuture<Types>>,
    pub data: Arc<FunctionData>,
}

impl DataType<FunctionData> for UnfinalizedFunction {
    fn data(&self) -> &Arc<FunctionData> {
        return &self.data;
    }
}

/// If the code is required to finalize the function, then recursion will deadlock
#[derive(Clone)]
pub struct CodelessFinalizedFunction {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub fields: Vec<FinalizedMemberField>,
    pub return_type: Option<FinalizedTypes>,
    pub data: Arc<FunctionData>,
}

impl Debug for CodelessFinalizedFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}{}", self.data.name, debug_parenless(&self.fields, ", "));
    }
}

impl CodelessFinalizedFunction {
    pub fn add_code(self, code: FinalizedCodeBody) -> FinalizedFunction {
        return FinalizedFunction {
            generics: self.generics,
            fields: self.fields,
            code,
            return_type: self.return_type,
            data: self.data,
        };
    }

    //The VariableManager here is for effects
    pub async fn degeneric(method: Arc<CodelessFinalizedFunction>, mut manager: Box<dyn ProcessManager>,
                           effects: &Vec<FinalizedEffects>, syntax: &Arc<Mutex<Syntax>>,
                           variables: &CheckerVariableManager,
                           returning: Option<FinalizedTypes>) -> Result<Arc<CodelessFinalizedFunction>, ParsingError> {
        if let Some(inner) = method.return_type.clone() {
            if let Some(mut returning) = returning {
                //TODO remove this and get generic types working with explicit generics?
                if let FinalizedTypes::GenericType(inner, _) = returning {
                    returning = FinalizedTypes::clone(inner.deref());
                }
                if let Some((old, other)) = inner.resolve_generic(&returning, syntax, placeholder_error("Invalid bounds!".to_string())).await? {
                    if let FinalizedTypes::Generic(name, _) = old {
                        manager.mut_generics().insert(name, other);
                    } else {
                        panic!("Guh?");
                    }
                }
            }
        }

        for i in 0..method.fields.len() {
            let effect = effects.get(i).unwrap().get_return(variables).unwrap();
            if let Some((old, other)) = method.fields.get(i).unwrap().field.field_type.resolve_generic(
                &effect, syntax, placeholder_error("Invalid bounds!".to_string())).await? {
                if let FinalizedTypes::Generic(name, _) = old {
                    manager.mut_generics().insert(name, other);
                } else {
                    panic!("Guh?");
                }
            }
        }

        let name = format!("{}_{}", method.data.name.split("_").next().unwrap(), display_parenless(
            &manager.generics().values().collect(), "_"));
        if syntax.lock().unwrap().functions.types.contains_key(&name) {
            let data = syntax.lock().unwrap().functions.types.get(&name).unwrap().clone();
            return Ok(AsyncDataGetter::new(syntax.clone(), data).await);
        } else {
            let mut new_method = CodelessFinalizedFunction::clone(&method);
            new_method.generics.clear();
            let mut method_data = FunctionData::clone(&method.data);
            method_data.name = name.clone();
            new_method.data = Arc::new(method_data);
            for field in &mut new_method.fields {
                field.field.field_type.degeneric(&manager.generics(), syntax,
                                                 placeholder_error("No generic!".to_string()),
                                                 placeholder_error("Invalid bounds!".to_string())).await?;
            }

            if let Some(returning) = &mut new_method.return_type {
                returning.degeneric(&manager.generics(), syntax,
                                    placeholder_error("No generic!".to_string()),
                                    placeholder_error("Invalid bounds!".to_string())).await?;
            }
            let original = method;
            let new_method = Arc::new(new_method);
            let mut locked = syntax.lock().unwrap();
            if let Some(wakers) = locked.functions.wakers.remove(&name) {
                for waker in wakers {
                    waker.wake();
                }
            }

            locked.functions.types.insert(name, new_method.data.clone());
            locked.functions.data.insert(new_method.data.clone(), new_method.clone());

            let handle = manager.handle().clone();
            handle.spawn(degeneric_code(syntax.clone(), original, new_method.clone(), manager));
            return Ok(new_method);
        };
    }
}

fn placeholder_error(error: String) -> ParsingError {
    return ParsingError::new(String::new(), (0, 0), 0, (0, 0), 0, error);
}

async fn degeneric_code(syntax: Arc<Mutex<Syntax>>, original: Arc<CodelessFinalizedFunction>,
                        degenericed_method: Arc<CodelessFinalizedFunction>, manager: Box<dyn ProcessManager>) {
    while !syntax.lock().unwrap().compiling.contains_key(&original.data.name) {
        thread::yield_now();
    }

    let code = {
        let locked = syntax.lock().unwrap();
        locked.compiling.get(&original.data.name).unwrap().code.clone()
    };

    let mut variables = CheckerVariableManager::for_function(degenericed_method.deref());
    let code = match code.degeneric(&manager, &mut variables, &syntax).await {
        Ok(inner) => inner,
        Err(error) => panic!("Error degenericing code: {}", error)
    };

    let output = CodelessFinalizedFunction::clone(degenericed_method.deref())
        .add_code(code);

    unsafe { Arc::get_mut_unchecked(&mut syntax.lock().unwrap().compiling) }
        .insert(output.data.name.clone(), Arc::new(output));
}

#[derive(Clone, Debug)]
pub struct FinalizedFunction {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub fields: Vec<FinalizedMemberField>,
    pub code: FinalizedCodeBody,
    pub return_type: Option<FinalizedTypes>,
    pub data: Arc<FunctionData>,
}

impl FinalizedFunction {
    pub fn to_codeless(&self) -> CodelessFinalizedFunction {
        return CodelessFinalizedFunction {
            generics: self.generics.clone(),
            fields: self.fields.clone(),
            return_type: self.return_type.clone(),
            data: self.data.clone(),
        };
    }
}

impl FunctionData {
    pub fn new(attributes: Vec<Attribute>, modifiers: u8, name: String) -> Self {
        return Self {
            attributes,
            modifiers,
            name,
            poisoned: Vec::new(),
        };
    }

    pub fn poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: Vec::new(),
            modifiers: 0,
            name,
            poisoned: vec!(error),
        };
    }
}

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

    async fn verify(current: UnfinalizedFunction, syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) {
        let name = current.data.name.clone();
        let (output, code) = process_manager.verify_func(current, &syntax).await;
        let output = process_manager.verify_code(output, code, resolver, &syntax).await;
        //SAFETY: compiling is only accessed from here and in the compiler, and neither is dropped
        //until after both finish.
        unsafe {
            Arc::get_mut_unchecked(&mut syntax.lock().unwrap().compiling)
        }.insert(name, Arc::new(output));
    }

    fn get_manager(syntax: &mut Syntax) -> &mut AsyncGetter<Self> {
        return &mut syntax.functions;
    }
}

#[derive(Clone, Default, Debug)]
pub struct CodeBody {
    pub label: String,
    pub expressions: Vec<Expression>,
}

#[derive(Clone, Default, Debug)]
pub struct FinalizedCodeBody {
    pub label: String,
    pub expressions: Vec<FinalizedExpression>,
    pub returns: bool,
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>, label: String) -> Self {
        return Self {
            label,
            expressions,
        };
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

    pub async fn degeneric(mut self, process_manager: &Box<dyn ProcessManager>, variables: &mut CheckerVariableManager, syntax: &Arc<Mutex<Syntax>>) -> Result<FinalizedCodeBody, ParsingError> {
        for expression in &mut self.expressions {
            expression.effect.degeneric(process_manager, variables, syntax).await?;
        }

        return Ok(self);
    }
}

pub fn display_joined<T>(input: &Vec<T>) -> String where T: Display {
    if input.is_empty() {
        return String::new();
    }
    let mut output = String::new();
    for element in input {
        output += &*format!("{} ", element);
    }
    return output[..output.len() - 1].to_string();
}

pub fn display<T>(input: &Vec<T>, deliminator: &str) -> String where T: Display {
    if input.is_empty() {
        return "()".to_string();
    }

    let mut output = String::new();
    for element in input {
        output += &*format!("{}{}", element, deliminator);
    }

    return format!("({})", (&output[..output.len() - deliminator.len()]).to_string());
}

pub fn display_indented<T>(f: &mut Formatter<'_>, input: &Vec<T>, space: &str, deliminator: &str)
                           -> std::fmt::Result where T: DisplayIndented {
    write!(f, "(")?;
    for element in input {
        element.format(space, f)?;
        write!(f, "{}", deliminator)?;
    }

    return write!(f, ")");
}

pub fn display_parenless<T>(input: &Vec<T>, deliminator: &str) -> String where T: Display {
    if input.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    for element in input {
        output += &*format!("{}{}", element, deliminator);
    }

    return (&output[..output.len() - deliminator.len()]).to_string();
}

pub fn debug_parenless<T>(input: &Vec<T>, deliminator: &str) -> String where T: Debug {
    if input.is_empty() {
        return String::new();
    }

    let mut output = String::new();
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