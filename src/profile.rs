use crate::config::Config;
use crate::error::{Result, map_io_error};
use std::fs;

/// Reads an ordered package list from a named profile.
///
/// A profile lives at `<STAU_DIR>/profiles/<name>`. Blank lines and lines
/// starting with `#` are ignored; every other line names one package. The
/// order of the lines is preserved, which is what lets a profile express
/// dependency order for hooks.
pub fn read_profile(config: &Config, name: &str) -> Result<Vec<String>> {
    Config::validate_package_name(name)?;

    let path = config.stau_dir.join("profiles").join(name);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(crate::error::StauError::ProfileNotFound(name.to_string()));
        }
        Err(e) => {
            return Err(map_io_error(
                e,
                format!("Cannot read profile: {}", path.display()),
            ));
        }
    };

    let mut packages = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        Config::validate_package_name(line)?;
        packages.push(line.to_string());
    }

    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_profile_preserves_order_and_skips_comments() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let profiles_dir = stau_dir.join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();

        fs::write(
            profiles_dir.join("headless"),
            "# ordering matters\n\nmise\nsesh\n\ngit\n",
        )
        .unwrap();

        let config = Config {
            stau_dir,
            default_target: temp_dir.path().to_path_buf(),
        };

        let packages = read_profile(&config, "headless").unwrap();
        assert_eq!(packages, vec!["mise", "sesh", "git"]);
    }

    #[test]
    fn test_read_profile_rejects_bad_package_name() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let profiles_dir = stau_dir.join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();

        fs::write(profiles_dir.join("bad"), "../evil\n").unwrap();

        let config = Config {
            stau_dir,
            default_target: temp_dir.path().to_path_buf(),
        };

        assert!(read_profile(&config, "bad").is_err());
    }

    #[test]
    fn test_read_profile_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        fs::create_dir(&stau_dir).unwrap();

        let config = Config {
            stau_dir,
            default_target: temp_dir.path().to_path_buf(),
        };

        assert!(read_profile(&config, "nope").is_err());
    }
}
