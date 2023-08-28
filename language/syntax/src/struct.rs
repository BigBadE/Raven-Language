use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use chalk_integration::interner::ChalkIr;
use chalk_integration::RawId;
use chalk_ir::{AdtId, Binders, GenericArg, Substitution, TraitId, Ty, TyKind};
use chalk_solve::rust_ir::{AdtDatum, AdtDatumBound, AdtFlags, AdtKind, TraitDatum, TraitDatumBound, TraitFlags};
use no_deadlocks::Mutex;
use indexmap::map::IndexMap;
use lazy_static::lazy_static;
use async_trait::async_trait;
use crate::{DataType, is_modifier, Modifier, ParsingFuture, ProcessManager, Syntax, TopElement};
use crate::code::{FinalizedMemberField, MemberField};
use crate::{Attribute, ParsingError};
use crate::async_getters::AsyncGetter;
use crate::async_util::NameResolver;
use crate::function::UnfinalizedFunction;
use crate::types::{FinalizedTypes, Types};

lazy_static! {
pub static ref I64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "i64".to_string())));
pub static ref I32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "i32".to_string())));
pub static ref I16: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "i16".to_string())));
pub static ref I8: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "i8".to_string())));
pub static ref F64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "f64".to_string())));
pub static ref F32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "f32".to_string())));
pub static ref U64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "u64".to_string())));
pub static ref U32: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "u32".to_string())));
pub static ref U16: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "u16".to_string())));
pub static ref U8: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "u8".to_string())));
pub static ref BOOL: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "bool".to_string())));
pub static ref STR: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "str".to_string())));
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
        _ => panic!("Unknown internal type {}", name)
    };
}

pub static ID: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

#[derive(Clone, Debug)]
pub enum ChalkData {
    Trait(TraitDatum<ChalkIr>),
    Struct(Ty<ChalkIr>, AdtDatum<ChalkIr>),
}

impl ChalkData {
    pub fn to_trait(&self) -> &TraitDatum<ChalkIr> {
        return match self {
            ChalkData::Trait(inner) => inner,
            _ => panic!("Expected struct, found trait")
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
    pub chalk_data: ChalkData,
    pub id: u64,
    pub name: String,
    pub attributes: Vec<Attribute>,
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
        self.data.id.hash(state);
    }
}

impl Hash for StructData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.id.to_be_bytes())
    }
}

impl PartialEq for StructData {
    fn eq(&self, other: &Self) -> bool {
        return self.id == other.id;
    }
}

impl PartialEq for FinalizedStruct {
    fn eq(&self, other: &Self) -> bool {
        return self.data.id == other.data.id;
    }
}

impl StructData {
    pub fn new(attributes: Vec<Attribute>, modifiers: u8, name: String) -> Self {
        let mut id = ID.lock().unwrap();
        *id += 1;
        if is_modifier(modifiers, Modifier::Trait) {
            let trait_id = TraitId(RawId {
                index: *id as u32
            });
            return Self {
                attributes,
                chalk_data: ChalkData::Trait(TraitDatum {
                    id: trait_id,
                    binders: Binders::empty(ChalkIr, TraitDatumBound {
                        where_clauses: vec![],
                    }),
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
                }),
                id: *id,
                modifiers,
                name,
                poisoned: Vec::new(),
            };
        } else {
            let temp: &[GenericArg<ChalkIr>] = &[];
            let adt_id = AdtId(RawId {
                index: *id as u32
            });
            return Self {
                attributes,
                chalk_data: ChalkData::Struct(TyKind::Adt(adt_id,
                                                          Substitution::from_iter(ChalkIr, temp.into_iter())).intern(ChalkIr),
                                              AdtDatum {
                                                  binders: Binders::empty(ChalkIr, AdtDatumBound {
                                                      variants: vec![],
                                                      where_clauses: vec![],
                                                  }),
                                                  id: adt_id,
                                                  flags: AdtFlags {
                                                      upstream: false,
                                                      fundamental: false,
                                                      phantom_data: false,
                                                  },
                                                  kind: AdtKind::Struct,
                                              }),
                id: *id,
                modifiers,
                name,
                poisoned: Vec::new(),
            };
        }
    }

    pub fn fix_id(&mut self) {
        let mut id = ID.lock().unwrap();
        *id += 1;
        self.id = *id;
    }

    pub fn new_poisoned(name: String, error: ParsingError) -> Self {
        let mut output = Self::new(Vec::new(), 0, name);
        output.poisoned = vec!(error);
        return output;
    }
}

impl FinalizedStruct {
    pub fn empty_of(data: StructData) -> Self {
        return Self {
            generics: IndexMap::new(),
            fields: Vec::new(),
            data: Arc::new(data),
        };
    }

    pub async fn degeneric(&mut self, generics: &Vec<FinalizedTypes>, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
        let mut i = 0;
        for value in self.generics.values_mut() {
            for generic in value {
                if let FinalizedTypes::Generic(name, bounds) = generic {
                    let name = name.clone();
                    let temp: &FinalizedTypes = generics.get(i).unwrap();
                    for bound in bounds {
                        if !temp.of_type(&bound, syntax) {
                            panic!("Generic {} set to a {} which isn't a {}", name, temp.name(), bound.name());
                        }
                    }
                    *generic = temp.clone();
                    i += 1;
                } else {
                    panic!("Guhh?????");
                }
            }
        }

        for field in &mut self.fields {
            let types = &mut field.field.field_type;
            if let FinalizedTypes::Generic(name, _) = types {
                let index = self.generics.iter().position(|(other_name, _)| name == other_name).unwrap();
                let generic: &FinalizedTypes = generics.get(index).unwrap();
                *types = generic.clone();
            }
        }

        return Ok(());
    }
}

#[async_trait]
impl TopElement for StructData {
    type Unfinalized = UnfinalizedStruct;
    type Finalized = FinalizedStruct;

    fn id(&self) -> u64 {
        return self.id;
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
        return StructData::new_poisoned(name, error);
    }

    async fn verify(mut current: UnfinalizedStruct, syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) {
        let data = current.data.clone();
        let functions = current.functions;
        current.functions = Vec::new();
        let structure = Arc::new(process_manager.verify_struct(current, resolver.boxed_clone(), &syntax).await);
        {
            let mut locked = syntax.lock().unwrap();
            if let Some(wakers) = locked.structures.wakers.remove(&data.name) {
                for waker in wakers {
                    waker.wake();
                }
            }

            locked.structures.data.insert(data, structure.clone());
        }

        for function in functions {
            let (mut function, code) = process_manager.verify_func(function, &syntax).await;

            for (name, bounds) in &structure.generics {
                function.generics.insert(name.clone(), bounds.clone());
            }

            let function = process_manager.verify_code(function, code, resolver.boxed_clone(), &syntax).await;

            //SAFETY: compiling is only accessed from here and in the compiler, and neither is dropped
            //until after both finish.
            unsafe {
                Arc::get_mut_unchecked(&mut syntax.lock().unwrap().compiling)
            }.insert(function.data.name.clone(), Arc::new(function));
        }
    }

    fn get_manager(syntax: &mut Syntax) -> &mut AsyncGetter<Self> {
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