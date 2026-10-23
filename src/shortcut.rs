//! ShortcutMap is an input adapter, not part of the core alias model.
//!
//! The resolver only consumes `RawDefinition` values. This module turns a
//! ShortcutMap YAML document into definitions by dispatching each top-level
//! key to a `ShortcutHandler`. Handlers own their schema and expansion rules;
//! the resolver never needs to know what `Config`, `Forward`, or any future
//! section means.

use crate::{
    dsl::{Diagnostic, RawDefinition, SourceSpan},
    CompileOptions,
};
use serde_yaml::Value;
use std::path::{Path, PathBuf};

/// Definitions, diagnostics, and files produced while loading a ShortcutMap.
#[derive(Default)]
pub struct ShortcutLoad {
    pub definitions: Vec<RawDefinition>,
    pub diagnostics: Vec<Diagnostic>,
    pub dependencies: Vec<PathBuf>,
}

/// Context shared by all ShortcutMap handlers.
///
/// Keep this small and explicit. Handlers that need more information, such as
/// the SSH config for a future `Forward` handler, should load it and report the
/// files through `HandlerOutput::dependencies`.
pub struct ShortcutContext<'a> {
    pub options: &'a CompileOptions,
    pub path: &'a Path,
}

impl ShortcutContext<'_> {
    fn span(&self) -> SourceSpan {
        SourceSpan {
            file: self.path.to_path_buf(),
            line: 1,
            column: 1,
        }
    }
}

/// The result of expanding one top-level ShortcutMap section.
#[derive(Default)]
pub struct HandlerOutput {
    pub definitions: Vec<RawDefinition>,
    pub diagnostics: Vec<Diagnostic>,
    pub dependencies: Vec<PathBuf>,
}

/// A ShortcutMap section handler.
///
/// New verbs are added by implementing this trait and registering the handler
/// in `handler_for`. The core resolver remains unaware of the verb.
pub trait ShortcutHandler {
    fn expand(
        &self,
        context: &ShortcutContext<'_>,
        key: &str,
        value: &Value,
    ) -> HandlerOutput;
}

struct PathVerb {
    key: &'static str,
    prefix: &'static str,
    suffix: &'static str,
    env: &'static str,
}

const PATH_VERBS: &[PathVerb] = &[
    PathVerb {
        key: "Config",
        prefix: "Edit",
        suffix: "Config",
        env: "EDITOR",
    },
    PathVerb {
        key: "History",
        prefix: "Edit",
        suffix: "History",
        env: "EDITOR",
    },
    PathVerb {
        key: "Log",
        prefix: "View",
        suffix: "Log",
        env: "LOGVIEWER",
    },
];

struct PathHandler;

impl ShortcutHandler for PathHandler {
    fn expand(
        &self,
        context: &ShortcutContext<'_>,
        key: &str,
        value: &Value,
    ) -> HandlerOutput {
        let Some(verb) = PATH_VERBS.iter().find(|verb| verb.key == key) else {
            return HandlerOutput::default();
        };
        let Some(entries) = value.as_mapping() else {
            return HandlerOutput {
                diagnostics: vec![Diagnostic::error(
                    context.span(),
                    format!("ShortcutMap {key} must be a mapping"),
                    Vec::new(),
                )],
                ..HandlerOutput::default()
            };
        };

        let mut output = HandlerOutput::default();
        for (entry_key, entry_value) in entries {
            let (Some(name), Some(path)) = (entry_key.as_str(), entry_value.as_str()) else {
                output.diagnostics.push(Diagnostic::error(
                    context.span(),
                    format!("ShortcutMap {key} entries must be strings"),
                    Vec::new(),
                ));
                continue;
            };
            output.definitions.push(definition(
                format!("{}{}{}", verb.prefix, name, verb.suffix),
                format!("${{{}}} {}", verb.env, quote_dsl(path)),
                context.span(),
            ));
        }
        output
    }
}

struct AliasesHandler;

impl ShortcutHandler for AliasesHandler {
    fn expand(
        &self,
        context: &ShortcutContext<'_>,
        _key: &str,
        value: &Value,
    ) -> HandlerOutput {
        let Some(entries) = value.as_mapping() else {
            return HandlerOutput {
                diagnostics: vec![Diagnostic::error(
                    context.span(),
                    "ShortcutMap aliases must be a mapping",
                    Vec::new(),
                )],
                ..HandlerOutput::default()
            };
        };

        let mut output = HandlerOutput::default();
        for (entry_key, entry_value) in entries {
            let (Some(name), Some(body)) = (entry_key.as_str(), entry_value.as_str()) else {
                output.diagnostics.push(Diagnostic::error(
                    context.span(),
                    "ShortcutMap aliases entries must be strings",
                    Vec::new(),
                ));
                continue;
            };
            output.definitions.push(definition(
                name.to_string(),
                body.to_string(),
                context.span(),
            ));
        }
        output
    }
}

static PATH_HANDLER: PathHandler = PathHandler;
static ALIASES_HANDLER: AliasesHandler = AliasesHandler;

