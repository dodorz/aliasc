pub mod backend;
pub mod context;
pub mod dsl;
pub mod manifest;
pub mod resolver;
pub mod shortcut;
pub mod ssh_config;

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

#[cfg(test)]
mod tests {
    use super::resolve_default_shortcut_map;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn prefers_user_config_when_it_exists() {
        let d = tempdir().unwrap();
        fs::create_dir_all(d.path().join(".config")).unwrap();
        fs::write(d.path().join(".config").join("ShortcutMap.yaml"), "Config:\n  Foo: /tmp/foo\n").unwrap();
        let source = d.path().join("subdir").join("alias");
        let resolved = resolve_default_shortcut_map(d.path(), &source);
        assert_eq!(resolved, d.path().join(".config").join("ShortcutMap.yaml"));
    }

    #[test]
    fn falls_back_to_source_sibling() {
        let d = tempdir().unwrap();
        fs::create_dir_all(d.path().join("home")).unwrap();
        fs::create_dir_all(d.path().join("subdir")).unwrap();
        fs::write(d.path().join("subdir").join("ShortcutMap.yaml"), "Config:\n  Foo: /tmp/foo\n").unwrap();
        let source = d.path().join("subdir").join("alias");
        let resolved = resolve_default_shortcut_map(&d.path().join("home"), &source);
        assert_eq!(resolved, d.path().join("subdir").join("ShortcutMap.yaml"));
    }
}

pub fn render(compilation: &Compilation) -> Result<backend::Generated, Vec<Diagnostic>> {
    backend::generate(&compilation.context, &compilation.definitions)
}

/// Resolve the default ShortcutMap path for a source file.
///
/// Prefers `$HOME/.config/ShortcutMap.yaml`; when that file does not exist,
/// falls back to a `ShortcutMap.yaml` beside the alias source so that
/// source-adjacent layouts keep working and consumers that track the
/// source directory for cache invalidation stay correct.
pub fn default_shortcut_map(source: &Path) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
    resolve_default_shortcut_map(&home, source)
}

pub fn resolve_default_shortcut_map(home: &Path, source: &Path) -> PathBuf {
    let user_config = home.join(".config").join("ShortcutMap.yaml");
    if user_config.is_file() {
        return user_config;
    }
    source
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("ShortcutMap.yaml")
}
