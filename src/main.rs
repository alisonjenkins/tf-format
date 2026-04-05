#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

use tf_format::error::{CliError, DiscoverFilesError, ProcessFileError};

const TF_EXTENSIONS: &[&str] = &["tf", "tofu", "tfvars"];

#[derive(Parser)]
#[command(
    name = "tf-format",
    about = "Opinionated Terraform/OpenTofu HCL formatter"
)]
struct Cli {
    /// Files, glob patterns, or directories to format
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
    if cli.stdin {
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .map_err(CliError::ReadStdin)?;

        let output = tf_format::format_hcl(&input)?;

        io::stdout()
            .write_all(output.as_bytes())
            .map_err(CliError::WriteStdout)?;

        return Ok(());
    }

    let paths = discover_files(&cli.files)?;

    if paths.is_empty() {
        eprintln!("No .tf, .tofu, or .tfvars files found");
        return Ok(());
    }

    let mut needs_formatting = Vec::new();

    for path in &paths {
        let changed = process_file(path, cli.check, cli.diff)?;
        if changed {
            needs_formatting.push(path.clone());
        }
    }

    if cli.check && !needs_formatting.is_empty() {
        for path in &needs_formatting {
            eprintln!("{}", path.display());
        }
        return Err(CliError::CheckFailed {
            count: needs_formatting.len(),
        });
    }

    Ok(())
}

fn process_file(path: &Path, check: bool, diff: bool) -> Result<bool, ProcessFileError> {
    let input = std::fs::read_to_string(path).map_err(|source| ProcessFileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let output = tf_format::format_hcl(&input).map_err(|source| ProcessFileError::Format {
        path: path.to_path_buf(),
        source,
    })?;

    if input == output {
        return Ok(false);
    }

    if check {
        return Ok(true);
    }

    if diff {
        print_diff(path, &input, &output);
        return Ok(true);
    }

    std::fs::write(path, &output).map_err(|source| ProcessFileError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(true)
}

fn print_diff(path: &Path, original: &str, formatted: &str) {
    let path_str = path.display().to_string();
    println!("--- {path_str}");
    println!("+++ {path_str}");

    for (i, (orig_line, fmt_line)) in original.lines().zip(formatted.lines()).enumerate() {
        if orig_line != fmt_line {
            println!("@@ -{line} +{line} @@", line = i + 1);
            println!("-{orig_line}");
            println!("+{fmt_line}");
        }
    }
}

fn discover_files(inputs: &[String]) -> Result<Vec<PathBuf>, DiscoverFilesError> {
    let mut paths = Vec::new();

    for input in inputs {
        let input_path = Path::new(input);

        if input_path.is_dir() {
            collect_tf_files_recursive(input_path, &mut paths)?;
        } else if input_path.is_file() {
            paths.push(input_path.to_path_buf());
        } else {
            // Treat as glob pattern
            let entries = glob::glob(input).map_err(|source| DiscoverFilesError::GlobPattern {
                pattern: input.clone(),
                source,
            })?;

            for entry in entries {
                let path = entry?;
                if path.is_file() {
                    paths.push(path);
                }
            }
        }
    }

    Ok(paths)
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
        let path = entry.path();

        if path.is_dir() {
            collect_tf_files_recursive(&path, paths)?;
        } else if path.is_file()
            && let Some(ext) = path.extension().and_then(|e| e.to_str())
            && TF_EXTENSIONS.contains(&ext)
        {
            paths.push(path);
        }
    }

    Ok(())
}
