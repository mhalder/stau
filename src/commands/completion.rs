use crate::cli::Cli;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::io::Write;

/// Writes the completion script for `shell` to `out`.
///
/// The script is static: it knows the subcommands, aliases, flags and value
/// enums, but not the package names under `STAU_DIR`.
pub fn print_completion(shell: Shell, out: &mut impl Write) {
    generate(shell, &mut Cli::command(), "stau", out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(shell: Shell) -> String {
        let mut out = Vec::new();
        print_completion(shell, &mut out);
        String::from_utf8(out).expect("completion script is valid UTF-8")
    }

    #[test]
    fn zsh_script_declares_the_binary_and_its_subcommands() {
        let script = script(Shell::Zsh);
        assert!(script.starts_with("#compdef stau"), "{script}");
        assert!(script.contains("install"));
        assert!(script.contains("completion"));
    }

    #[test]
    fn fish_script_completes_the_binary_and_its_subcommands() {
        let script = script(Shell::Fish);
        assert!(script.contains("complete -c stau"), "{script}");
        assert!(script.contains("install"));
        assert!(script.contains("completion"));
    }

    #[test]
    fn scripts_cover_aliases_and_value_enums() {
        for shell in [Shell::Zsh, Shell::Fish, Shell::Bash] {
            let script = script(shell);
            assert!(script.contains("uninstall"), "{shell}: missing uninstall");
            assert!(script.contains("rm"), "{shell}: missing rm alias");
            assert!(script.contains("script-timeout"), "{shell}: missing flag");
        }
        // --expect is a ValueEnum, so its variants are completable values.
        assert!(script(Shell::Fish).contains("uninstalled"));
    }
}
