//! Command-line argument parsing and security validation for Shadhin Browser

use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

/// Parsed command-line configuration options
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CliOptions {
    pub show_version: bool,
    pub show_help: bool,
    pub new_window: bool,
    pub private_window: bool,
    pub profile_dir: Option<PathBuf>,
    pub urls: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CliError {
    #[error("Missing argument for option: {0}")]
    MissingArgument(String),
    #[error("Profile directory must be an absolute path: {0}")]
    ProfileDirNotAbsolute(String),
    #[error("Profile directory cannot be world-writable: {0}")]
    ProfileDirWorldWritable(String),
    #[error("Failed to create profile directory '{0}': {1}")]
    ProfileDirCreationFailed(String, String),
    #[error("Unknown option: {0}")]
    UnknownOption(String),
}

/// Parses and validates command-line arguments.
///
/// Skips the binary name (first argument) if present.
pub fn parse_cli_args<I, S>(args: I) -> Result<CliOptions, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let raw_items: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    let mut options = CliOptions::default();

    // If invoked as standard CLI, the first argument is typically the executable name.
    // If it does not start with '-' and matches the binary name or path, skip it.
    let mut iter = raw_items.into_iter().peekable();
    if let Some(first) = iter.peek()
        && !first.starts_with('-')
        && (first.ends_with("shadhin-browser") || first.contains('/'))
    {
        iter.next();
    }

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--version" | "-v" => {
                options.show_version = true;
            }
            "--help" | "-h" => {
                options.show_help = true;
            }
            "--new-window" => {
                options.new_window = true;
            }
            "--private-window" => {
                options.private_window = true;
            }
            "--new-tab" => {
                if let Some(url) = iter.next() {
                    options.urls.push(url);
                } else {
                    return Err(CliError::MissingArgument("--new-tab".to_string()));
                }
            }
            "--profile-dir" => {
                if let Some(dir_str) = iter.next() {
                    let path = validate_and_prepare_profile_dir(&dir_str)?;
                    options.profile_dir = Some(path);
                } else {
                    return Err(CliError::MissingArgument("--profile-dir".to_string()));
                }
            }
            other if other.starts_with("--profile-dir=") => {
                let dir_str = &other["--profile-dir=".len()..];
                let path = validate_and_prepare_profile_dir(dir_str)?;
                options.profile_dir = Some(path);
            }
            other if other.starts_with("--new-tab=") => {
                let url = &other["--new-tab=".len()..];
                options.urls.push(url.to_string());
            }
            other if other.starts_with('-') => {
                return Err(CliError::UnknownOption(other.to_string()));
            }
            positional_url => {
                // Desktop launchers and command line pass URLs as positional arguments (%U)
                if !positional_url.is_empty() {
                    options.urls.push(positional_url.to_string());
                }
            }
        }
    }

    Ok(options)
}

/// Validates profile directory security:
/// 1. Must be an absolute path.
/// 2. Must reject world-writable permissions if it already exists.
/// 3. If it does not exist, creates it with strict 0700 permissions.
fn validate_and_prepare_profile_dir(path_str: &str) -> Result<PathBuf, CliError> {
    let path = PathBuf::from(path_str);
    if !path.is_absolute() {
        return Err(CliError::ProfileDirNotAbsolute(path_str.to_string()));
    }

    if path.exists() {
        #[cfg(unix)]
        {
            if let Ok(metadata) = fs::metadata(&path) {
                let mode = metadata.permissions().mode();
                // Reject world-writable (other write bit 0o002)
                if mode & 0o002 != 0 {
                    return Err(CliError::ProfileDirWorldWritable(path_str.to_string()));
                }
            }
        }
    } else {
        #[cfg(unix)]
        {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(&path).map_err(|e| {
                CliError::ProfileDirCreationFailed(path_str.to_string(), e.to_string())
            })?;
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(&path).map_err(|e| {
                CliError::ProfileDirCreationFailed(path_str.to_string(), e.to_string())
            })?;
        }
    }

    Ok(path)
}