fn handler_for(key: &str) -> Option<&'static dyn ShortcutHandler> {
    if PATH_VERBS.iter().any(|verb| verb.key == key) {
        return Some(&PATH_HANDLER);
    }
    match key {
        "aliases" => Some(&ALIASES_HANDLER),
        _ => None,
    }
}

/// Load and expand a ShortcutMap file.
///
/// A missing file is intentionally silent: the default ShortcutMap is
/// optional. The path is still returned as a dependency so the manifest can
/// record it.
pub fn load(path: &Path, options: &CompileOptions) -> ShortcutLoad {
    let mut output = ShortcutLoad {
        dependencies: vec![path.to_path_buf()],
        ..ShortcutLoad::default()
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return output;
    };
    let normalized = normalize_yaml(&text);
    let value: Value = match serde_yaml::from_str(&normalized) {
        Ok(value) => value,
        Err(error) => {
            let location = error.location();
            output.diagnostics.push(Diagnostic::error(
                SourceSpan {
                    file: path.to_path_buf(),
                    line: location.as_ref().map(|location| location.line()).unwrap_or(1),
                    column: location
                        .as_ref()
                        .map(|location| location.column())
                        .unwrap_or(1),
                },
                format!("invalid ShortcutMap YAML: {error}"),
                Vec::new(),
            ));
            return output;
        }
    };
    let Some(root) = value.as_mapping() else {
        output.diagnostics.push(Diagnostic::error(
            SourceSpan {
                file: path.to_path_buf(),
                line: 1,
                column: 1,
            },
            "ShortcutMap root must be a mapping",
            Vec::new(),
        ));
        return output;
    };

    let context = ShortcutContext { options, path };
    for (key, value) in root {
        let Some(key) = key.as_str() else {
            output.diagnostics.push(Diagnostic::error(
                context.span(),
                "ShortcutMap top-level keys must be strings",
                Vec::new(),
            ));
            continue;
        };
        let Some(handler) = handler_for(key) else {
            output.diagnostics.push(Diagnostic::warning(
                context.span(),
                format!("unknown ShortcutMap section `{key}`"),
                Vec::new(),
            ));
            continue;
        };
        let mut expanded = handler.expand(&context, key, value);
        output.definitions.append(&mut expanded.definitions);
        output.diagnostics.append(&mut expanded.diagnostics);
        output.dependencies.append(&mut expanded.dependencies);
    }
    output
}

fn definition(name: String, body: String, span: SourceSpan) -> RawDefinition {
    RawDefinition {
        name,
        body,
        section: Some("Common".into()),
        span,
        include_stack: Vec::new(),
        local: false,
    }
}

/// Quote a literal as an Alias DSL double-quoted argument.
///
/// ShortcutMap values are data, not DSL. Escaping here prevents a path from
/// accidentally introducing DSL syntax.
fn quote_dsl(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

/// Quote unquoted YAML values starting with `%`.
///
/// `%` at the beginning of a YAML scalar is a directive indicator, but values
/// such as `%USERPROFILE%/.config` are useful on Windows. Only the value after
/// the first colon is adjusted; the rest of the document is left untouched.
fn normalize_yaml(text: &str) -> String {
    text.lines()
        .map(|line| {
            let Some(colon) = line.find(':') else {
                return line.to_string();
            };
            let (prefix, value) = line.split_at(colon + 1);
            let value = value.trim();
            if value.starts_with('%')
                && !value.starts_with("'%")
                && !value.starts_with("\"%")
            {
                format!("{} '{}'", prefix, value.replace('\'', "''"))
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        context::{Distro, Environment, Platform, Shell},
        Context,
    };

    fn options(source: PathBuf) -> CompileOptions {
        CompileOptions {
            context: Context {
                shell: Shell::Posix,
                platform: Platform::Linux,
                distro: Distro::None,
                environment: Environment::None,
            },
            source,
            local: false,
            shortcut_map: None,
        }
    }

    #[test]
    fn quote_dsl_escapes_generated_literal_syntax() {
        assert_eq!(quote_dsl(r#"a"b\c$HOME"#), r#""a\"b\\c$HOME""#);
    }

    #[test]
    fn unknown_sections_are_reported_and_known_sections_are_expanded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ShortcutMap.yaml");
        std::fs::write(
            &path,
            "Config:\n  Foo: /tmp/foo\nUnknown:\n  Bar: baz\n",
        )
        .unwrap();
        let loaded = load(&path, &options(directory.path().join("alias")));
        assert_eq!(loaded.definitions.len(), 1);
        assert_eq!(loaded.definitions[0].name, "EditFooConfig");
        assert!(loaded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown ShortcutMap section")));
    }

    #[test]
    fn percent_values_remain_literal_paths() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ShortcutMap.yaml");
        std::fs::write(&path, "Config:\n  Foo: %USERPROFILE%/.foo\n").unwrap();
        let loaded = load(&path, &options(directory.path().join("alias")));
        assert_eq!(loaded.definitions.len(), 1);
        assert_eq!(
            loaded.definitions[0].body,
            "${EDITOR} \"%USERPROFILE%/.foo\""
        );
    }
}
