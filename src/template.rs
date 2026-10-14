use crate::error::{Result, StauError, map_io_error};
use crate::symlink::{MappingKind, SymlinkMapping};
use minijinja::{Environment, UndefinedBehavior};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// Builds the fully merged template data for this machine.
///
/// Precedence (low -> high), deep-merged:
/// 1. auto facts under `stau.*` (see [`facts`]);
/// 2. `<STAU_DIR>/.stau/data.toml` (committed repo defaults, optional);
/// 3. `~/.config/stau/machine.toml` (machine-local, optional);
/// 4. `--set KEY=VALUE` pairs (dotted keys nest).
///
/// A missing data file is not an error; a present file that fails to parse is
/// a template error naming the path. The reserved top-level key `stau` may not
/// appear in any user data source.
pub fn build_data(stau_dir: &Path, sets: &[String]) -> Result<Value> {
    let mut data = facts();

    let repo_path = stau_dir.join(".stau").join("data.toml");
    if repo_path.exists() {
        let parsed = load_toml_file(&repo_path)?;
        reject_reserved_stau(&parsed)?;
        deep_merge(&mut data, parsed);
    }

    if let Some(config_dir) = dirs::config_dir() {
        let machine_path = config_dir.join("stau").join("machine.toml");
        if machine_path.exists() {
            let parsed = load_toml_file(&machine_path)?;
            reject_reserved_stau(&parsed)?;
            deep_merge(&mut data, parsed);
        }
    }

    let set_value = parse_sets(sets)?;
    reject_reserved_stau(&set_value)?;
    deep_merge(&mut data, set_value);

    Ok(data)
}

/// The read-only auto facts, exposed under the reserved top-level key `stau`.
///
/// Facts are best effort: `stau.hostname`/`stau.fqdn`, `stau.user`,
/// `stau.home`, `stau.kernel`, and `stau.os_release` are omitted when their
/// source is unavailable; `stau.arch` and `stau.os` are always present.
pub fn facts() -> Value {
    let mut stau = Map::new();

    if let Some(hostname) = read_trimmed("/proc/sys/kernel/hostname") {
        stau.insert("hostname".to_string(), Value::String(hostname.clone()));
        // Best effort: stau does not resolve a fully qualified name yet.
        stau.insert("fqdn".to_string(), Value::String(hostname));
    }

    let user = std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("LOGNAME").ok());
    if let Some(user) = user {
        stau.insert("user".to_string(), Value::String(user));
    }

    if let Some(home) = dirs::home_dir() {
        stau.insert(
            "home".to_string(),
            Value::String(home.display().to_string()),
        );
    }

    stau.insert(
        "arch".to_string(),
        Value::String(std::env::consts::ARCH.to_string()),
    );
    stau.insert(
        "os".to_string(),
        Value::String(std::env::consts::OS.to_string()),
    );

    if let Some(kernel) = read_trimmed("/proc/sys/kernel/osrelease") {
        stau.insert("kernel".to_string(), Value::String(kernel));
    }

    if let Some(os_release) = read_os_release() {
        stau.insert("os_release".to_string(), os_release);
    }

    let mut root = Map::new();
    root.insert("stau".to_string(), Value::Object(stau));
    Value::Object(root)
}

/// Renders a template file with [`minijinja`] using strict undefined
/// behaviour, so referencing a missing variable is an error rather than
/// silently producing empty output.
pub fn render(source: &Path, data: &Value) -> Result<String> {
    let contents = fs::read_to_string(source)
        .map_err(|e| map_io_error(e, format!("Cannot read template: {}", source.display())))?;

    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    // A trailing newline is content in a dotfile; render bytes exactly.
    env.set_keep_trailing_newline(true);
    let name = source.display().to_string();
    env.add_template_owned(&name, contents)
        .map_err(|e| template_error(source, &e))?;
    let template = env
        .get_template(&name)
        .map_err(|e| template_error(source, &e))?;
    template
        .render(data)
        .map_err(|e| template_error(source, &e))
}

