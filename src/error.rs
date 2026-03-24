use std::io;
use std::path::PathBuf;
use std::string::FromUtf8Error;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("failed to parse YAML snapshot: {0}")]
    InvalidYaml(#[from] serde_norway::Error),
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("source path has unsupported extension: {path}")]
    InvalidExtension { path: PathBuf },
    #[error("no default task file found; checked data/tasks.yml and data/tasks.yaml")]
    MissingDefaultPaths,
    #[error("source path does not exist: {path}")]
    MissingPath { path: PathBuf },
    #[error("source path is a directory: {path}")]
    DirectoryPath { path: PathBuf },
    #[error("source path is a broken symlink: {path}")]
    BrokenSymlink { path: PathBuf },
    #[error("failed to read source metadata for {path}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to determine modified time for {path}: {source}")]
    ModifiedTime {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read source file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("source file is not valid UTF-8: {path}")]
    InvalidUtf8 {
        path: PathBuf,
        #[source]
        source: FromUtf8Error,
    },
    #[error(transparent)]
    Parse(#[from] ParseError),
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Load(#[from] LoadError),
}
