use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcError {
    #[error("proc's exe file is not available")]
    Exe(#[source] io::Error),
    #[error("proc's mem file is not available")]
    Mem(#[source] io::Error),
}
