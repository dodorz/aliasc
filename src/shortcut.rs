//! ShortcutMap is an input adapter, not part of the core alias model.
//!
//! The resolver only consumes `RawDefinition` values. This module turns a
//! ShortcutMap YAML document into definitions by dispatching each top-level
//! key to a `ShortcutHandler`. Handlers own their schema and expansion rules;
//! the resolver never needs to know what `Config`, `Forward`, or any future
//! section means.

use crate::{
    dsl::{Diagnostic, RawDefinition, SourceSpan},
    ssh_config::{self, SshConfig, SshConfigLoad},
    CompileOptions,
};
use serde::Deserialize;
use serde_yaml::Value;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

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

struct AliasesHandler;impl ShortcutHandler for AliasesHandler {
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

struct ForwardHandler;

#[derive(Deserialize)]
#[serde(untagged)]
enum ForwardSection {
    Detailed(ForwardDetailed),
    Apps(BTreeMap<String, ForwardSpec>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForwardDetailed {
    #[serde(default)]
    hosts: Option<HostSelection>,
    #[serde(default)]
    exclude_hosts: Vec<String>,
    #[serde(default)]
    exclude_users: Vec<String>,
    apps: BTreeMap<String, ForwardSpec>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum HostSelection {
    Names(Vec<String>),
    Aliases(BTreeMap<String, String>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ForwardSpec {
    Port(u16),
    Options(ForwardOptions),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForwardOptions {
    #[serde(alias = "remote_port")]
    port: u16,
    #[serde(default)]
    local_port: Option<u16>,
    #[serde(default)]
    bind: Option<String>,
    #[serde(default)]
    remote_host: Option<String>,
}

struct ForwardParts<'a> {
    remote_port: u16,
    local_port: u16,
    bind: Option<&'a str>,
    remote_host: &'a str,
}

impl ForwardSpec {
    fn parts(&self) -> ForwardParts<'_> {
        match self {
            Self::Port(port) => ForwardParts {
                remote_port: *port,
                local_port: *port,
                bind: None,
                remote_host: "127.0.0.1",
            },
            Self::Options(options) => ForwardParts {
                remote_port: options.port,
                local_port: options.local_port.unwrap_or(options.port),
                bind: options.bind.as_deref(),
                remote_host: options.remote_host.as_deref().unwrap_or("127.0.0.1"),
            },
        }
    }
}

struct SelectedHost {
    ssh_name: String,
    alias_name: String,
    user: Option<String>,
}

impl ShortcutHandler for ForwardHandler {
    fn expand(
        &self,
        context: &ShortcutContext<'_>,
        _key: &str,
        value: &Value,
    ) -> HandlerOutput {
        let SshConfigLoad {
            config,
            diagnostics,
        } = SshConfig::load_default();
        let mut output = expand_forward(context, value, &config);
        output.diagnostics.extend(diagnostics);
        output.dependencies.extend(config.files);
        output
    }
}

fn expand_forward(
    context: &ShortcutContext<'_>,
    value: &Value,
    ssh: &SshConfig,
) -> HandlerOutput {
    let mut output = HandlerOutput::default();
    let section: ForwardSection = match serde_yaml::from_value(value.clone()) {
        Ok(section) => section,
        Err(error) => {
            output.diagnostics.push(Diagnostic::error(
                context.span(),
                format!("invalid ShortcutMap Forward: {error}"),
                Vec::new(),
            ));
            return output;
        }
    };
    let (selection, exclude_hosts, exclude_users, apps) = match section {
        ForwardSection::Detailed(detailed) => (
            detailed.hosts,
            detailed.exclude_hosts,
            detailed.exclude_users,
            detailed.apps,
        ),
        ForwardSection::Apps(apps) => (None, Vec::new(), Vec::new(), apps),
    };

    if apps.is_empty() {
        output.diagnostics.push(Diagnostic::warning(
            context.span(),
            "ShortcutMap Forward has no apps",
            Vec::new(),
        ));
        return output;
    }

    let hosts = select_hosts(
        context,
        ssh,
        selection.as_ref(),
        &exclude_hosts,
        &exclude_users,
        &mut output.diagnostics,
    );
    if hosts.is_empty() {
        output.diagnostics.push(Diagnostic::warning(
            context.span(),
            "ShortcutMap Forward has no SSH hosts",
            Vec::new(),
        ));
        return output;
    }

    let mut generated = HashSet::new();
    for (app, spec) in &apps {
        if !valid_fragment(app) {
            output.diagnostics.push(Diagnostic::error(
                context.span(),
                format!("ShortcutMap Forward app `{app}` is not a valid alias fragment"),
                Vec::new(),
            ));
            continue;
        }
        let parts = spec.parts();
        if parts.remote_port == 0 || parts.local_port == 0 {
            output.diagnostics.push(Diagnostic::error(
                context.span(),
                format!("ShortcutMap Forward app `{app}` has an invalid port"),
                Vec::new(),
            ));
            continue;
        }
        if parts.bind.is_some_and(invalid_address) {
            output.diagnostics.push(Diagnostic::error(
                context.span(),
                format!("ShortcutMap Forward app `{app}` has an invalid bind address"),
                Vec::new(),
            ));
            continue;
        }
        if invalid_address(parts.remote_host) {
            output.diagnostics.push(Diagnostic::error(
                context.span(),
                format!("ShortcutMap Forward app `{app}` has an invalid remote host"),
                Vec::new(),
            ));
            continue;
        }

        for host in &hosts {
            let name = format!("Forward{}{}", host.alias_name, app);
            if !valid_generated_name(&name) {
                output.diagnostics.push(Diagnostic::error(
                    context.span(),
                    format!("ShortcutMap Forward generated an invalid alias `{name}`"),
                    Vec::new(),
                ));
                continue;
            }
            if !generated.insert(name.to_ascii_lowercase()) {
                output.diagnostics.push(Diagnostic::error(
                    context.span(),
                    format!("ShortcutMap Forward generated a duplicate alias `{name}`"),
                    Vec::new(),
                ));
                continue;
            }
            let forward = match parts.bind {
                Some(bind) => format!(
                    "{bind}:{}:{}:{}",
                    parts.local_port, parts.remote_host, parts.remote_port
                ),
                None => format!(
                    "{}:{}:{}",
                    parts.local_port, parts.remote_host, parts.remote_port
                ),
            };
            let body = format!(
                "ssh -N -o ExitOnForwardFailure=yes -L {} {}",
                quote_dsl(&forward),
                quote_dsl(&host.ssh_name)
            );
            output
                .definitions
                .push(definition(name, body, context.span()));
        }
    }
    output
}

fn select_hosts(
    context: &ShortcutContext<'_>,
    ssh: &SshConfig,
    selection: Option<&HostSelection>,
    exclude_hosts: &[String],
    exclude_users: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<SelectedHost> {
    let mut candidates = Vec::new();
    match selection {
        None => {
            for host in &ssh.hosts {
                candidates.push(SelectedHost {
                    ssh_name: host.ssh_name.clone(),
                    alias_name: host.alias_name.clone(),
                    user: host.user.clone(),
                });
            }
        }
        Some(HostSelection::Names(names)) => {
            for name in names {
                if let Some(host) = ssh
                    .hosts
                    .iter()
                    .find(|host| host.ssh_name.eq_ignore_ascii_case(name))
                {
                    candidates.push(SelectedHost {
                        ssh_name: host.ssh_name.clone(),
                        alias_name: host.alias_name.clone(),
                        user: host.user.clone(),
                    });
                } else if let Some(alias_name) = ssh_config::alias_fragment(name) {
                    candidates.push(SelectedHost {
                        ssh_name: name.clone(),
                        alias_name,
                        user: None,
                    });
                } else {
                    diagnostics.push(Diagnostic::error(
                        context.span(),
                        format!("ShortcutMap Forward host `{name}` is not a valid alias fragment"),
                        Vec::new(),
                    ));
                }
            }
        }
        Some(HostSelection::Aliases(aliases)) => {
            for (ssh_name, alias_name) in aliases {
                if !valid_fragment(alias_name) {
                    diagnostics.push(Diagnostic::error(
                        context.span(),
                        format!(
                            "ShortcutMap Forward host alias `{alias_name}` is not a valid alias fragment"
                        ),
                        Vec::new(),
                    ));
                    continue;
                }
                candidates.push(SelectedHost {
                    ssh_name: ssh_name.clone(),
                    alias_name: alias_name.clone(),
                    user: ssh.hosts
                        .iter()
                        .find(|host| host.ssh_name.eq_ignore_ascii_case(ssh_name))
                        .and_then(|host| host.user.clone()),
                });
            }
        }
    }

    let mut selected = Vec::new();
    let mut seen_hosts = HashSet::new();
    let mut alias_owners: BTreeMap<String, String> = BTreeMap::new();
    for host in candidates {
        if exclude_hosts.iter().any(|pattern| {
            host_matches(pattern, &host.ssh_name) || host_matches(pattern, &host.alias_name)
        }) {
            continue;
        }
        if host
            .user
            .as_deref()
            .is_some_and(|user| exclude_users.iter().any(|excluded| excluded == user))
        {
            continue;
        }
        if !valid_ssh_name(&host.ssh_name) {
            diagnostics.push(Diagnostic::error(
                context.span(),
                format!("ShortcutMap Forward SSH host `{}` is invalid", host.ssh_name),
                Vec::new(),
            ));
            continue;
        }
        if !seen_hosts.insert(host.ssh_name.to_ascii_lowercase()) {
            continue;
        }
        let alias_key = host.alias_name.to_ascii_lowercase();
        if let Some(existing) = alias_owners.get(&alias_key) {
            diagnostics.push(Diagnostic::error(
                context.span(),
                format!(
                    "ShortcutMap Forward host alias `{}` is ambiguous between `{existing}` and `{}`",
                    host.alias_name, host.ssh_name
                ),
                Vec::new(),
            ));
            continue;
        }
        alias_owners.insert(alias_key, host.ssh_name.clone());
        selected.push(host);
    }
    selected
}

fn host_matches(pattern: &str, value: &str) -> bool {
    ssh_config::wildcard_match(&pattern.to_ascii_lowercase(), &value.to_ascii_lowercase())
}

fn valid_fragment(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            })
}

fn valid_generated_name(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(
        characters.next(),
        Some(character) if character.is_ascii_alphabetic() || character == '_'
    ) && characters.all(|character| {
        character.is_ascii_alphanumeric() || character == '_' || character == '-'
    })
}

fn valid_ssh_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

fn invalid_address(value: &str) -> bool {
    value.is_empty()
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

static PATH_HANDLER: PathHandler = PathHandler;
static ALIASES_HANDLER: AliasesHandler = AliasesHandler;
static FORWARD_HANDLER: ForwardHandler = ForwardHandler;

fn handler_for(key: &str) -> Option<&'static dyn ShortcutHandler> {
    if PATH_VERBS.iter().any(|verb| verb.key == key) {
        return Some(&PATH_HANDLER);
    }
    match key {
        "aliases" => Some(&ALIASES_HANDLER),
        "Forward" => Some(&FORWARD_HANDLER),
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

    fn ssh_config(hosts: &[(&str, &str)]) -> SshConfig {
        SshConfig {
            hosts: hosts
                .iter()
                .map(|(ssh_name, alias_name)| crate::ssh_config::SshHost {
                    ssh_name: (*ssh_name).into(),
                    alias_name: (*alias_name).into(),
                    user: None,
                    source: PathBuf::from("config"),
                    line: 1,
                })
                .collect(),
            files: Vec::new(),
        }
    }

    fn ssh_config_with_users(hosts: &[(&str, &str, Option<&str>)]) -> SshConfig {
        SshConfig {
            hosts: hosts
                .iter()
                .map(|(ssh_name, alias_name, user)| crate::ssh_config::SshHost {
                    ssh_name: (*ssh_name).into(),
                    alias_name: (*alias_name).into(),
                    user: user.map(str::to_string),
                    source: PathBuf::from("config"),
                    line: 1,
                })
                .collect(),
            files: Vec::new(),
        }
    }

    fn forward_output(yaml: &str, ssh: &SshConfig) -> HandlerOutput {
        let directory = tempfile::tempdir().unwrap();
        let options = options(directory.path().join("alias"));
        let path = directory.path().join("ShortcutMap.yaml");
        let context = ShortcutContext {
            options: &options,
            path: &path,
        };
        let value: Value = serde_yaml::from_str(yaml).unwrap();
        expand_forward(&context, &value, ssh)
    }

    #[test]
    fn forward_short_form_expands_all_ssh_hosts() {
        let ssh = ssh_config(&[("mac", "Mac"), ("ecs", "Ecs")]);
        let output = forward_output("Syncthing: 8384\n", &ssh);
        let names = output
            .definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["ForwardMacSyncthing", "ForwardEcsSyncthing"]);
        assert_eq!(
            output.definitions[0].body,
            "ssh -N -o ExitOnForwardFailure=yes -L \"8384:127.0.0.1:8384\" \"mac\""
        );
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn forward_detailed_form_selects_hosts_and_bind_address() {
        let ssh = ssh_config(&[("mac", "Mac"), ("ecs", "Ecs")]);
        let output = forward_output(
            "hosts:\n  mac: Mac\n  ecs: ECS\napps:\n  Syncthing:\n    port: 8384\n    bind: 0.0.0.0\n",
            &ssh,
        );
        let names = output
            .definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["ForwardECSSyncthing", "ForwardMacSyncthing"]);
        let ecs = output
            .definitions
            .iter()
            .find(|definition| definition.name == "ForwardECSSyncthing")
            .unwrap();
        assert_eq!(
            ecs.body,
            "ssh -N -o ExitOnForwardFailure=yes -L \"0.0.0.0:8384:127.0.0.1:8384\" \"ecs\""
        );
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn forward_supports_different_local_and_remote_ports() {
        let ssh = ssh_config(&[("mac", "Mac")]);
        let output = forward_output(
            "apps:\n  Syncthing:\n    remote_port: 8384\n    local_port: 18384\n    remote_host: 192.168.1.10\n",
            &ssh,
        );
        assert_eq!(output.definitions.len(), 1);
        assert_eq!(output.definitions[0].name, "ForwardMacSyncthing");
        assert_eq!(
            output.definitions[0].body,
            "ssh -N -o ExitOnForwardFailure=yes -L \"18384:192.168.1.10:8384\" \"mac\""
        );
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn forward_excludes_hosts_by_pattern() {
        let ssh = ssh_config(&[("mac", "Mac"), ("ecs", "Ecs")]);
        let output = forward_output(
            "exclude_hosts: [\"e*\"]\napps:\n  Syncthing: 8384\n",
            &ssh,
        );
        assert_eq!(output.definitions.len(), 1);
        assert_eq!(output.definitions[0].name, "ForwardMacSyncthing");
    }

    #[test]
    fn forward_excludes_hosts_by_user() {
        let ssh = ssh_config_with_users(&[
            ("mac", "Mac", Some("macuser")),
            ("github.com", "GithubCom", Some("git")),
        ]);
        let output = forward_output(
            "exclude_users: [git]\napps:\n  Syncthing: 8384\n",
            &ssh,
        );
        assert_eq!(output.definitions.len(), 1);
        assert_eq!(output.definitions[0].name, "ForwardMacSyncthing");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn forward_rejects_zero_ports_and_ambiguous_aliases() {
        let ssh = ssh_config(&[("mac.local", "MacLocal"), ("mac_local", "MacLocal")]);
        let output = forward_output("Syncthing: 0\n", &ssh);
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("invalid port")));

        let ssh = ssh_config(&[("mac.local", "MacLocal"), ("mac_local", "MacLocal")]);
        let output = forward_output("Syncthing: 8384\n", &ssh);
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ambiguous")));
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
