use crate::config::Config;
use crate::error::Result;
use crate::template;
use std::path::Path;

/// Entry point for `stau render`.
///
/// Resolves a relative `<file>` against `STAU_DIR`, renders it with the merged
/// data, and prints the exact result to stdout with no added newline.
pub fn run_render(config: &Config, file: &Path, data: &serde_json::Value) -> Result<()> {
    let path = if file.is_absolute() {
        file.to_path_buf()
    } else {
        config.stau_dir.join(file)
    };

    let rendered = template::render(&path, data)?;
    print!("{rendered}");
    Ok(())
}