/// Renders every `Render` mapping, recording the outcome per mapping instead of
/// returning early: on success `rendered` is set, on failure `render_error` is
/// set. One broken template must not hide the state of its siblings.
pub fn render_mappings(mappings: &mut [SymlinkMapping], data: &Value) {
    for mapping in mappings {
        if mapping.kind != MappingKind::Render {
            continue;
        }
        match render(&mapping.source, data) {
            Ok(content) => {
                *mapping =
                    SymlinkMapping::render(mapping.source.clone(), mapping.target.clone(), content);
            }
            Err(error) => {
                mapping.render_error = Some(error.to_string());
            }
        }
    }
}

/// Returns a `TemplateError` naming every mapping whose render failed, or `None`
/// when every `Render` mapping rendered successfully.
pub fn collect_render_errors(mappings: &[SymlinkMapping]) -> Option<StauError> {
    let messages: Vec<&str> = mappings
        .iter()
        .filter_map(|mapping| mapping.render_error.as_deref())
        .collect();
    if messages.is_empty() {
        None
    } else {
        Some(StauError::TemplateError(messages.join("\n")))
    }
}

/// Parses `KEY=VALUE` pairs into a nested object, so `host.pgx=10.0.0.1`
/// becomes `{"host": {"pgx": "10.0.0.1"}}`. Values are always strings.
fn parse_sets(sets: &[String]) -> Result<Value> {
    let mut root = Map::new();
    for pair in sets {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(StauError::InvalidArguments(format!(
                "--set expects KEY=VALUE, got {pair:?}"
            )));
        };
        if key.is_empty() {
            return Err(StauError::InvalidArguments(
                "--set expects a non-empty KEY".to_string(),
            ));
        }
        if matches!(key, "STAU_DIR" | "STAU_TARGET" | "STAU_PACKAGE") {
            return Err(StauError::InvalidArguments(format!(
                "--set cannot override stau's own hook variable {key}"
            )));
        }
        insert_dotted(&mut root, key, Value::String(value.to_string()));
    }
    Ok(Value::Object(root))
}

/// Inserts `value` at the dotted `key` path, creating intermediate tables.
fn insert_dotted(root: &mut Map<String, Value>, key: &str, value: Value) {
    let segments: Vec<&str> = key.split('.').collect();
    let mut map = root;
    for (i, segment) in segments.iter().enumerate() {
        let last = i + 1 == segments.len();
        if last {
            map.insert((*segment).to_string(), value.clone());
        } else {
            map.entry((*segment).to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            let child = map.get_mut(*segment).expect("just inserted");
            if !child.is_object() {
                *child = Value::Object(Map::new());
            }
            map = child.as_object_mut().expect("is an object");
        }
    }
}

/// Deep-merges `overlay` into `base`: tables merge recursively, scalars and
/// arrays replace.
fn deep_merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        (slot, overlay) => *slot = overlay,
    }
}

/// Rejects any user data source that uses the reserved top-level `stau` key.
fn reject_reserved_stau(value: &Value) -> Result<()> {
    if value.get("stau").is_some() {
        return Err(StauError::InvalidArguments(
            "'stau' is a reserved top-level data key and cannot be set by user data".to_string(),
        ));
    }
    Ok(())
}

/// Parses a TOML data file into a JSON object, reporting parse failures as
/// template errors that name the path.
fn load_toml_file(path: &Path) -> Result<Value> {
    let contents = fs::read_to_string(path)
        .map_err(|e| map_io_error(e, format!("Cannot read data file: {}", path.display())))?;
    let parsed: toml::Value = toml::from_str(&contents).map_err(|e| {
        StauError::TemplateError(format!("failed to parse data file {}: {e}", path.display()))
    })?;
    serde_json::to_value(parsed).map_err(|e| {
        StauError::TemplateError(format!(
            "failed to convert data file {}: {e}",
            path.display()
        ))
    })
}

fn template_error(source: &Path, error: &minijinja::Error) -> StauError {
    StauError::TemplateError(format!(
        "failed to render template {}: {}",
        source.display(),
        error
    ))
}

fn read_trimmed(path: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    Some(contents.trim().to_string())
}

/// Parses `/etc/os-release` into a map of lowercased, quote-stripped values.
fn read_os_release() -> Option<Value> {
    let contents = fs::read_to_string("/etc/os-release").ok()?;
    Some(parse_os_release(&contents))
}

