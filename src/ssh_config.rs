//! Minimal SSH client configuration reader.
//!
//! `Forward` aliases need only the concrete `Host` aliases declared by
//! `~/.ssh/config` (and its `Include` files). The generated command still calls
//! `ssh <alias>`, so `HostName`, `User`, `Port`, `ProxyJump`, and other options
//! remain the responsibility of SSH at runtime.

use crate::dsl::{Diagnostic, SourceSpan};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default)]
pub struct SshConfig {
    pub hosts: Vec<SshHost>,
    pub files: Vec<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshHost {
    pub ssh_name: String,
    pub alias_name: String,
    pub user: Option<String>,
    pub source: PathBuf,
    pub line: usize,
}

#[derive(Default)]
pub struct SshConfigLoad {
    pub config: SshConfig,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn default_path() -> PathBuf {
    if let Some(path) = env::var_os("ALIASC_SSH_CONFIG").filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ssh")
        .join("config")
}

impl SshConfig {
    pub fn load_default() -> SshConfigLoad {
        Self::load(&default_path())
    }

    pub fn load(path: &Path) -> SshConfigLoad {
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut parser = Parser {
            root,
            home,
            hosts: Vec::new(),
            seen_hosts: HashSet::new(),
            user_rules: Vec::new(),
            current_patterns: vec!["*".into()],
            files: Vec::new(),
            seen_files: HashSet::new(),
            diagnostics: Vec::new(),
        };
        parser.visit(path, &mut Vec::new());
        parser.apply_users();
        SshConfigLoad {
            config: SshConfig {
                hosts: parser.hosts,
                files: parser.files,
            },
            diagnostics: parser.diagnostics,
        }
    }
}

struct Parser {
    root: PathBuf,
    home: PathBuf,
    hosts: Vec<SshHost>,
    seen_hosts: HashSet<String>,
    user_rules: Vec<UserRule>,
    current_patterns: Vec<String>,
    files: Vec<PathBuf>,
    seen_files: HashSet<PathBuf>,
    diagnostics: Vec<Diagnostic>,
}

struct UserRule {
    patterns: Vec<String>,
    user: String,
}

impl Parser {
    fn visit(&mut self, path: &Path, stack: &mut Vec<PathBuf>) {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        };
        let key = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if stack.contains(&key) {
            self.diagnostics.push(Diagnostic::error(
                SourceSpan {
                    file: key,
                    line: 1,
                    column: 1,
                },
                "SSH config include cycle detected",
                Vec::new(),
            ));
            return;
        }
        if !self.seen_files.insert(key.clone()) {
            return;
        }
        self.files.push(key.clone());
        let Ok(text) = fs::read_to_string(&key) else {
            return;
        };
        stack.push(key.clone());
        for (line_index, line) in text.lines().enumerate() {
            let tokens = tokenize(line);
            let Some(keyword) = tokens.first() else {
                continue;
            };
            if keyword.eq_ignore_ascii_case("Host") {
                self.current_patterns = tokens[1..].to_vec();
                for pattern in &tokens[1..] {
                    if !is_concrete_host(pattern) {
                        continue;
                    }
                    let Some(alias_name) = alias_fragment(pattern) else {
                        self.diagnostics.push(Diagnostic::warning(
                            SourceSpan {
                                file: key.clone(),
                                line: line_index + 1,
                                column: 1,
                            },
                            format!("cannot derive an alias name for SSH host `{pattern}`"),
                            Vec::new(),
                        ));
                        continue;
                    };
                    if !self.seen_hosts.insert(pattern.to_ascii_lowercase()) {
                        continue;
                    }
                    self.hosts.push(SshHost {
                        ssh_name: pattern.clone(),
                        alias_name,
                        user: None,
                        source: key.clone(),
                        line: line_index + 1,
                    });
                }
            } else if keyword.eq_ignore_ascii_case("User") {
                if let Some(user) = tokens.get(1)
                    && !self.current_patterns.is_empty()
                {
                    self.user_rules.push(UserRule {
                        patterns: self.current_patterns.clone(),
                        user: user.clone(),
                    });
                }
            } else if keyword.eq_ignore_ascii_case("Match") {
                // Conditional Match blocks are outside the small parser's
                // supported subset. Do not attribute their options to the
                // preceding Host block.
                self.current_patterns.clear();
            } else if keyword.eq_ignore_ascii_case("Include") {
                for include in &tokens[1..] {
                    let pattern = resolve_include(include, &self.root, &self.home);
                    let mut matches = expand_glob(&pattern);
                    if matches.is_empty() {
                        matches.push(pattern);
                    }
                    for include_path in matches {
                        self.visit(&include_path, stack);
                    }
                }
            }
        }
        stack.pop();
    }

    fn apply_users(&mut self) {
        let Self {
            hosts, user_rules, ..
        } = self;
        for host in hosts {
            host.user = user_rules
                .iter()
                .find(|rule| host_block_matches(&rule.patterns, &host.ssh_name))
                .map(|rule| rule.user.clone());
        }
    }
}

fn host_block_matches(patterns: &[String], host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let mut matched = false;
    for pattern in patterns {
        let pattern = pattern.to_ascii_lowercase();
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard_match(negated, &host) {
                return false;
            }
        } else if wildcard_match(&pattern, &host) {
            matched = true;
        }
    }
    matched
}

fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut present = false;
    for character in line.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            present = true;
            continue;
        }
        if quoted && character == '\\' {
            escaped = true;
            continue;
        }
        match character {
            '"' => {
                quoted = !quoted;
                present = true;
            }
            '#' if !quoted => break,
            character if character.is_whitespace() && !quoted => {
                if present {
                    tokens.push(std::mem::take(&mut current));
                    present = false;
                }
            }
            character => {
                current.push(character);
                present = true;
            }
        }
    }
    if present {
        tokens.push(current);
    }
    tokens
}

fn is_concrete_host(pattern: &str) -> bool {
    !pattern.is_empty()
        && !pattern.starts_with('!')
        && !pattern.contains('*')
        && !pattern.contains('?')
}

pub(crate) fn alias_fragment(value: &str) -> Option<String> {
    let mut result = String::new();
    for segment in value.split(|character: char| {
        !character.is_ascii_alphanumeric() && character != '_'
    }) {
        let mut characters = segment.chars();
        let Some(first) = characters.next() else {
            continue;
        };
        result.push(first.to_ascii_uppercase());
        result.extend(characters);
    }
    if result.is_empty() {
        return None;
    }
    if result
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        result.insert_str(0, "Host");
    }
    Some(result)
}

fn resolve_include(value: &str, root: &Path, home: &Path) -> PathBuf {
    let path = if value == "~" {
        home.to_path_buf()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home.join(rest)
    } else if let Some(rest) = value.strip_prefix("~\\") {
        home.join(rest)
    } else {
        PathBuf::from(value)
    };
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn expand_glob(pattern: &Path) -> Vec<PathBuf> {
    if !pattern
        .to_string_lossy()
        .chars()
        .any(|character| character == '*' || character == '?')
    {
        return vec![pattern.to_path_buf()];
    }
    let mut results = vec![PathBuf::new()];
    for component in pattern.components() {
        let name = component.as_os_str().to_string_lossy();
        if name.contains('*') || name.contains('?') {
            let mut next = Vec::new();
            for base in &results {
                let directory = if base.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    base.as_path()
                };
                let Ok(entries) = fs::read_dir(directory) else {
                    continue;
                };
                let mut names = entries
                    .flatten()
                    .map(|entry| entry.file_name())
                    .collect::<Vec<_>>();
                names.sort();
                for file_name in names {
                    if wildcard_match(&name, &file_name.to_string_lossy()) {
                        let mut path = base.clone();
                        path.push(file_name);
                        next.push(path);
                    }
                }
            }
            results = next;
        } else {
            for path in &mut results {
                path.push(component.as_os_str());
            }
        }
        if results.is_empty() {
            break;
        }
    }
    results.sort();
    results
}

pub(crate) fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    let mut table = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    table[0][0] = true;
    for index in 1..=pattern.len() {
        if pattern[index - 1] == '*' {
            table[index][0] = table[index - 1][0];
        }
    }
    for pattern_index in 1..=pattern.len() {
        for text_index in 1..=text.len() {
            table[pattern_index][text_index] = match pattern[pattern_index - 1] {
                '*' => {
                    table[pattern_index - 1][text_index]
                        || table[pattern_index][text_index - 1]
                }
                '?' => table[pattern_index - 1][text_index - 1],
                character => {
                    table[pattern_index - 1][text_index - 1]
                        && character == text[text_index - 1]
                }
            };
        }
    }
    table[pattern.len()][text.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concrete_hosts_skip_patterns_and_include_files_are_loaded() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("config.d")).unwrap();
        fs::write(
            root.join("config"),
            "Host *\nHost mac\nHost ecs !bad\n    User ecsuser\nHost github.com\n    User git\nInclude config.d/*.conf\n",
        )
        .unwrap();
        fs::write(root.join("config.d/extra.conf"), "Host newmac\n").unwrap();

        let loaded = SshConfig::load(&root.join("config"));
        let names = loaded
            .config
            .hosts
            .iter()
            .map(|host| host.ssh_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["mac", "ecs", "github.com", "newmac"]);
        assert_eq!(loaded.config.hosts[0].alias_name, "Mac");
        assert_eq!(loaded.config.hosts[0].user, None);
        assert_eq!(loaded.config.hosts[1].user.as_deref(), Some("ecsuser"));
        assert_eq!(loaded.config.hosts[2].alias_name, "GithubCom");
        assert_eq!(loaded.config.hosts[2].user.as_deref(), Some("git"));
        assert_eq!(loaded.config.files.len(), 2);
        assert!(loaded.diagnostics.is_empty());
    }

    #[test]
    fn first_matching_user_rule_wins() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("config"),
            "Host *\n    User global\nHost mac\n    User macuser\n",
        )
        .unwrap();
        let loaded = SshConfig::load(&root.join("config"));
        assert_eq!(loaded.config.hosts.len(), 1);
        assert_eq!(loaded.config.hosts[0].user.as_deref(), Some("global"));
    }

    #[test]
    fn negated_host_patterns_do_not_receive_user_rules() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("config"),
            "Host mac !mactail\n    User macuser\nHost mactail\n",
        )
        .unwrap();
        let loaded = SshConfig::load(&root.join("config"));
        assert_eq!(loaded.config.hosts[0].user.as_deref(), Some("macuser"));
        assert_eq!(loaded.config.hosts[1].user, None);
    }

    #[test]
    fn include_cycles_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("config"), "Include other\n").unwrap();
        fs::write(root.join("other"), "Include config\n").unwrap();
        let loaded = SshConfig::load(&root.join("config"));
        assert!(loaded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("include cycle")));
    }

    #[test]
    fn wildcard_match_supports_star_and_question_mark() {
        assert!(wildcard_match("mac*", "macbook"));
        assert!(wildcard_match("m?c", "mac"));
        assert!(!wildcard_match("mac", "macbook"));
    }
}
