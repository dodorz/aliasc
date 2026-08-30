use clap::ValueEnum;
use serde::Serialize;
use std::{env, fs};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Shell { #[default] Posix, Bash, Zsh, Fish, Nu, Powershell, Pwsh, Cmd }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum PlatformArg { #[default] Auto, Linux, Macos, Windows }
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform { Linux, Macos, Windows }

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub enum DistroArg { #[default] Auto, None, Name(String) }
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub enum EnvironmentArg { #[default] Auto, Name(String) }
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub enum Distro { #[default] None, Ubuntu, Debian, Fedora, Arch, Other(String) }
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub enum Environment { #[default] None, Msys2, GitBash, Wsl, Other(String) }

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Context { pub shell: Shell, pub platform: Platform, pub distro: Distro, pub environment: Environment }
impl Default for Context { fn default() -> Self { Self { shell: Shell::Posix, platform: host_platform(), distro: Distro::None, environment: Environment::None } } }

impl FromStr for DistroArg { type Err = String; fn from_str(s:&str)->Result<Self,Self::Err>{Ok(match s.to_ascii_lowercase().as_str(){"auto"=>Self::Auto,"none"=>Self::None,_=>Self::Name(s.to_string())})} }
impl FromStr for EnvironmentArg { type Err = String; fn from_str(s:&str)->Result<Self,Self::Err>{Ok(if s.eq_ignore_ascii_case("auto"){Self::Auto}else{Self::Name(s.to_string())})} }

pub fn host_platform() -> Platform {
    match env::consts::OS { "windows" => Platform::Windows, "macos" => Platform::Macos, _ => Platform::Linux }
}

impl Context {
    pub fn resolve(shell: Shell, platform: PlatformArg, distro: DistroArg, environment: EnvironmentArg) -> Self {
        let actual_platform = match platform { PlatformArg::Auto => host_platform(), PlatformArg::Linux => Platform::Linux, PlatformArg::Macos => Platform::Macos, PlatformArg::Windows => Platform::Windows };
        let auto_distro = if actual_platform == Platform::Linux && platform == PlatformArg::Auto { detect_distro() } else { Distro::None };
        let auto_environment = if actual_platform == Platform::Windows && platform == PlatformArg::Auto { detect_environment() } else { Environment::None };
        Self { shell, platform: actual_platform, distro: match distro { DistroArg::Auto => auto_distro, DistroArg::None => Distro::None, DistroArg::Name(n) => parse_distro(&n) }, environment: match environment { EnvironmentArg::Auto => auto_environment, EnvironmentArg::Name(n) => parse_environment(&n) } }
    }
    pub fn section_active(&self, name: &str) -> bool {
        let n = name.to_ascii_lowercase();
        if n == "common" { return true; }
        match self.platform {
            Platform::Linux => n == "unix" || distro_name(&self.distro).is_some_and(|v| v == n),
            Platform::Macos => n == "unix" || n == "macos",
            Platform::Windows => n == "windows" || environment_name(&self.environment).is_some_and(|v| v == n),
        }
    }
}

fn parse_distro(s: &str) -> Distro { match s.to_ascii_lowercase().as_str() { "none" => Distro::None, "ubuntu" => Distro::Ubuntu, "debian" => Distro::Debian, "fedora" => Distro::Fedora, "arch" => Distro::Arch, _ => Distro::Other(s.to_string()) } }
fn parse_environment(s: &str) -> Environment { match s.to_ascii_lowercase().as_str() { "none" => Environment::None, "msys2" => Environment::Msys2, "gitbash" | "git-bash" => Environment::GitBash, "wsl" => Environment::Wsl, _ => Environment::Other(s.to_string()) } }
fn distro_name(v: &Distro) -> Option<String> { Some(match v { Distro::None => return None, Distro::Ubuntu => "ubuntu".into(), Distro::Debian => "debian".into(), Distro::Fedora => "fedora".into(), Distro::Arch => "arch".into(), Distro::Other(x) => x.to_ascii_lowercase() }) }
fn environment_name(v: &Environment) -> Option<String> { Some(match v { Environment::None => return None, Environment::Msys2 => "msys2".into(), Environment::GitBash => "gitbash".into(), Environment::Wsl => "wsl".into(), Environment::Other(x) => x.to_ascii_lowercase() }) }
fn detect_distro() -> Distro {
    let Ok(text) = fs::read_to_string("/etc/os-release") else { return Distro::None };
    let mut identifiers = Vec::new();
    for key in ["ID=", "ID_LIKE="] {
        if let Some(value) = text.lines().find_map(|line| line.strip_prefix(key)) {
            identifiers.extend(value.trim_matches('"').split_whitespace().map(str::to_ascii_lowercase));
        }
    }
    for name in ["ubuntu", "debian", "fedora", "arch"] {
        if identifiers.iter().any(|id| id == name) { return parse_distro(name); }
    }
    Distro::None
}
fn detect_environment() -> Environment {
    if env::var_os("MSYSTEM").is_some() || env::var_os("MSYS2_PATH_TYPE").is_some() { Environment::Msys2 }
    else if env::var_os("WSL_DISTRO_NAME").is_some() { Environment::Wsl }
    else if env::var_os("GIT_INSTALL_ROOT").is_some() { Environment::GitBash } else { Environment::None }
}

pub fn known_section(name: &str) -> bool { matches!(name.to_ascii_lowercase().as_str(), "common"|"windows"|"unix"|"macos"|"ubuntu"|"debian"|"fedora"|"arch"|"msys2"|"gitbash"|"wsl") }
pub fn shell_section(name: &str) -> bool { matches!(name.to_ascii_lowercase().as_str(), "bash"|"zsh"|"fish"|"nu"|"powershell"|"pwsh"|"cmd"|"posix") }
