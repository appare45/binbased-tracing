use std::{io, num::ParseIntError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcError {
    #[error("proc's exe file is not available")]
    Exe(#[source] io::Error),
    #[error("proc's mem file is not available")]
    Mem(#[source] io::Error),
    #[error("proc's map file is not available")]
    Map(#[source] io::Error),
}

#[derive(Error, Debug)]
pub enum ElfError {
    #[error("Not an Elf file")]
    NotAnElfFile,

    #[error("Failed to read file")]
    FailedToRead,

    #[error("Not found")]
    NotFound,
}

#[derive(Error, Debug)]
pub enum MapsError {
    #[error("Failed to Read file")]
    ReadFailed(#[from] io::Error),

    #[error("Failed to parse int")]
    FailedToParse(#[from] ParseIntError),

    #[error("Parse error")]
    ParseError,

    #[error("Not found")]
    NotFound,
}
