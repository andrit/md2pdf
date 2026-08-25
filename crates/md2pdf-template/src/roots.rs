//! Where templates are looked for, and in what order.
//!
//! Three roots, merged, first match winning on a name collision:
//!
//! | | Where | Why |
//! |---|---|---|
//! | 1 | an explicit directory | how a test or a one-off run pins a location |
//! | 2 | the user's config directory | a person's own templates, surviving an upgrade |
//! | 3 | beside the binary | the shipped `github-print`, also the reference example |
//!
//! **Merged rather than first-wins**, so the shipped template is always available even when a
//! user has their own. A name collision resolves to the earlier root, which is what lets a
//! person copy `github-print`, edit it, and shadow the original — the workflow 3e exists for.
//!
//! ## Resolved by hand, deliberately
//!
//! **[measured]** neither `dirs` nor `directories` is in the offline registry, and `INV-1`
//! forbids fetching one. The rules are short and stable, and the failure mode of getting one
//! wrong is a directory that is simply not found — visible immediately rather than silently
//! wrong. Written as a pure function of the environment so all three platforms are testable
//! from any one of them.

use std::path::PathBuf;

/// The environment, as a value — so the resolver is a pure function and every platform's
/// rule can be tested from every platform.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub home: Option<String>,
    pub xdg_config_home: Option<String>,
    pub appdata: Option<String>,
}

impl Env {
    /// Read the real environment. The only impure part, and it does nothing else.
    pub fn current() -> Self {
        Self {
            home: std::env::var("HOME").ok(),
            xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
            appdata: std::env::var("APPDATA").ok(),
        }
    }
}

/// Which platform's convention to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
}

impl Platform {
    /// The one this binary was built for.
    pub const fn host() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// The user's md2pdf configuration directory, by platform convention.
///
/// `None` when the environment does not say where home is — a container with no `HOME`, a
/// service account. The caller falls back to the other roots rather than inventing a path.
pub fn config_dir(platform: Platform, env: &Env) -> Option<PathBuf> {
    match platform {
        // XDG first, then the specified default. `$XDG_CONFIG_HOME` is only honoured when
        // absolute — the spec says a relative value must be ignored, and honouring one would
        // resolve against the current directory, which for a batch run is arbitrary.
        Platform::Linux => match &env.xdg_config_home {
            Some(x) if PathBuf::from(x).is_absolute() => Some(PathBuf::from(x).join("md2pdf")),
            _ => env
                .home
                .as_ref()
                .map(|h| PathBuf::from(h).join(".config").join("md2pdf")),
        },
        Platform::MacOs => env.home.as_ref().map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("md2pdf")
        }),
        Platform::Windows => env
            .appdata
            .as_ref()
            .map(|a| PathBuf::from(a).join("md2pdf")),
    }
}

/// Every directory to search, in precedence order, skipping ones that cannot be named.
///
/// Existence is **not** checked here: this is the pure half, and a missing directory is a
/// discovery-time fact rather than a resolution-time one.
pub fn roots(explicit: Option<PathBuf>, beside_binary: Option<PathBuf>, env: &Env) -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.extend(explicit);
    out.extend(config_dir(Platform::host(), env).map(|c| c.join("templates")));
    out.extend(beside_binary);
    out
}

/// `templates/` next to the running executable — where the shipped one lives.
pub fn beside_binary() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|d| d.join("templates"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(home: &str) -> Env {
        Env {
            home: Some(home.into()),
            ..Default::default()
        }
    }

    #[test]
    fn linux_uses_dot_config_by_default() {
        let got = config_dir(Platform::Linux, &env("/home/ada"));
        assert_eq!(got, Some(PathBuf::from("/home/ada/.config/md2pdf")));
    }

    #[test]
    fn linux_honours_xdg_config_home() {
        let e = Env {
            home: Some("/home/ada".into()),
            xdg_config_home: Some("/elsewhere/cfg".into()),
            ..Default::default()
        };
        assert_eq!(
            config_dir(Platform::Linux, &e),
            Some(PathBuf::from("/elsewhere/cfg/md2pdf"))
        );
    }

    #[test]
    fn a_relative_xdg_config_home_is_ignored() {
        // The XDG spec says a relative value is invalid. Honouring one would resolve
        // against the current directory, which for a batch run is wherever the user
        // happened to be — a template appearing or vanishing with `cd` is the kind of
        // bug that takes an afternoon to believe.
        let e = Env {
            home: Some("/home/ada".into()),
            xdg_config_home: Some("relative/cfg".into()),
            ..Default::default()
        };
        assert_eq!(
            config_dir(Platform::Linux, &e),
            Some(PathBuf::from("/home/ada/.config/md2pdf"))
        );
    }

    #[test]
    fn macos_uses_application_support() {
        assert_eq!(
            config_dir(Platform::MacOs, &env("/Users/ada")),
            Some(PathBuf::from(
                "/Users/ada/Library/Application Support/md2pdf"
            ))
        );
    }

    #[test]
    fn windows_uses_appdata_and_ignores_home() {
        let e = Env {
            home: Some("/should/be/ignored".into()),
            appdata: Some("C:\\Users\\Ada\\AppData\\Roaming".into()),
            ..Default::default()
        };
        let got = config_dir(Platform::Windows, &e).expect("APPDATA is set");

        // Asserted structurally, not as a literal string. `PathBuf::join` uses the
        // *running* platform's separator, so on Linux this produces `…Roaming/md2pdf`
        // with a forward slash — which is correct behaviour being tested from the wrong
        // machine, not a bug. Windows accepts either separator, and what actually has to
        // hold is that APPDATA is the parent and `md2pdf` is the leaf.
        assert_eq!(got.file_name().unwrap(), "md2pdf");
        assert!(
            got.to_string_lossy()
                .starts_with("C:\\Users\\Ada\\AppData\\Roaming"),
            "APPDATA is not the parent: {got:?}"
        );
        assert!(
            !got.to_string_lossy().contains("should/be/ignored"),
            "HOME leaked into the Windows path: {got:?}"
        );
    }

    #[test]
    fn without_an_environment_there_is_no_config_directory() {
        // Not a panic and not a guess: a container with no HOME still has the shipped
        // templates beside the binary.
        for p in [Platform::Linux, Platform::MacOs, Platform::Windows] {
            assert_eq!(config_dir(p, &Env::default()), None);
        }
    }

    #[test]
    fn explicit_comes_first_and_shipped_comes_last() {
        let got = roots(
            Some(PathBuf::from("/pinned")),
            Some(PathBuf::from("/app/templates")),
            &env("/home/ada"),
        );
        assert_eq!(got.first(), Some(&PathBuf::from("/pinned")));
        assert_eq!(got.last(), Some(&PathBuf::from("/app/templates")));
        assert!(got.len() >= 2);
    }
}
