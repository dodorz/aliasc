use serde::Deserialize;
use std::{
    env,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

const RELEASES_URL: &str = "https://api.github.com/repos/dodorz/aliasc/releases/latest";

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug)]
pub struct UpdateResult {
    pub path: PathBuf,
    pub version: String,
    pub already_current: bool,
    pub replacement_pending: bool,
}

pub fn update(force: bool, requested_location: Option<PathBuf>) -> Result<UpdateResult, String> {
    let target = requested_location
        .or_else(|| default_install_path().ok())
        .ok_or_else(|| "cannot determine the default install location; pass --location".to_string())?;
    let target = normalize_target(&target)?;
    let release = fetch_release()?;
    let version = normalize_version(&release.tag_name);

    if !force && !is_newer(&version, env!("CARGO_PKG_VERSION")) {
        return Ok(UpdateResult {
            path: target,
            version,
            already_current: true,
            replacement_pending: false,
        });
    }

    let asset_name = asset_name()?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| {
            format!(
                "release {} has no asset for this target ({asset_name})",
                release.tag_name
            )
        })?;
    let bytes = fetch_asset(&asset.browser_download_url)?;
    let replacement_pending = install(&target, &bytes)?;

    Ok(UpdateResult {
        path: target,
        version,
        already_current: false,
        replacement_pending,
    })
}

fn fetch_release() -> Result<Release, String> {
    let response = ureq::get(RELEASES_URL)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", concat!("aliasc/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|error| format!("failed to query GitHub releases: {error}"))?;
    let body = response
        .into_string()
        .map_err(|error| format!("failed to read GitHub release response: {error}"))?;
    serde_json::from_str(&body).map_err(|error| format!("invalid GitHub release response: {error}"))
}

fn fetch_asset(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .set("Accept", "application/octet-stream")
        .set("User-Agent", concat!("aliasc/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|error| format!("failed to download release asset: {error}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read release asset: {error}"))?;
    if bytes.is_empty() {
        return Err("downloaded release asset is empty".to_string());
    }
    Ok(bytes)
}

fn asset_name() -> Result<String, String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let target = match (os, arch) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "x86") => "i686-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("android", "aarch64") => "aarch64-linux-android",
        _ => {
            return Err(format!(
                "no published aliasc asset for this platform ({os}/{arch})"
            ))
        }
    };
    Ok(if os == "windows" {
        format!("aliasc-{target}.exe")
    } else {
        format!("aliasc-{target}")
    })
}

fn default_install_path() -> Result<PathBuf, String> {
    if env::consts::OS == "windows" {
        let profile = env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .ok_or_else(|| "USERPROFILE is not set".to_string())?;
        return Ok(PathBuf::from(profile).join("Tools").join("bin").join("aliasc.exe"));
    }

    if let Some(path) = env::var_os("ALIASC_BIN").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if let Some(path) = find_on_path() {
        return Ok(path);
    }

    if let Ok(path) = env::current_exe() {
        if path.file_name().is_some_and(|name| name == "aliasc") {
            return Ok(path);
        }
    }

    let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".local").join("bin").join("aliasc"))
}

fn find_on_path() -> Option<PathBuf> {
    let name = if env::consts::OS == "windows" {
        "aliasc.exe"
    } else {
        "aliasc"
    };
    env::split_paths(&env::var_os("PATH")?).find_map(|directory| {
        let candidate = directory.join(name);
        if candidate.is_file() {
            fs::canonicalize(&candidate).ok().or(Some(candidate))
        } else {
            None
        }
    })
}

fn normalize_target(path: &Path) -> Result<PathBuf, String> {
    if path.file_name().is_none() {
        return Err(format!("install location is not a file path: {}", path.display()));
    }
    if env::consts::OS == "windows" && path.extension().is_none() {
        Ok(path.with_extension("exe"))
    } else {
        Ok(path.to_path_buf())
    }
}

fn install(target: &Path, bytes: &[u8]) -> Result<bool, String> {
    let parent = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create install directory {}: {error}", parent.display()))?;
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid install location: {}", target.display()))?;
    let temporary = parent.join(format!(".{file_name}.{}.download", std::process::id()));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write temporary download {}: {error}", temporary.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&temporary)
            .map_err(|error| format!("cannot inspect temporary download: {error}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&temporary, permissions)
            .map_err(|error| format!("cannot make downloaded aliasc executable: {error}"))?;
    }

    #[cfg(windows)]
    if is_current_executable(target) {
        let script = format!(
            "$ErrorActionPreference='Stop'; Start-Sleep -Milliseconds 300; Move-Item -LiteralPath '{}' -Destination '{}' -Force",
            powershell_quote(&temporary),
            powershell_quote(target)
        );
        Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .spawn()
            .map_err(|error| format!("cannot schedule Windows executable replacement: {error}"))?;
        return Ok(true);
    }

    #[cfg(windows)]
    if target.exists() {
        fs::remove_file(target)
            .map_err(|error| format!("cannot replace existing aliasc {}: {error}", target.display()))?;
    }
    fs::rename(&temporary, target).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("cannot install aliasc at {}: {error}", target.display())
    })?;
    Ok(false)
}

#[cfg(windows)]
fn is_current_executable(target: &Path) -> bool {
    let Ok(current) = env::current_exe() else { return false };
    fs::canonicalize(current).ok() == fs::canonicalize(target).ok()
}

#[cfg(windows)]
fn powershell_quote(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn normalize_version(version: &str) -> String {
    version
        .trim()
        .trim_start_matches('v')
        .split('-')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn is_newer(remote: &str, local: &str) -> bool {
    version_parts(remote) > version_parts(&normalize_version(local))
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_version_is_compared_without_a_v_prefix() {
        assert!(is_newer("1.2.0", "0.1.0"));
        assert!(is_newer("1.2.0", "v1.1.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn windows_asset_names_include_exe() {
        if env::consts::OS == "windows" {
            assert!(asset_name().unwrap().ends_with(".exe"));
        }
    }
}
