use std::io::{Read, Seek};

use goblin::{Hint, peek};

use crate::error::ElfError;

pub struct ELF<'a, R: Read + Seek> {
    file: &'a mut R,
}

pub fn new<'a, R: Read + Seek>(file: &'a mut R) -> Result<ELF<'a, R>, ElfError> {
    let hint = peek(&mut *file).map_err(|e| ElfError::ReadError(e))?;
    match hint {
        Hint::Elf(_) => Ok(ELF { file }),
        _ => Err(ElfError::NotAnElfFile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Cursor;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_new_with_valid_elf() {
        let mut file = File::open("/proc/self/exe").unwrap();
        let result = new(&mut file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_error_is_not_an_elf_file() {
        // 16バイト以上必要（goblinのpeekが読む最小サイズ）
        let data = b"This is not ELF!";
        let mut cursor = Cursor::new(data.to_vec());
        let result = new(&mut cursor);
        assert!(matches!(result, Err(ElfError::NotAnElfFile)));
    }

    #[test]
    fn test_new_with_empty_file() {
        let data: Vec<u8> = vec![];
        let mut cursor = Cursor::new(data);
        let result = new(&mut cursor);
        // 空ファイルはELFではない
        assert!(result.is_err());
    }
}
