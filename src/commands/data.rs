use crate::cli::DataCommand;
use crate::config::Config;
use crate::error::{Result, StauError, map_io_error};
use crate::template;
use std::fs;
use std::path::PathBuf;

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
    fs::write(&path, content)
        .map_err(|e| map_io_error(e, format!("Cannot write data file: {}", path.display())))?;

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
