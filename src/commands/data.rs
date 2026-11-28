use crate::cli::DataCommand;
use crate::config::Config;
use crate::error::{Result, StauError, map_io_error};
use crate::fsutil;
use crate::template;
use std::fs;
use std::path::{Path, PathBuf};

/// Entry point for `stau data`.
///
/// Without a subcommand it prints the fully merged data as TOML (default) or
/// pretty JSON (`--json`). With `set <key> <value>` it upserts a dotted key
/// into `~/.config/stau/machine.toml`.
pub fn run_data(
    config: &Config,
    sets: &[String],
    json: bool,
    dry_run: bool,
    command: Option<DataCommand>,
) -> Result<()> {
    match command {
        None => {
            let data = template::build_data(&config.stau_dir, sets)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).expect("template data is serializable")
                );
            } else {
                println!(
                    "{}",
                    toml::to_string_pretty(&data).map_err(|e| {
                        StauError::TemplateError(format!("cannot serialize data as TOML: {e}"))
                    })?
                );
            }
            Ok(())
        }
        Some(DataCommand::Set { key, value }) => set_machine_value(&key, &value, dry_run),
    }
}

/// Upserts `key` into `~/.config/stau/machine.toml`, creating the directory and
/// file as needed. With `dry_run` the resulting file is printed instead of
/// written.
fn set_machine_value(key: &str, value: &str, dry_run: bool) -> Result<()> {
    validate_data_key(key)?;

    let path = machine_toml_path()?;
    let mut root = match fs::read_to_string(&path) {
        Ok(contents) => toml::from_str::<toml::Value>(&contents).map_err(|e| {
            StauError::TemplateError(format!("failed to parse {}: {e}", path.display()))
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(Default::default())
        }
        Err(e) => {
            return Err(map_io_error(
                e,
                format!("Cannot read data file: {}", path.display()),
            ));
        }
    };

    let typed = parse_value(value);
    upsert_dotted(&mut root, key, typed)?;

    let serialized = toml::to_string_pretty(&root).map_err(|e| {
        StauError::TemplateError(format!("cannot serialize {}: {e}", path.display()))
    })?;
    let content = format!("{serialized}\n");

    if dry_run {
        print!("{content}");
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            map_io_error(e, format!("Cannot create directory: {}", parent.display()))
        })?;
    }
    write_data_file(&path, &content)
}

/// Writes `content` over `path` atomically, keeping the file's permission bits
/// and, when `path` is a symlink, the link itself.
///
/// A plain truncate-and-write leaves `machine.toml` empty or half-written when
/// the write fails partway, and stau then refuses to parse the file it just
/// corrupted, so every later `stau data set` fails too.
fn write_data_file(path: &Path, content: &str) -> Result<()> {
    let target = fsutil::resolve_symlink_target(path);
    let permissions = fs::metadata(&target)
        .ok()
        .map(|metadata| metadata.permissions());

    fsutil::write_atomic(&target, content.as_bytes())?;

    if let Some(permissions) = permissions {
        fs::set_permissions(&target, permissions).map_err(|e| {
            map_io_error(e, format!("Cannot set permissions: {}", target.display()))
        })?;
    }
    Ok(())
}

/// Rejects empty segments, a leading/trailing dot, and the reserved `stau`
/// top-level key.
fn validate_data_key(key: &str) -> Result<()> {
    if key.is_empty() || key.starts_with('.') || key.ends_with('.') || key.contains("..") {
        return Err(StauError::InvalidArguments(format!(
            "invalid data key {key:?}: must be a dotted key with non-empty segments"
        )));
    }
    if key.split('.').next() == Some("stau") {
        return Err(StauError::InvalidArguments(
            "'stau' is a reserved top-level data key".to_string(),
        ));
    }
    Ok(())
}

fn machine_toml_path() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|dir| dir.join("stau").join("machine.toml"))
        .ok_or_else(|| StauError::Other("cannot determine config directory".to_string()))
}

/// Types the value as a TOML literal when it parses, otherwise as a string.
fn parse_value(value: &str) -> toml::Value {
    // `toml::from_str` parses a whole document (a table), so a bare value is
    // wrapped in `v = <value>` and unwrapped, which lets numbers, booleans,
    // arrays, quoted strings, and inline tables keep their literal type.
    match toml::from_str::<toml::Value>(&format!("v = {value}")) {
        Ok(parsed) => parsed
            .get("v")
            .cloned()
            .unwrap_or_else(|| toml::Value::String(value.to_string())),
        Err(_) => toml::Value::String(value.to_string()),
    }
}