/// Formats the CLI help text
pub fn format_help_text(app_name: &str, app_version: &str) -> String {
    format!(
        "{app_name} {app_version}\n\
         Fast · Private · Open — Native Linux web browser\n\n\
         Usage: shadhin-browser [OPTIONS] [URL...]\n\n\
         Arguments:\n\
           [URL...]                 URLs to open in browser tabs\n\n\
         Options:\n\
           -h, --help               Print this help message and exit\n\
           -v, --version            Print version information and exit\n\
           --new-window             Open a new browser window\n\
           --private-window         Open a new private browsing window\n\
           --new-tab <URL>          Open URL in a new tab\n\
           --profile-dir <PATH>     Specify an isolated profile directory (must be absolute, 0700)\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_version_and_help() {
        let opts = parse_cli_args(["shadhin-browser", "--version"]).unwrap();
        assert!(opts.show_version);
        assert!(!opts.show_help);

        let opts = parse_cli_args(["shadhin-browser", "-h"]).unwrap();
        assert!(opts.show_help);
        assert!(!opts.show_version);
    }

    #[test]
    fn test_cli_new_window_and_private() {
        let opts = parse_cli_args(["shadhin-browser", "--new-window"]).unwrap();
        assert!(opts.new_window);
        assert!(!opts.private_window);

        let opts = parse_cli_args(["shadhin-browser", "--private-window"]).unwrap();
        assert!(opts.private_window);
        assert!(!opts.new_window);
    }

    #[test]
    fn test_cli_positional_urls_and_new_tab() {
        let opts = parse_cli_args([
            "shadhin-browser",
            "https://example.com",
            "--new-tab",
            "https://wikipedia.org",
            "about:blank",
        ])
        .unwrap();

        assert_eq!(
            opts.urls,
            vec![
                "https://example.com".to_string(),
                "https://wikipedia.org".to_string(),
                "about:blank".to_string(),
            ]
        );
    }

    #[test]
    fn test_cli_rejects_relative_profile_dir() {
        let err =
            parse_cli_args(["shadhin-browser", "--profile-dir", "relative/path"]).unwrap_err();
        assert_eq!(
            err,
            CliError::ProfileDirNotAbsolute("relative/path".to_string())
        );
    }

    #[test]
    fn test_cli_valid_profile_dir_creates_with_0700() {
        let temp_dir = std::env::temp_dir().join(format!("test_prof_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        let path_str = temp_dir.to_str().unwrap().to_string();
        let opts = parse_cli_args(["shadhin-browser", "--profile-dir", &path_str]).unwrap();
        assert_eq!(opts.profile_dir, Some(temp_dir.clone()));

        #[cfg(unix)]
        {
            let mode = fs::metadata(&temp_dir).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "Profile dir must be created with 0700");
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cli_rejects_world_writable_profile_dir() {
        let temp_dir = std::env::temp_dir().join(format!("test_prof_ww_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&temp_dir).unwrap().permissions();
            perms.set_mode(0o777); // World-writable
            fs::set_permissions(&temp_dir, perms).unwrap();

            let path_str = temp_dir.to_str().unwrap().to_string();
            let err = parse_cli_args(["shadhin-browser", "--profile-dir", &path_str]).unwrap_err();
            assert_eq!(err, CliError::ProfileDirWorldWritable(path_str));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cli_missing_argument_errors() {
        assert_eq!(
            parse_cli_args(["shadhin-browser", "--profile-dir"]).unwrap_err(),
            CliError::MissingArgument("--profile-dir".to_string())
        );
        assert_eq!(
            parse_cli_args(["shadhin-browser", "--new-tab"]).unwrap_err(),
            CliError::MissingArgument("--new-tab".to_string())
        );
    }

    #[test]
    fn test_cli_unknown_option_error() {
        assert_eq!(
            parse_cli_args(["shadhin-browser", "--unsupported-flag"]).unwrap_err(),
            CliError::UnknownOption("--unsupported-flag".to_string())
        );
    }
}
