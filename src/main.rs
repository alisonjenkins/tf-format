#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};

use tf_format::error::{CliError, DiscoverFilesError, ProcessFileError};
use tf_format::{FormatOptions, FormatStyle};

const TF_EXTENSIONS: &[&str] = &["tf", "tofu", "tfvars"];

/// Filename suffixes for Terraform/OpenTofu test files. These carry a `.hcl`
/// (or `.json`) final extension, so `Path::extension` reports `hcl`, not
/// `tftest` — match the full suffix instead to avoid formatting every `.hcl`.
const TF_TEST_SUFFIXES: &[&str] = &[".tftest.hcl", ".tofutest.hcl"];

/// Whether `path` is a Terraform/OpenTofu file we should format: a recognised
/// extension (`.tf`, `.tofu`, `.tfvars`) or a test-file suffix
/// (`.tftest.hcl`, `.tofutest.hcl`).
fn is_tf_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str())
        && TF_EXTENSIONS.contains(&ext)
    {
        return true;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| TF_TEST_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)))
}

/// Style selector exposed on the CLI. Maps 1:1 to
/// [`tf_format::FormatStyle`]; declared separately so the CLI's
/// derive macro doesn't reach into the library type.
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum StyleArg {
    /// tf-format's default — alphabetisation, hoisting, expansion.
    #[default]
    Opinionated,
    /// `terraform fmt` / `tofu fmt` parity — alignment + spacing only.
    Minimal,
}

impl From<StyleArg> for FormatStyle {
    fn from(value: StyleArg) -> Self {
        match value {
            StyleArg::Opinionated => FormatStyle::Opinionated,
            StyleArg::Minimal => FormatStyle::Minimal,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "tf-format",
    about = "Opinionated Terraform/OpenTofu HCL formatter"
)]
struct Cli {
    /// Files, glob patterns, or directories to format [default: .]
    files: Vec<String>,

    /// Read from stdin, write to stdout
    #[arg(long)]
    stdin: bool,

    /// Check mode: exit 1 if any files need formatting
    #[arg(long)]
    check: bool,

    /// Print unified diff instead of writing files
    #[arg(long)]
    diff: bool,

    /// Formatting style. `opinionated` is tf-format's default
    /// behaviour; `minimal` mirrors `terraform fmt` / `tofu fmt`
    /// (alignment + spacing only, no reordering).
    #[arg(long, value_enum, default_value_t = StyleArg::Opinionated)]
    style: StyleArg,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), CliError> {
    let opts = FormatOptions {
        style: cli.style.into(),
    };

    if cli.stdin {
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .map_err(CliError::ReadStdin)?;

        let output = tf_format::format_hcl_with(&input, &opts)?;

        // In --check / --diff mode, stdin must not blindly emit the formatted
        // body (that would give an editor/CI integration a false OK). Report
        // whether the input was already formatted instead. The two flags
        // compose: --check --diff prints the diff AND fails the check.
        if cli.diff && input != output {
            print_diff(Path::new("<stdin>"), &input, &output);
        }

        if cli.check {
            if input == output {
                return Ok(());
            }
            return Err(CliError::CheckFailed { count: 1 });
        }

        if cli.diff {
            return Ok(());
        }

        io::stdout()
            .write_all(output.as_bytes())
            .map_err(CliError::WriteStdout)?;

        return Ok(());
    }

    // No paths means "the current directory", like `terraform fmt` — so a
    // bare `tf-format --check` in CI actually checks something instead of
    // silently passing with nothing to do.
    let inputs: Vec<String> = if cli.files.is_empty() {
        vec![String::from(".")]
    } else {
        cli.files.clone()
    };
    let paths = discover_files(&inputs)?;

    if paths.is_empty() {
        eprintln!("No .tf, .tofu, or .tfvars files found");
        return Ok(());
    }

    let mut needs_formatting = Vec::new();
    let mut failed = 0usize;

    // Process every file even if some fail: a single unparseable or
    // unreadable file must not abort the batch and leave the codebase
    // half-formatted. Errors are reported as they occur; we exit
    // non-zero at the end if any file failed.
    for path in &paths {
        match process_file(path, cli.check, cli.diff, &opts) {
            Ok(true) => needs_formatting.push(path.clone()),
            Ok(false) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                failed += 1;
            }
        }
    }

    if cli.check {
        for path in &needs_formatting {
            eprintln!("{}", path.display());
        }
    }

    if failed > 0 {
        return Err(CliError::ProcessFailed { count: failed });
    }

    if cli.check && !needs_formatting.is_empty() {
        return Err(CliError::CheckFailed {
            count: needs_formatting.len(),
        });
    }

    Ok(())
}