/// Parses os-release contents; a malformed line (no `=`) is skipped so it does
/// not drop the whole map.
fn parse_os_release(contents: &str) -> Value {
    let mut map = Map::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
        map.insert(key.to_lowercase(), Value::String(value.to_string()));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn hermetic_stau_dir(tmp: &TempDir) -> std::path::PathBuf {
        let stau_dir = tmp.path().join("dotfiles");
        fs::create_dir_all(&stau_dir).unwrap();
        stau_dir
    }

    #[test]
    fn test_facts_presence() {
        let facts = facts();
        let stau = facts.get("stau").expect("stau key").as_object().unwrap();

        assert!(
            stau.get("hostname")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty()),
            "hostname should be present and non-empty"
        );
        assert_eq!(stau.get("fqdn"), stau.get("hostname"));
        assert_eq!(
            stau.get("arch").and_then(Value::as_str),
            Some(std::env::consts::ARCH)
        );
        assert_eq!(
            stau.get("os").and_then(Value::as_str),
            Some(std::env::consts::OS)
        );
        assert!(stau.get("home").is_some());
        assert!(stau.get("kernel").is_some());
    }

    #[test]
    fn test_facts_user_from_env() {
        temp_env::with_vars(
            vec![("USER", Some("alice")), ("LOGNAME", None::<&str>)],
            || {
                let facts = facts();
                assert_eq!(facts["stau"]["user"], "alice");
            },
        );
    }

    #[test]
    fn test_deep_merge_precedence() {
        let tmp = TempDir::new().unwrap();
        let stau_dir = hermetic_stau_dir(&tmp);
        let xdg = tmp.path().join("xdg");
        fs::create_dir_all(stau_dir.join(".stau")).unwrap();
        fs::create_dir_all(xdg.join("stau")).unwrap();

        fs::write(
            stau_dir.join(".stau/data.toml"),
            "[host]\npgx = \"repo\"\n[user]\nemail = \"repo@example\"\n",
        )
        .unwrap();
        fs::write(xdg.join("stau/machine.toml"), "[host]\npgx = \"machine\"\n").unwrap();

        temp_env::with_vars(
            vec![
                ("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap())),
                ("HOME", Some(tmp.path().to_str().unwrap())),
            ],
            || {
                let data = build_data(&stau_dir, &["host.pgx=cli".to_string()]).unwrap();
                // --set wins over machine.toml, which wins over the repo file.
                assert_eq!(data["host"]["pgx"], "cli");
                // Deep merge keeps unrelated keys from the lower layers.
                assert_eq!(data["user"]["email"], "repo@example");
            },
        );
    }

    #[test]
    fn test_strict_undefined_is_an_error() {
        let tmp = TempDir::new().unwrap();
        let template = tmp.path().join("t.tmpl");
        fs::write(&template, "hello {{ missing.var }}").unwrap();

        let data = facts();
        let err = render(&template, &data).unwrap_err();
        assert!(matches!(err, StauError::TemplateError(_)), "got {err:?}");
        assert!(err.to_string().contains("undefined"), "message: {err}");
        assert!(err.to_string().contains("t.tmpl"), "message: {err}");
    }

    #[test]
    fn test_dotted_set_nesting() {
        let value = parse_sets(&["host.pgx=10.0.0.1".to_string()]).unwrap();
        assert_eq!(value["host"]["pgx"], "10.0.0.1");
        // Values are always strings.
        assert!(value["host"]["pgx"].is_string());
    }

    #[test]
    fn test_basic_render() {
        let tmp = TempDir::new().unwrap();
        let template = tmp.path().join("t.tmpl");
        fs::write(
            &template,
            "user={{ user.email }}\n{% if enabled %}on{% else %}off{% endif %}",
        )
        .unwrap();

        let data = serde_json::json!({
            "user": { "email": "a@b.c" },
            "enabled": true,
        });
        assert_eq!(render(&template, &data).unwrap(), "user=a@b.c\non");
    }

    #[test]
    fn test_reserved_stau_rejected() {
        let tmp = TempDir::new().unwrap();
        let stau_dir = hermetic_stau_dir(&tmp);
        let xdg = tmp.path().join("xdg");
        fs::create_dir_all(&xdg).unwrap();

        temp_env::with_vars(
            vec![
                ("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap())),
                ("HOME", Some(tmp.path().to_str().unwrap())),
            ],
            || {
                for set in [
                    &["stau.foo=bar".to_string()][..],
                    &["stau=bar".to_string()][..],
                ] {
                    let err = build_data(&stau_dir, set).unwrap_err();
                    assert!(matches!(err, StauError::InvalidArguments(_)), "got {err:?}");
                }
            },
        );
    }

    #[test]
    fn test_repo_data_with_reserved_stau_rejected() {
        let tmp = TempDir::new().unwrap();
        let stau_dir = hermetic_stau_dir(&tmp);
        let xdg = tmp.path().join("xdg");
        fs::create_dir_all(stau_dir.join(".stau")).unwrap();
        fs::create_dir_all(&xdg).unwrap();
        fs::write(stau_dir.join(".stau/data.toml"), "stau = \"reserved\"\n").unwrap();

        temp_env::with_vars(
            vec![
                ("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap())),
                ("HOME", Some(tmp.path().to_str().unwrap())),
            ],
            || {
                let err = build_data(&stau_dir, &[]).unwrap_err();
                assert!(matches!(err, StauError::InvalidArguments(_)), "got {err:?}");
            },
        );
    }

    #[test]
    fn test_malformed_data_file_is_template_error() {
        let tmp = TempDir::new().unwrap();
        let stau_dir = hermetic_stau_dir(&tmp);
        let xdg = tmp.path().join("xdg");
        fs::create_dir_all(stau_dir.join(".stau")).unwrap();
        fs::create_dir_all(&xdg).unwrap();
        fs::write(stau_dir.join(".stau/data.toml"), "not = [valid toml").unwrap();

        temp_env::with_vars(
            vec![
                ("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap())),
                ("HOME", Some(tmp.path().to_str().unwrap())),
            ],
            || {
                let err = build_data(&stau_dir, &[]).unwrap_err();
                assert!(matches!(err, StauError::TemplateError(_)), "got {err:?}");
                assert!(err.to_string().contains("data.toml"), "message: {err}");
            },
        );
    }

    #[test]
    fn test_set_rejects_stau_own_variables() {
        for var in ["STAU_DIR", "STAU_TARGET", "STAU_PACKAGE"] {
            let err = parse_sets(&[format!("{var}=x")]).unwrap_err();
            assert!(matches!(err, StauError::InvalidArguments(_)), "got {err:?}");
        }
    }

    #[test]
    fn test_parse_os_release_tolerates_malformed_line() {
        let value = parse_os_release(
            "ID=ubuntu\nTHIS LINE HAS NO EQUALS SIGN\nPRETTY_NAME=\"Ubuntu 26.04\"\n",
        );
        assert_eq!(value["id"], "ubuntu");
        assert_eq!(value["pretty_name"], "Ubuntu 26.04");
        assert!(value.get("THIS LINE HAS NO EQUALS SIGN").is_none());
    }

    #[test]
    fn test_render_mappings_records_error_and_renders_siblings() {
        use crate::symlink::MappingKind;

        let tmp = TempDir::new().unwrap();
        let good = tmp.path().join("good.tmpl");
        let bad = tmp.path().join("bad.tmpl");
        fs::write(&good, "hello {{ name }}").unwrap();
        fs::write(&bad, "{{ missing.var }}").unwrap();

        let mut mappings = vec![
            SymlinkMapping::new(tmp.path().join("plain"), tmp.path().join("plain-target")),
            SymlinkMapping {
                source: good.clone(),
                target: tmp.path().join("good-target"),
                kind: MappingKind::Render,
                rendered: None,
                render_error: None,
            },
            SymlinkMapping {
                source: bad.clone(),
                target: tmp.path().join("bad-target"),
                kind: MappingKind::Render,
                rendered: None,
                render_error: None,
            },
        ];

        let data = serde_json::json!({ "name": "alice" });
        render_mappings(&mut mappings, &data);

        // The symlink mapping is untouched.
        assert_eq!(mappings[0].kind, MappingKind::Symlink);
        assert_eq!(mappings[0].rendered, None);
        // The good template renders; the bad one records an error.
        assert_eq!(mappings[1].rendered.as_deref(), Some("hello alice"));
        assert_eq!(mappings[1].render_error, None);
        assert_eq!(mappings[2].rendered, None);
        assert!(mappings[2].render_error.is_some());

        let error = collect_render_errors(&mappings).expect("one render failed");
        assert!(matches!(error, StauError::TemplateError(_)));
        assert!(error.to_string().contains("bad.tmpl"), "message: {error}");
    }
}
