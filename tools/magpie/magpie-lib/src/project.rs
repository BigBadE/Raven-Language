use core::fmt::Debug;

use magpie_derive::RavenExtern;

/// A Raven project
#[derive(Debug, RavenExtern)]
pub struct RavenProject {
    /// Project name
    pub name: String,
}

/// A Raven project dependency
#[derive(Debug, RavenExtern)]
pub struct Dependency {
    /// Dependency name
    pub name: String,
}
