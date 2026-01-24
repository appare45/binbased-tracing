use std::{io, num::ParseIntError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcError {
    #[error("proc's exe file is not available")]
    FailedToGetStatus(#[source] io::Error),
}

#[derive(Error, Debug)]
pub enum ElfError {
    #[error("Not an Elf file")]
    NotAnElfFile,

    #[error("IO error")]
    IoError(#[from] std::io::Error),

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
}

#[derive(Error, Debug)]
pub enum PtraceError {
    #[error("Attaching failed")]
    AttachFailed(#[source] nix::errno::Errno),

    #[error("Already stopped")]
    AlreadyStopped,

    #[error("Interrupt failed")]
    InterruptFailed(#[source] nix::errno::Errno),

    #[error("Continue failed")]
    ContinueFailed(#[source] nix::errno::Errno),

    #[error("WaitPID Failed")]
    WaitPIDFailed(#[source] nix::errno::Errno),

    #[error("WaitPID unexpected status")]
    WaitPIDUnexpectedStatus(nix::sys::wait::WaitStatus),

    #[error("Program Exited")]
    ProgramExited,

    #[error("Failed to detach")]
    DetachFailed(#[source] nix::errno::Errno),

    #[error("Process is running")]
    ProcessRunning,

    #[error("Failed to get registers")]
    GetRegistersFailed(#[source] nix::errno::Errno),

    #[error("Failed to set registers")]
    SetRegistersFailed(#[source] nix::errno::Errno),

    #[error("Failed to read")]
    ReadFailed(#[source] nix::errno::Errno),

    #[error("Failed to write")]
    WriteFailed(#[source] nix::errno::Errno),
}

#[derive(Error, Debug)]
pub enum InstrumentError {
    #[error("Already pre instrumented")]
    AlreadyPreInstrumented,

    #[error("Ptrace error")]
    PtraceError(#[from] PtraceError),

    #[error("Not preinstrumentd")]
    NotPreInstrumentd,

    #[error("Failed to mprotect")]
    MprotectFailed(u64),
}

#[derive(Error, Debug)]
pub enum InstructionError {
    #[error("Index out of bounds")]
    IndexOutOfBounds,
}
