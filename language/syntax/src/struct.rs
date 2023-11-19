use crate::async_util::{HandleWrapper, NameResolver};
use crate::chalk_interner::ChalkIr;
use crate::code::{FinalizedMemberField, MemberField};
use crate::function::{FunctionData, UnfinalizedFunction};
use crate::top_element_manager::TopElementManager;
use crate::types::{FinalizedTypes, Types};
use crate::{is_modifier, DataType, Modifier, ParsingFuture, ProcessManager, Syntax, TopElement};
use crate::{Attribute, ParsingError};
use async_trait::async_trait;
use chalk_ir::{AdtId, Binders, GenericArg, Substitution, TraitId, Ty, TyKind};
use chalk_solve::rust_ir::{
    AdtDatum, AdtDatumBound, AdtFlags, AdtKind, TraitDatum, TraitDatumBound, TraitFlags,
};
use indexmap::map::IndexMap;
use lazy_static::lazy_static;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::Mutex;

lazy_static! {
    pub static ref I64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("i64".to_string())
    ));
    pub static ref I32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("i32".to_string())
    ));
    pub static ref I16: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("i16".to_string())
    ));
    pub static ref I8: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("i8".to_string())
    ));
    pub static ref F64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("f64".to_string())
    ));
    pub static ref F32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("f32".to_string())
    ));
    pub static ref U64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("u64".to_string())
    ));
    pub static ref U32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("u32".to_string())
    ));
    pub static ref U16: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("u16".to_string())
    ));
    pub static ref U8: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("u8".to_string())
    ));
    pub static ref BOOL: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("bool".to_string())
    ));
    pub static ref STR: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("str".to_string())
    ));
    pub static ref CHAR: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("char".to_string())
    ));
    pub static ref VOID: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(
        StructData::empty("()".to_string())
    ));
}

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

#[derive(Clone, Debug)]
pub enum ChalkData {
    Trait(Ty<ChalkIr>, AdtDatum<ChalkIr>, TraitDatum<ChalkIr>),
    Struct(Ty<ChalkIr>, AdtDatum<ChalkIr>),
}

impl ChalkData {
    pub fn get_ty(&self) -> &Ty<ChalkIr> {
        return match self {
            ChalkData::Trait(first, _, _) => &first,
            ChalkData::Struct(first, _) => &first,
        };
    }

    pub fn get_adt(&self) -> &AdtDatum<ChalkIr> {
        return match self {
            ChalkData::Trait(_, first, _) => &first,
            ChalkData::Struct(_, first) => &first,
        };
    }

    pub fn to_trait(&self) -> (&Ty<ChalkIr>, &AdtDatum<ChalkIr>, &TraitDatum<ChalkIr>) {
        return match self {
            ChalkData::Trait(inner, data, other) => (inner, data, other),
            _ => panic!("Expected struct, found trait"),
        };
    }

    pub fn to_struct(&self) -> (&Ty<ChalkIr>, &AdtDatum<ChalkIr>) {
        return match self {
            ChalkData::Struct(types, inner) => (types, inner),
            _ => panic!("Expected struct, found trait"),
        };
    }
}

#[derive(Clone)]
pub struct StructData {
    pub modifiers: u8,
    pub chalk_data: Option<ChalkData>,
    pub id: u64,
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub functions: Vec<Arc<FunctionData>>,
    pub poisoned: Vec<ParsingError>,
}

pub struct UnfinalizedStruct {
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    pub fields: Vec<ParsingFuture<MemberField>>,
    pub functions: Vec<UnfinalizedFunction>,
    pub data: Arc<StructData>,
}

impl DataType<StructData> for UnfinalizedStruct {
    fn data(&self) -> &Arc<StructData> {
        return &self.data;
    }
}

#[derive(Clone, Debug)]
pub struct FinalizedStruct {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub fields: Vec<FinalizedMemberField>,
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
        return self.data.name == other.data.name;
    }
}

impl StructData {
    pub fn empty(name: String) -> Self {
        return Self {
            attributes: Vec::default(),
            chalk_data: None,
            id: 0,
            modifiers: Modifier::Internal as u8,
            name,
            functions: Vec::default(),
            poisoned: Vec::default(),
        };
    }

    pub fn new(
        attributes: Vec<Attribute>,
        functions: Vec<Arc<FunctionData>>,
        modifiers: u8,
        name: String,
    ) -> Self {
        return Self {
            attributes,
            chalk_data: None,
            id: 0,
            modifiers,
            name,
            functions,
            poisoned: Vec::default(),
        };
    }

