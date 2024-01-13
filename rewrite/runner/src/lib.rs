use std::any::TypeId;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::mem;
use std::ops::DerefMut;
use std::rc::Rc;

use program::{Program, ProgramAccess};
use types::{ComparableType, RawType, Type};
use util::source::SourceSet;
use util::IndexedGenericMap;

pub struct RavenRunner {
    pub task_managers: Vec<Box<dyn TaskManager>>,
    pub sources: Rc<Vec<Box<dyn SourceSet>>>,
    pub raw_tasks: Vec<Box<dyn RavenTask<RawType>>>,
    pub finalizing_tasks: Vec<Box<dyn RavenTask<dyn ComparableType>>>,
}

pub trait Downcastable<T> {
    fn downcast(&mut self) -> &mut T;
}

impl<T: RavenTask<E> + 'static, E: Type + 'static> Downcastable<T> for Vec<Box<dyn RavenTask<E>>> {
    fn downcast(&mut self) -> &mut T {
        for task in self {
            if task.id() == TypeId::of::<T>() {
                return unsafe { mem::transmute::<&mut Box<dyn RavenTask<E>>, &mut Box<T>>(task) };
            }
        }
        unreachable!()
    }
}

impl Default for RavenRunner {
    fn default() -> Self {
        return Self {
            task_managers: Default::default(),
            sources: Default::default(),
            raw_tasks: Default::default(),
            finalizing_tasks: Default::default(),
        };
    }
}

pub trait TaskManager {
    fn setup(
        &mut self,
        raw_tasks: &mut Vec<Box<dyn RavenTask<RawType>>>,
        finalizing_tasks: &mut Vec<Box<dyn RavenTask<dyn ComparableType>>>,
    );

    fn finish_setup(
        &mut self,
        raw_tasks: &mut Vec<Box<dyn RavenTask<RawType>>>,
        finalizing_tasks: &mut Vec<Box<dyn RavenTask<dyn ComparableType>>>,
    );
}

pub struct MapWrapper<T: ?Sized> {
    pub inner: IndexedGenericMap,
    _phantom: PhantomData<T>,
}

impl<E: Type> MapWrapper<E> {
    pub fn get_type<T: RavenTask<E> + 'static>(&self) -> &T {
        return self.inner.get_type();
    }

    pub fn get_type_mut<T: RavenTask<E> + 'static>(&mut self) -> &mut T {
        return self.inner.get_type_mut();
    }

    pub fn initialize<T: RavenTask<E> + 'static>(&mut self, value: T) {
        return self.inner.initialize(value);
    }
}

impl RavenRunner {
    pub fn setup(&mut self) {
        for task in &mut self.task_managers {
            task.setup(&mut self.raw_tasks, &mut self.finalizing_tasks);
        }

        for task in &mut self.task_managers {
            task.finish_setup(&mut self.raw_tasks, &mut self.finalizing_tasks);
        }
    }

    pub fn run(&mut self) {
        let mut program = SingleThreadedProgramAccess::<RawType>::default();
        program.program.borrow_mut().sources = self.sources.clone();

        for raw_task in &mut self.raw_tasks {
            raw_task.run(&mut program);
        }
    }
}

#[derive(Clone)]
pub struct SingleThreadedProgramAccess<T: Type> {
    program: Rc<RefCell<Program<T>>>,
}

impl<T: Type> Default for SingleThreadedProgramAccess<T> {
    fn default() -> Self {
        return Self { program: Rc::new(RefCell::new(Program::default())) };
    }
}

impl<T: Type + 'static> ProgramAccess<T> for SingleThreadedProgramAccess<T> {
    fn lock(&mut self) -> Box<dyn DerefMut<Target = Program<T>> + '_> {
        return Box::new(self.program.borrow_mut());
    }

    fn clone(&self) -> Box<dyn ProgramAccess<T>> {
        return Box::new(SingleThreadedProgramAccess { program: self.program.clone() });
    }
}

pub trait RavenTask<T: Type> {
    fn run(&self, program: &mut dyn ProgramAccess<T>);

    fn name(&self) -> &'static str;

    fn id(&self) -> TypeId;
}
