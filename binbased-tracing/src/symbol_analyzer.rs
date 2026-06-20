use crate::error::ProcError;
use crate::proc::Proc;
use std::io::{Read, Seek, SeekFrom};

pub struct FunctionAnalysis {
    pub entry_addr: u64,
    pub ret_addrs: Vec<u64>,
}

pub fn analyze_function(proc: &Proc, symbol_name: &str) -> Result<FunctionAnalysis, ProcError> {
    let elf = proc.get_bin()?;
    let exec_base = proc
        .exe_base()
        .ok_or_else(|| ProcError::IoError(std::io::Error::other("Failed to get exe base")))?;

    let (offset, size) = elf.get_symbol(symbol_name.into())?;
    let entry_addr = offset + exec_base;

    println!("{symbol_name} is at 0x{entry_addr:x}");

    let mut exe = proc.get_exe()?;
    let mut buf = vec![0u8; size as usize];
    exe.seek(SeekFrom::Start(offset))?;
    exe.read_exact(&mut buf)?;

    let ret_addrs: Vec<u64> = buf
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(i, chunk)| {
            let inst = u32::from_le_bytes(chunk.try_into().unwrap());
            if inst == 0xd65f03c0 {
                Some(exec_base + offset + (i as u64 * 4))
            } else {
                None
            }
        })
        .collect();

    println!("Found {} ret instructions", ret_addrs.len());
    for (idx, addr) in ret_addrs.iter().enumerate() {
        println!("  ret #{}: 0x{:x}", idx + 1, addr);
    }

    Ok(FunctionAnalysis {
        entry_addr,
        ret_addrs,
    })
}
