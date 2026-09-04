use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=VERSION");

    let version = env::var("VERSION")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION is set by Cargo"));

    if !is_release_version(&version) {
        panic!("VERSION must be MAJOR.MINOR.PATCH, got `{}`", version);
    }

    println!("cargo:rustc-env=ALIASC_VERSION={}", version);
}

fn is_release_version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| !part.is_empty() && part.chars().all(|character| character.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::is_release_version;

    #[test]
    fn accepts_plain_semver() {
        assert!(is_release_version("0.0.7"));
    }

    #[test]
    fn rejects_tag_prefixes_and_other_versions() {
        assert!(!is_release_version("v0.0.7"));
        assert!(!is_release_version("0.0"));
        assert!(!is_release_version("0.0.7-pre1"));
    }
}
