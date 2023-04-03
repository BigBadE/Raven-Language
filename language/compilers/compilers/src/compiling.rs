use async_trait::async_trait;
use syntax::ParsingError;

#[async_trait]
pub trait Compiler {
    /// Compiles the main function and returns the project.
    async fn compile(&self) -> Result<Box<dyn CompiledProject>, Vec<ParsingError>>;
}

pub trait CompiledProject {

}