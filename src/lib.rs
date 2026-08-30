pub mod backend;
pub mod context;
pub mod dsl;
pub mod manifest;
pub mod resolver;

use std::path::{Path, PathBuf};

pub use context::{Context, Distro, Environment, Platform, Shell};
pub use dsl::{Definition, Diagnostic, Severity, SourceSpan};

#[derive(Clone, Debug)]
pub struct CompileOptions {
    pub context: Context,
    pub source: PathBuf,
    pub local: bool,
    pub shortcut_map: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct Compilation {
    pub definitions: Vec<Definition>,
    pub diagnostics: Vec<Diagnostic>,
    pub tracked_inputs: Vec<resolver::TrackedInput>,
    pub includes: Vec<PathBuf>,
    pub source: PathBuf,
    pub local_path: Option<PathBuf>,
    pub shortcut_map: Option<PathBuf>,
    pub context: Context,
}

pub fn compile_model(options: &CompileOptions) -> Result<Compilation, Vec<Diagnostic>> {
    resolver::resolve(options)
}

pub fn render(compilation: &Compilation) -> Result<backend::Generated, Vec<Diagnostic>> {
    backend::generate(&compilation.context, &compilation.definitions)
}

pub fn default_shortcut_map(source: &Path) -> PathBuf {
    source.parent().unwrap_or_else(|| Path::new(".")).join("ShortcutMap.yaml")
}
