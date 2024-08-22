use parking_lot::Mutex;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use chalk_ir::{AdtId, Binders, GenericArg, Substitution, TraitId, Ty, TyKind};
use chalk_solve::rust_ir::{AdtDatum, AdtDatumBound, AdtFlags, AdtKind, TraitDatum, TraitDatumBound, TraitFlags};
use indexmap::map::IndexMap;
use lazy_static::lazy_static;

use async_trait::async_trait;
use data::tokens::Span;

use crate::async_util::{HandleWrapper, NameResolver, UnparsedType};
use crate::chalk_interner::ChalkIr;
use crate::program::code::{FinalizedMemberField, MemberField};
use crate::program::function::{FunctionData, UnfinalizedFunction};
use crate::program::types::FinalizedTypes;
use crate::top_element_manager::TopElementManager;
use crate::{is_modifier, DataType, Modifier, ParsingFuture, ProcessManager, Syntax, TopElement};
use crate::{Attribute, ParsingError};

lazy_static! {
    /// 64-bit integer type
    pub static ref I64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("i64".to_string())));
    /// 32-bit integer type
    pub static ref I32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("i32".to_string())));
    /// 16-bit integer type
    pub static ref I16: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("i16".to_string())));
    /// 8-bit integer type
    pub static ref I8: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("i8".to_string())));
    /// 64-bit float type
    pub static ref F64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("f64".to_string())));
    /// 32-bit float type
    pub static ref F32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("f32".to_string())));
    /// 64-bit unsigned integer type
    pub static ref U64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("u64".to_string())));
    /// 32-bit unsigned integer type
    pub static ref U32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("u32".to_string())));
    /// 16-bit unsigned integer type
    pub static ref U16: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("u16".to_string())));
    /// 8-bit unsigned integer type
    pub static ref U8: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("u8".to_string())));
    /// Boolean type
    pub static ref BOOL: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("bool".to_string())));
    /// String type
    pub static ref STR: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("str".to_string())));
    /// Character type
    pub static ref CHAR: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("char".to_string())));
    /// Void type
    pub static ref VOID: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::empty("()".to_string())));
}

/// Gets the internal struct from its name
pub fn get_internal(name: String) -> Arc<StructData> {
    return match name.as_str() {
        "i64" => I64.data.clone(),
        "i32" => I32.data.clone(),
        "i16" => I16.data.clone(),
        "i8" => I8.data.clone(),
        "f64" => F64.data.clone(),
        "f32" => F32.data.clone(),
        "u64" => U64.data.clone(),
        "u32" => U32.data.clone(),
        "u16" => U16.data.clone(),
        "u8" => U8.data.clone(),
        "bool" => BOOL.data.clone(),
        "str" => STR.data.clone(),
        "char" => CHAR.data.clone(),
        _ => panic!("Unknown internal type {}", name),
    };
}

/// The chalk data of the two different types
#[derive(Clone, Debug)]
pub enum ChalkData {
    /// Chalk data for traits
    Trait(Ty<ChalkIr>, AdtDatum<ChalkIr>, TraitDatum<ChalkIr>),
    /// Chalk data for structs
    Struct(Ty<ChalkIr>, AdtDatum<ChalkIr>),
}

impl ChalkData {
    /// Gets the Ty from the data
    pub fn get_ty(&self) -> &Ty<ChalkIr> {
        return match self {
            ChalkData::Trait(first, _, _) => &first,
            ChalkData::Struct(first, _) => &first,
        };
    }

    /// Gets the ADT from the data
    pub fn get_adt(&self) -> &AdtDatum<ChalkIr> {
        return match self {
            ChalkData::Trait(_, first, _) => &first,
            ChalkData::Struct(_, first) => &first,
        };
    }
}

/// The static data of a program.
#[derive(Clone)]
pub struct StructData {
    /// The program's modifiers
    pub modifiers: u8,
    /// The program's chalk data
    pub chalk_data: ChalkData,
    /// The program's numerical ID, each struct has a unique ID that starts at 0 and increments by 1
    pub id: u64,
    /// The program's name
    pub name: String,
    /// The struct's span
    pub span: Span,
    /// The program's attributes
    pub attributes: Vec<Attribute>,
    /// The program's functions, if it's a trait
    pub functions: Vec<Arc<FunctionData>>,
    /// The program's errors
    pub poisoned: Vec<ParsingError>,
}

/// An unfinalized struct
pub struct UnfinalizedStruct {
    /// The program's generics
    pub generics: IndexMap<String, Vec<UnparsedType>>,
    /// The program's fields
    pub fields: Vec<ParsingFuture<MemberField>>,
    /// The program's functions
    pub functions: Vec<UnfinalizedFunction>,
    /// The program's data
    pub data: Arc<StructData>,
}

impl DataType<StructData> for UnfinalizedStruct {
    fn data(&self) -> &Arc<StructData> {
        return &self.data;
    }
}

/// A finalized struct
#[derive(Clone, Debug)]
pub struct FinalizedStruct {
    /// The program's generics
    pub generics: IndexMap<String, FinalizedTypes>,
    /// The program's fields
    pub fields: Vec<FinalizedMemberField>,
    /// The program's data
    pub data: Arc<StructData>,
}

impl Hash for FinalizedStruct {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.name.hash(state);
    }
}

impl Hash for StructData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}

impl PartialEq for StructData {
    fn eq(&self, other: &Self) -> bool {
        return self.name == other.name;
    }
}