    pub fn set_chalk_data(&mut self) {
        let temp: &[GenericArg<ChalkIr>] = &[];
        let adt_id = AdtId(self.id as u32);
        let tykind =
            TyKind::Adt(adt_id, Substitution::from_iter(ChalkIr, temp.into_iter())).intern(ChalkIr);
        let adt_data = AdtDatum {
            binders: Binders::empty(
                ChalkIr,
                AdtDatumBound {
                    variants: vec![],
                    where_clauses: vec![],
                },
            ),
            id: adt_id,
            flags: AdtFlags {
                upstream: false,
                fundamental: false,
                phantom_data: false,
            },
            kind: AdtKind::Struct,
        };
        if is_modifier(self.modifiers, Modifier::Trait) {
            let trait_id = TraitId(self.id as u32);
            self.chalk_data = Some(ChalkData::Trait(
                tykind,
                adt_data,
                TraitDatum {
                    id: trait_id,
                    binders: Binders::empty(
                        ChalkIr,
                        TraitDatumBound {
                            where_clauses: vec![],
                        },
                    ),
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
            ))
        } else {
            self.chalk_data = Some(ChalkData::Struct(tykind, adt_data));
        }
    }

    pub fn new_poisoned(name: String, error: ParsingError) -> Self {
        let mut output = Self::new(Vec::default(), Vec::default(), 0, name);
        output.poisoned = vec![error];
        return output;
    }
}

impl FinalizedStruct {
    pub fn empty_of(data: StructData) -> Self {
        return Self {
            generics: IndexMap::default(),
            fields: Vec::default(),
            data: Arc::new(data),
        };
    }

    pub async fn degeneric(
        &mut self,
        generics: &Vec<FinalizedTypes>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> Result<(), ParsingError> {
        let mut i = 0;
        for value in self.generics.values_mut() {
            for generic in value {
                if let FinalizedTypes::Generic(name, bounds) = generic {
                    let name = name.clone();
                    let temp: &FinalizedTypes = generics.get(i).unwrap();
                    for bound in bounds {
                        if !temp.of_type(&bound, syntax.clone()).await {
                            panic!(
                                "Generic {} set to a {} which isn't a {}",
                                name,
                                temp.name(),
                                bound.name()
                            );
                        }
                    }
                    generic.clone_from(temp);
                    i += 1;
                } else {
                    panic!("Guhh?????");
                }
            }
        }

        for field in &mut self.fields {
            let types = &mut field.field.field_type;
            if let FinalizedTypes::Generic(name, _) = types {
                let index = self
                    .generics
                    .iter()
                    .position(|(other_name, _)| name == other_name)
                    .unwrap();
                let generic: &FinalizedTypes = generics.get(index).unwrap();
                types.clone_from(generic);
            }
        }

        return Ok(());
    }
}

#[async_trait]
impl TopElement for StructData {
    type Unfinalized = UnfinalizedStruct;
    type Finalized = FinalizedStruct;

    fn set_id(&mut self, id: u64) {
        self.id = id;
        self.set_chalk_data();
    }

    fn poison(&mut self, error: ParsingError) {
        self.poisoned.push(error);
    }

    fn is_operator(&self) -> bool {
        return self.is_trait()
            && Attribute::find_attribute("operation", &self.attributes).is_some();
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
        return StructData::new_poisoned(name, error);
    }

    async fn verify(
        handle: Arc<Mutex<HandleWrapper>>,
        mut current: UnfinalizedStruct,
        syntax: Arc<Mutex<Syntax>>,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    ) {
        let data = current.data.clone();
        let functions = current.functions;
        current.functions = Vec::default();
        let structure = Arc::new(
            process_manager
                .verify_struct(current, resolver.boxed_clone(), &syntax)
                .await,
        );
        {
            let mut locked = syntax.lock().unwrap();
            if let Some(wakers) = locked.structures.wakers.remove(&data.name) {
                for waker in wakers {
                    waker.wake();
                }
            }

            locked
                .structures
                .data
                .insert(data.clone(), structure.clone());
        }

        for function in functions {
            let (mut function, code) = process_manager.verify_func(function, &syntax).await;

            for (name, bounds) in &structure.generics {
                function.generics.insert(name.clone(), bounds.clone());
            }

            let function = process_manager
                .verify_code(function, code, resolver.boxed_clone(), &syntax)
                .await;

            let mut locked = syntax.lock().unwrap();
            locked
                .compiling
                .insert(function.data.name.clone(), Arc::new(function));
            for waker in &locked.compiling_wakers {
                waker.wake_by_ref();
            }
            locked.compiling_wakers.clear();
        }
        handle.lock().unwrap().finish_task(&data.name);
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