/// Inserts `value` at the dotted `key` path, creating intermediate tables.
fn upsert_dotted(root: &mut toml::Value, key: &str, value: toml::Value) -> Result<()> {
    let segments: Vec<&str> = key.split('.').collect();
    let mut table = root.as_table_mut().ok_or_else(|| {
        StauError::InvalidArguments(format!("cannot set {key:?}: the root is not a table"))
    })?;

    for (i, segment) in segments.iter().enumerate() {
        let last = i + 1 == segments.len();
        if last {
            table.insert((*segment).to_string(), value.clone());
            return Ok(());
        }
        let entry = table
            .entry((*segment).to_string())
            .or_insert_with(|| toml::Value::Table(Default::default()));
        table = entry.as_table_mut().ok_or_else(|| {
            StauError::InvalidArguments(format!("cannot set {key:?}: {segment} is not a table"))
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tempfile::TempDir;

    /// Points `dirs::config_dir` at a temporary XDG directory and creates the
    /// `stau` subdirectory, so `machine_toml_path` resolves inside `tmp`.
    fn hermetic_config_home(tmp: &TempDir) -> PathBuf {
        let xdg = tmp.path().join("config");
        fs::create_dir_all(xdg.join("stau")).unwrap();
        xdg
    }

    #[test]
    fn test_set_machine_value_replaces_the_file_atomically() {
        let tmp = TempDir::new().unwrap();
        let xdg = hermetic_config_home(&tmp);
        let path = xdg.join("stau").join("machine.toml");
        fs::write(&path, "a = 1\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let inode_before = fs::metadata(&path).unwrap().ino();

        temp_env::with_var("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()), || {
            set_machine_value("host.name", "x", false).unwrap();
        });

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("a = 1"), "contents: {contents}");
        assert!(contents.contains("name"), "contents: {contents}");
        assert_ne!(
            inode_before,
            fs::metadata(&path).unwrap().ino(),
            "machine.toml should be replaced, not truncated in place"
        );
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the existing file's mode should survive");
    }

    #[test]
    fn test_set_machine_value_writes_through_a_symlinked_file() {
        let tmp = TempDir::new().unwrap();
        let xdg = hermetic_config_home(&tmp);
        let real_dir = tmp.path().join("dotfiles");
        fs::create_dir_all(&real_dir).unwrap();
        let real = real_dir.join("machine.toml");
        fs::write(&real, "a = 1\n").unwrap();
        let link = xdg.join("stau").join("machine.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        temp_env::with_var("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()), || {
            set_machine_value("host.name", "x", false).unwrap();
        });

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "a dotfiles-managed machine.toml must stay a symlink"
        );
        let contents = fs::read_to_string(&real).unwrap();
        assert!(contents.contains("name"), "contents: {contents}");
    }

    #[test]
    fn test_set_machine_value_recovers_from_a_stale_temp_file() {
        let tmp = TempDir::new().unwrap();
        let xdg = hermetic_config_home(&tmp);
        let dir = xdg.join("stau");
        let path = dir.join("machine.toml");
        fs::write(&path, "a = 1\n").unwrap();
        // The first temporary name `write_atomic` picks is taken, so it has to
        // move on to the next one instead of failing or spinning.
        fs::write(
            dir.join(format!(".machine.toml.{}.0.tmp", std::process::id())),
            "stale",
        )
        .unwrap();

        temp_env::with_var("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()), || {
            set_machine_value("host.name", "x", false).unwrap();
        });

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("name"), "contents: {contents}");
        assert!(contents.contains("a = 1"), "contents: {contents}");
    }

    #[test]
    fn test_set_machine_value_creates_the_target_of_a_relative_dangling_link() {
        let tmp = TempDir::new().unwrap();
        let xdg = hermetic_config_home(&tmp);
        let real_dir = tmp.path().join("dotfiles");
        fs::create_dir_all(&real_dir).unwrap();
        let link = xdg.join("stau").join("machine.toml");
        std::os::unix::fs::symlink("../../dotfiles/machine.toml", &link).unwrap();

        temp_env::with_var("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()), || {
            set_machine_value("host.name", "x", false).unwrap();
        });

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the dangling link should be kept and its destination created"
        );
        let real = real_dir.join("machine.toml");
        let contents = fs::read_to_string(&real).unwrap();
        assert!(contents.contains("name"), "contents: {contents}");
    }

    #[test]
    fn test_set_machine_value_reports_an_unreadable_file() {
        let tmp = TempDir::new().unwrap();
        let xdg = hermetic_config_home(&tmp);
        // Reading a directory fails with something other than `NotFound`, so
        // it must be reported rather than treated as an absent file and
        // overwritten.
        fs::create_dir(xdg.join("stau").join("machine.toml")).unwrap();

        let err = temp_env::with_var("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()), || {
            set_machine_value("host.name", "x", false)
        })
        .unwrap_err();

        assert!(
            err.to_string().contains("Cannot read data file"),
            "message: {err}"
        );
    }

    #[test]
    fn test_parse_value_types_literals() {
        assert_eq!(parse_value("123"), toml::Value::Integer(123));
        assert_eq!(parse_value("true"), toml::Value::Boolean(true));
        assert_eq!(
            parse_value("\"str\""),
            toml::Value::String("str".to_string())
        );
        assert_eq!(
            parse_value("hello"),
            toml::Value::String("hello".to_string())
        );
    }

    #[test]
    fn test_validate_data_key() {
        assert!(validate_data_key("host.pgx").is_ok());
        assert!(validate_data_key("").is_err());
        assert!(validate_data_key(".a").is_err());
        assert!(validate_data_key("a.").is_err());
        assert!(validate_data_key("a..b").is_err());
        assert!(validate_data_key("stau.foo").is_err());
    }

    #[test]
    fn test_upsert_dotted_nests_tables() {
        let mut root = toml::Value::Table(Default::default());
        upsert_dotted(&mut root, "host.pgx", toml::Value::String("x".to_string())).unwrap();
        upsert_dotted(&mut root, "host.name", toml::Value::String("y".to_string())).unwrap();

        let rendered = toml::to_string_pretty(&root).unwrap();
        assert!(rendered.contains("pgx"), "rendered: {rendered}");
        assert!(rendered.contains("name"), "rendered: {rendered}");
    }
}
