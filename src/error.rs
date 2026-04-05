#![allow(clippy::enum_variant_names)]

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("failed to parse HCL input: {0}")]
    ParseHcl(#[from] hcl_edit::parser::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessFileError {
    #[error("failed to read file '{}': {source}", path.display())]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to format file '{}': {source}", path.display())]
    Format {
        path: PathBuf,
        #[source]
        source: FormatError,
    },

    #[error("failed to write formatted output to file '{}': {source}", path.display())]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoverFilesError {
    #[error("failed to expand glob pattern '{pattern}': {source}")]
    GlobPattern {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },

    #[error("failed to read glob entry: {0}")]
    GlobEntry(#[from] glob::GlobError),

    #[error("failed to read directory '{}': {source}", path.display())]
    ReadDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("failed to discover files: {0}")]
    DiscoverFiles(#[from] DiscoverFilesError),

    #[error("failed to process file: {0}")]
    ProcessFile(#[from] ProcessFileError),

    #[error("failed to read from stdin: {0}")]
    ReadStdin(#[source] std::io::Error),

    #[error("failed to write to stdout: {0}")]
    WriteStdout(#[source] std::io::Error),

    #[error("failed to format HCL from stdin: {0}")]
    FormatStdin(#[from] FormatError),

    #[error("{count} file(s) need formatting")]
    CheckFailed { count: usize },
}