fn process_file(
    path: &Path,
    check: bool,
    diff: bool,
    opts: &FormatOptions,
) -> Result<bool, ProcessFileError> {
    let input = std::fs::read_to_string(path).map_err(|source| ProcessFileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let output =
        tf_format::format_hcl_with(&input, opts).map_err(|source| ProcessFileError::Format {
            path: path.to_path_buf(),
            source,
        })?;

    if input == output {
        return Ok(false);
    }

    // --check and --diff compose: print the diff (if asked) and report the
    // file as needing formatting; --check alone used to short-circuit and
    // swallow the diff.
    if diff {
        print_diff(path, &input, &output);
    }
    if check || diff {
        return Ok(true);
    }

    write_atomically(path, &output).map_err(|source| ProcessFileError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(true)
}

/// Write `contents` to `path` atomically: write to a temp file in the same
/// directory, flush it, then rename it over the original. A crash mid-write
/// can never leave a `.tf` file truncated or empty — the rename either
/// completes or it doesn't. The original file's permissions are preserved.
fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };

    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(contents.as_bytes())?;
    tmp.flush()?;

    // Preserve the original file's permissions on the replacement.
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = tmp.as_file().set_permissions(meta.permissions());
    }

    // Same directory => same filesystem => the rename is atomic.
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

fn print_diff(path: &Path, original: &str, formatted: &str) {
    let path_str = path.display().to_string();
    // A real unified diff: the previous hand-rolled version zipped the two line
    // iterators, so net insertions/deletions (changed line counts) were
    // dropped or shown as bogus pairs. similar produces correct hunks with
    // context, insertions, and deletions.
    let diff = similar::TextDiff::from_lines(original, formatted);
    print!("{}", diff.unified_diff().header(&path_str, &path_str));
}

fn discover_files(inputs: &[String]) -> Result<Vec<PathBuf>, DiscoverFilesError> {
    let mut paths = Vec::new();

    for input in inputs {
        let input_path = Path::new(input);

        if input_path.is_dir() {
            collect_tf_files_recursive(input_path, &mut paths)?;
        } else if input_path.is_file() {
            paths.push(input_path.to_path_buf());
        } else if has_glob_metacharacters(input) {
            // Treat as glob pattern.
            let entries = glob::glob(input).map_err(|source| DiscoverFilesError::GlobPattern {
                pattern: input.clone(),
                source,
            })?;

            let before = paths.len();
            for entry in entries {
                let path = entry?;
                if path.is_file() {
                    paths.push(path);
                }
            }
            // A pattern that matches no files is a typo'd or stale glob; fail
            // loudly like the literal-path case so it can't silently pass
            // `--check` in CI.
            if paths.len() == before {
                return Err(DiscoverFilesError::GlobNoMatches {
                    pattern: input.clone(),
                });
            }
        } else {
            // A literal path (no glob metacharacters) that is neither a file
            // nor a directory does not exist. Fail loudly so a typo'd or
            // deleted path doesn't silently pass `--check` in CI.
            return Err(DiscoverFilesError::PathNotFound {
                path: input_path.to_path_buf(),
            });
        }
    }

    Ok(dedup_paths(paths))
}

/// True if `input` contains any glob metacharacter, so it should be expanded
/// as a pattern rather than treated as a literal path.
fn has_glob_metacharacters(input: &str) -> bool {
    input.contains(['*', '?', '[', ']'])
}

/// Canonicalize and deduplicate discovered paths, preserving first-seen order.
/// A file reachable via several inputs (a duplicate argument, or a directory
/// plus an overlapping glob) is processed once. Canonicalization failures fall
/// back to the raw path so unreadable entries still surface downstream.
fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::with_capacity(paths.len());

    for path in paths {
        let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.insert(key) {
            deduped.push(path);
        }
    }

    deduped
}

fn collect_tf_files_recursive(
    dir: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), DiscoverFilesError> {
    let entries = std::fs::read_dir(dir).map_err(|source| DiscoverFilesError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| DiscoverFilesError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;

        // Use the entry's own file type (does NOT follow symlinks) so a symlink
        // pointing at an ancestor can't send us into an infinite/redundant
        // traversal. Symlinks are skipped entirely.
        let file_type = entry
            .file_type()
            .map_err(|source| DiscoverFilesError::ReadDir {
                path: dir.to_path_buf(),
                source,
            })?;
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();

        if file_type.is_dir() {
            // Skip dot-directories (e.g. `.terraform`, `.git`), matching
            // `terraform fmt`, so vendored/cached modules aren't reformatted.
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with('.'))
            {
                continue;
            }
            collect_tf_files_recursive(&path, paths)?;
        } else if file_type.is_file() && is_tf_file(&path) {
            paths.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_tf_extensions_and_test_suffixes() {
        for ok in [
            "main.tf",
            "main.tofu",
            "terraform.tfvars",
            "setup.tftest.hcl",
            "setup.tofutest.hcl",
        ] {
            assert!(is_tf_file(Path::new(ok)), "{ok} should be a tf file");
        }
        for skip in ["README.md", "config.hcl", "data.json", "notes.txt"] {
            assert!(!is_tf_file(Path::new(skip)), "{skip} should be skipped");
        }
    }
}
