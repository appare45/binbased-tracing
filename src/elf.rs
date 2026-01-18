use std::collections::HashMap;

use goblin::{
    Object,
    elf::{self, Sym, Symtab},
    strtab::Strtab,
};

use crate::error::ElfError;

pub type SymbolMap = HashMap<String, Sym>;

pub struct ELF<'a> {
    elf: elf::Elf<'a>,
    // 関数名->シンボルの対応をキャッシュする
    funcs: SymbolMap,
}

pub fn new<'a>(file: &'a [u8]) -> Result<ELF<'a>, ElfError> {
    match Object::parse(file) {
        Ok(Object::Elf(elf)) => {
            let funcs = new_symbol_map(&elf.syms, &elf.strtab);
            Ok(ELF { elf, funcs })
        }
        _ => Err(ElfError::NotAnElfFile),
    }
}

fn new_symbol_map(symtab: &Symtab, strtab: &Strtab) -> SymbolMap {
    let mut symbol_map = HashMap::new();
    for sym in symtab.iter() {
        if sym.is_function()
            && let Some(name) = strtab.get_at(sym.st_name)
        {
            symbol_map.insert(name.to_string(), sym);
        }
    }
    symbol_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_new_with_valid_elf() {
        use std::io::Read;

        let mut file = File::open("/proc/self/exe").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let result = new(&buffer);
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_error_is_not_an_elf_file() {
        // 16バイト以上必要（goblinのpeekが読む最小サイズ）
        let data = b"This is not ELF!";
        let result = new(data);
        assert!(matches!(result, Err(ElfError::NotAnElfFile)));
    }

    #[test]
    fn test_new_with_empty_file() {
        let data: Vec<u8> = vec![];
        let result = new(&data);
        // 空ファイルはELFではない
        assert!(result.is_err());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_symbol_map_is_not_empty() {
        use std::io::Read;

        let mut file = File::open("/proc/self/exe").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let elf = new(&buffer).unwrap();
        assert!(!elf.funcs.is_empty());
        assert!(elf.funcs.contains_key("main"));
    }
}
