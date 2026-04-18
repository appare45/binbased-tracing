use std::{io, num::ParseIntError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcError {
    #[error("proc's exe file is not available")]
    FailedToGetStatus(#[source] io::Error),

    #[error("Failed to waitpid")]
    FailedToWaitPid(#[source] nix::errno::Errno),

    #[error("IO error")]
    IoError(#[from] std::io::Error),

    #[error("Elf Error")]
    ElfError(#[from] ElfError),
}

#[derive(Error, Debug)]
pub enum ElfError {
    #[error("Not an Elf file")]
    NotAnElfFile,

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
    #[error("Ptrace error")]
    PtraceError(#[from] PtraceError),

    #[error("Failed to mprotect")]
    MprotectFailed(u64),

    #[error("Failed to mmap")]
    SyscallFailed(u64),

    #[error("String is not available")]
    StringIsNotAvailable(#[from] std::ffi::NulError),

    #[error("Failed to convert u64 into u32")]
    Overflow(#[from] std::num::TryFromIntError),

    #[error("DWARF error")]
    DwarfError(#[from] DwarfError),

    #[error("IO error")]
    IoError(#[from] std::io::Error),

    #[error("Elf error")]
    ElfError(#[from] ElfError),

    #[error("Proc error")]
    ProcError(#[from] ProcError),
}

#[derive(Error, Debug)]
pub enum MonitorError {
    #[error("Process error")]
    ProcError(#[from] ProcError),

    #[error("Failed to join collector thread")]
    CollectorThreadJoinFailed,
}

#[derive(Error, Debug)]
pub enum EventBufferError {
    #[error("memfd_create failed: {0}")]
    MemfdCreateFailed(#[source] nix::errno::Errno),

    #[error("ftruncate failed: {0}")]
    FtruncateFailed(#[source] nix::errno::Errno),

    #[error("mmap failed: {0}")]
    MmapFailed(#[source] nix::errno::Errno),
}

#[derive(Error, Debug)]
pub enum DwarfError {
    #[error("DWARF情報が見つかりません")]
    NoDwarfInfo,

    #[error("構造体 {0} が見つかりません")]
    StructNotFound(String),

    #[error("フィールド {field} が構造体 {struct_name} 内に見つかりません")]
    FieldNotFound { struct_name: String, field: String },

    #[error("DWARF属性が見つかりません")]
    AttributeNotFound,

    #[error("Gimliエラー")]
    GimliError(#[from] gimli::Error),
}
