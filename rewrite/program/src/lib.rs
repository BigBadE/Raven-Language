use lexer::Span;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::rc::Rc;
use types::Type;
use util::source::SourceSet;
use util::GenericMap;

pub struct Program<T: Type> {
    components: GenericMap,
    pub sources: Rc<Vec<Box<dyn SourceSet>>>,
    _phantom: PhantomData<T>,
}

impl<T: Type> Default for Program<T> {
    fn default() -> Self {
        return Self {
            components: GenericMap::default(),
            sources: Rc::new(Vec::default()),
            _phantom: PhantomData::default(),
        };
    }
}

impl<T: Type> Program<T> {
    pub fn get<C: ProgramComponent + 'static>(&self) -> &C {
        return self.components.get_type::<C>();
    }

    pub fn add_component<C: ProgramComponent + 'static>(&mut self, adding: C) {
        self.components.initialize(adding);
    }
}

pub trait ProgramAccess<T: Type> {
    fn lock(&mut self) -> Box<dyn DerefMut<Target = Program<T>> + '_>;

    fn clone(&self) -> Box<dyn ProgramAccess<T>>;
}

pub trait ProgramComponent {
    fn join(&mut self, other: &dyn ProgramComponent);
}

pub struct ProgramError {
    span: Span,
    error: &'static str,
}