impl PartialEq for FinalizedStruct {
    fn eq(&self, other: &Self) -> bool {
        return self.data.name.split_once('$').unwrap_or((&self.data.name, &"")).0
            == other.data.name.split_once('$').unwrap_or((&other.data.name, &"")).0;
    }
}

static mut ID: AtomicU64 = AtomicU64::new(0);

impl StructData {
    /// creates an empty struct data, usually for internal structs
    pub fn empty(name: String) -> Self {
        let id = unsafe { ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) };
        return Self {
            attributes: Vec::default(),
            chalk_data: Self::get_chalk_data(id, 0),
            id,
            modifiers: Modifier::Internal as u8,
            name,
            span: Span::default(),
            functions: Vec::default(),
            poisoned: Vec::default(),
        };
    }

    /// Creates a new struct data with the given args
    pub fn new(
        attributes: Vec<Attribute>,
        functions: Vec<Arc<FunctionData>>,
        modifiers: u8,
        span: Span,
        name: String,
    ) -> Self {
        let id = unsafe { ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) };

        return Self {
            attributes,
            chalk_data: Self::get_chalk_data(id, modifiers),
            id,
            modifiers,
            name,
            span,
            functions,
            poisoned: Vec::default(),
        };
    }

    /// Sets the internal chalk data, used to make sure all ids are unique and incremental in an async environment
    pub fn get_chalk_data(id: u64, modifiers: u8) -> ChalkData {
        let temp: &[GenericArg<ChalkIr>] = &[];
        let adt_id = AdtId(id as u32);
        let tykind = TyKind::Adt(adt_id, Substitution::from_iter(ChalkIr, temp.into_iter())).intern(ChalkIr);
        let adt_data = AdtDatum {
            binders: Binders::empty(ChalkIr, AdtDatumBound { variants: vec![], where_clauses: vec![] }),
            id: adt_id,
            flags: AdtFlags { upstream: false, fundamental: false, phantom_data: false },
            kind: AdtKind::Struct,
        };
        return if is_modifier(modifiers, Modifier::Trait) {
            let trait_id = TraitId(id as u32);
            ChalkData::Trait(
                tykind,
                adt_data,
                TraitDatum {
                    id: trait_id,
                    binders: Binders::empty(ChalkIr, TraitDatumBound { where_clauses: vec![] }),
                    flags: TraitFlags {
                        auto: false,
                        marker: false,
                        upstream: false,
                        fundamental: false,
                        non_enumerable: false,
                        coinductive: false,
                    },
                    associated_ty_ids: vec![],
                    well_known: None,
                },
            )
        } else {
            ChalkData::Struct(tykind, adt_data)
        };
    }

    /// Creates a new poison'd struct data
    pub fn new_poisoned(name: String, error: ParsingError) -> Self {
        let mut output = Self::new(Vec::default(), Vec::default(), 0, error.span.clone(), name);
        output.poisoned = vec![error];
        return output;
    }
}

impl FinalizedStruct {
    /// Creates an empty struct from the data, usually for internal structs
    pub fn empty_of(data: StructData) -> Self {
        return Self { generics: IndexMap::default(), fields: Vec::default(), data: Arc::new(data) };
    }
}

#[async_trait]
impl TopElement for StructData {
    type Unfinalized = UnfinalizedStruct;
    type Finalized = FinalizedStruct;

    fn get_span(&self) -> &Span {
        return &self.span;
    }

    fn is_operator(&self) -> bool {
        return self.is_trait() && Attribute::find_attribute("operation", &self.attributes).is_some();
    }

    fn is_trait(&self) -> bool {
        return is_modifier(self.modifiers, Modifier::Trait);
    }

    fn default(&self, id: u64) -> Arc<Self> {
        return Arc::new(StructData {
            modifiers: 0,
            chalk_data: Self::get_chalk_data(id, 0),
            id,
            name: "empty".to_string(),
            span: Span::default(),
            attributes: vec![],
            functions: vec![],
            poisoned: vec![],
        });
    }

    fn id(&self) -> Option<u64> {
        return Some(self.id);
    }

    fn errors(&self) -> &Vec<ParsingError> {
        return &self.poisoned;
    }

    fn name(&self) -> &String {
        return &self.name;
    }

    fn new_poisoned(name: String, error: ParsingError) -> Self {
        return StructData::new_poisoned(name, error);
    }

    async fn verify(
        handle: Arc<Mutex<HandleWrapper>>,
        mut current: UnfinalizedStruct,
        syntax: Arc<Mutex<Syntax>>,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    ) -> Result<(), ParsingError> {
        let data = current.data.clone();
        let functions = current.functions;
        current.functions = Vec::default();
        let structure = Arc::new(process_manager.verify_struct(current, resolver.deref(), &syntax).await);
        {
            let mut locked = syntax.lock();
            locked.structures.add_data(data.clone(), structure.clone());
        }

        for function in functions {
            handle.lock().spawn(
                function.data.name.clone(),
                FunctionData::verify(
                    handle.clone(),
                    function,
                    syntax.clone(),
                    resolver.boxed_clone(),
                    process_manager.cloned(),
                ),
            );
        }
        handle.lock().finish_task(&data.name);
        return Ok(());
    }

    fn get_manager(syntax: &mut Syntax) -> &mut TopElementManager<Self> {
        return &mut syntax.structures;
    }
}

impl Debug for StructData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Eq for StructData {}

impl Eq for FinalizedStruct {}
