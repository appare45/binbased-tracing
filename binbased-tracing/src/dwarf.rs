use crate::error::DwarfError;
use gimli::{AttributeValue, DebuggingInformationEntry, Dwarf, EndianSlice, RunTimeEndian, Unit};
use object::{Object, ObjectSection};

/// Go runtime構造体のフィールドオフセット情報
#[derive(Clone)]
pub struct RuntimeOffsets {
    pub goid: u64,
    // 将来的に追加予定:
    // pub m: u64,           // runtime.mへのポインタ
    // pub stack_lo: u64,    // スタックの下限
    // pub stack_hi: u64,    // スタックの上限
}

impl RuntimeOffsets {
    pub fn from_elf(elf_bytes: &[u8]) -> Result<Self, DwarfError> {
        let object_file = object::File::parse(elf_bytes)
            .map_err(|_| DwarfError::NoDwarfInfo)?;

        // Load DWARF sections - need to own the data
        let endian = if object_file.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };

        // Pre-load all sections to owned Vec to avoid lifetime issues
        let load_section_data = |name: &str| -> Vec<u8> {
            object_file
                .section_by_name(name)
                .and_then(|section| section.uncompressed_data().ok())
                .map(|cow| cow.into_owned())
                .unwrap_or_default()
        };

        let debug_abbrev = load_section_data(".debug_abbrev");
        let debug_info = load_section_data(".debug_info");
        let debug_str = load_section_data(".debug_str");
        let debug_line = load_section_data(".debug_line");
        let debug_str_offsets = load_section_data(".debug_str_offsets");
        let debug_addr = load_section_data(".debug_addr");
        let debug_rnglists = load_section_data(".debug_rnglists");
        let debug_loclists = load_section_data(".debug_loclists");

        let dwarf = gimli::Dwarf {
            debug_abbrev: gimli::DebugAbbrev::new(&debug_abbrev, endian),
            debug_info: gimli::DebugInfo::new(&debug_info, endian),
            debug_str: gimli::DebugStr::new(&debug_str, endian),
            debug_line: gimli::DebugLine::new(&debug_line, endian),
            debug_str_offsets: gimli::DebugStrOffsets::from(EndianSlice::new(&debug_str_offsets, endian)),
            debug_addr: gimli::DebugAddr::from(EndianSlice::new(&debug_addr, endian)),
            ranges: gimli::RangeLists::new(gimli::DebugRanges::new(&[], endian), gimli::DebugRngLists::new(&debug_rnglists, endian)),
            locations: gimli::LocationLists::new(
                gimli::DebugLoc::new(&[], endian),
                gimli::DebugLocLists::new(&debug_loclists, endian)
            ),
            ..Default::default()
        };

        // Search for runtime.g struct fields
        let goid = find_field_offset(&dwarf, "runtime.g", "goid")?;

        // 将来的な拡張例:
        // let m = find_field_offset(&dwarf, "runtime.g", "m")?;
        // let stack = find_field_offset(&dwarf, "runtime.g", "stack")?;

        Ok(RuntimeOffsets { goid })
    }
}

/// 指定した構造体の指定したフィールドのオフセットを取得する
fn find_field_offset(
    dwarf: &Dwarf<EndianSlice<RunTimeEndian>>,
    struct_name: &str,
    field_name: &str,
) -> Result<u64, DwarfError> {
    let mut units = dwarf.units();

    while let Some(header) = units.next()? {
        let unit = dwarf.unit(header)?;

        if let Some(offset) = search_unit_for_field(&dwarf, &unit, struct_name, field_name)? {
            return Ok(offset);
        }
    }

    Err(DwarfError::StructNotFound(struct_name.to_string()))
}

fn search_unit_for_field(
    dwarf: &Dwarf<EndianSlice<RunTimeEndian>>,
    unit: &Unit<EndianSlice<RunTimeEndian>>,
    struct_name: &str,
    field_name: &str,
) -> Result<Option<u64>, DwarfError> {
    let mut entries = unit.entries();

    while let Some((_, entry)) = entries.next_dfs()? {
        // Look for structure type with the specified name
        if entry.tag() == gimli::DW_TAG_structure_type {
            if let Some(name) = get_entry_name(dwarf, unit, entry)? {
                if name == struct_name {
                    // Found the struct, now find the field
                    return find_struct_field_offset(dwarf, unit, entry, field_name);
                }
            }
        }
    }

    Ok(None)
}

fn get_entry_name(
    dwarf: &Dwarf<EndianSlice<RunTimeEndian>>,
    _unit: &Unit<EndianSlice<RunTimeEndian>>,
    entry: &DebuggingInformationEntry<EndianSlice<RunTimeEndian>>,
) -> Result<Option<String>, DwarfError> {
    if let Some(attr) = entry.attr(gimli::DW_AT_name)? {
        match attr.value() {
            AttributeValue::DebugStrRef(offset) => {
                let name = dwarf.debug_str.get_str(offset)?;
                return Ok(Some(name.to_string()?.to_string()));
            }
            AttributeValue::String(s) => {
                return Ok(Some(s.to_string()?.to_string()));
            }
            _ => {}
        }
    }
    Ok(None)
}

fn find_struct_field_offset(
    dwarf: &Dwarf<EndianSlice<RunTimeEndian>>,
    unit: &Unit<EndianSlice<RunTimeEndian>>,
    struct_entry: &DebuggingInformationEntry<EndianSlice<RunTimeEndian>>,
    field_name: &str,
) -> Result<Option<u64>, DwarfError> {
    let mut entries = unit.entries_at_offset(struct_entry.offset())?;
    let mut depth = 0;

    // Get struct name for better error messages
    let struct_name = get_entry_name(dwarf, unit, struct_entry)?
        .unwrap_or_else(|| "unknown".to_string());

    // Skip the struct entry itself
    if let Some((delta, _)) = entries.next_dfs()? {
        depth += delta;
    }

    // Iterate through children (struct members)
    while let Some((delta, entry)) = entries.next_dfs()? {
        depth += delta;

        // If we've gone back up to the parent level, we're done
        if depth <= 0 {
            break;
        }

        // Look for members with the specified field name
        if entry.tag() == gimli::DW_TAG_member {
            if let Some(name) = get_entry_name(dwarf, unit, entry)? {
                if name == field_name {
                    // Found the field, get its offset
                    if let Some(attr) = entry.attr(gimli::DW_AT_data_member_location)? {
                        let offset = match attr.value() {
                            AttributeValue::Udata(offset) => offset,
                            AttributeValue::Data1(offset) => offset as u64,
                            AttributeValue::Data2(offset) => offset as u64,
                            AttributeValue::Data4(offset) => offset as u64,
                            AttributeValue::Data8(offset) => offset,
                            _ => return Err(DwarfError::AttributeNotFound),
                        };
                        return Ok(Some(offset));
                    }
                }
            }
        }
    }

    Err(DwarfError::FieldNotFound {
        struct_name,
        field: field_name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_goid_offset_in_demo() {
        // Test with demo/demo binary if it exists
        if let Ok(bytes) = fs::read("./demo/demo") {
            let result = RuntimeOffsets::from_elf(&bytes);
            assert!(result.is_ok());
            let offsets = result.unwrap();
            // Based on readelf verification, goid is at offset 152
            assert_eq!(offsets.goid, 152);
        }
    }

    #[test]
    fn test_generic_find_field_offset() {
        // Test the generic find_field_offset function
        if let Ok(bytes) = fs::read("./demo/demo") {
            let object_file = object::File::parse(bytes.as_slice()).unwrap();
            let endian = if object_file.is_little_endian() {
                RunTimeEndian::Little
            } else {
                RunTimeEndian::Big
            };

            let load_section_data = |name: &str| -> Vec<u8> {
                object_file
                    .section_by_name(name)
                    .and_then(|section| section.uncompressed_data().ok())
                    .map(|cow| cow.into_owned())
                    .unwrap_or_default()
            };

            let debug_abbrev = load_section_data(".debug_abbrev");
            let debug_info = load_section_data(".debug_info");
            let debug_str = load_section_data(".debug_str");
            let debug_line = load_section_data(".debug_line");
            let debug_str_offsets = load_section_data(".debug_str_offsets");
            let debug_addr = load_section_data(".debug_addr");
            let debug_rnglists = load_section_data(".debug_rnglists");
            let debug_loclists = load_section_data(".debug_loclists");

            let dwarf = gimli::Dwarf {
                debug_abbrev: gimli::DebugAbbrev::new(&debug_abbrev, endian),
                debug_info: gimli::DebugInfo::new(&debug_info, endian),
                debug_str: gimli::DebugStr::new(&debug_str, endian),
                debug_line: gimli::DebugLine::new(&debug_line, endian),
                debug_str_offsets: gimli::DebugStrOffsets::from(EndianSlice::new(&debug_str_offsets, endian)),
                debug_addr: gimli::DebugAddr::from(EndianSlice::new(&debug_addr, endian)),
                ranges: gimli::RangeLists::new(
                    gimli::DebugRanges::new(&[], endian),
                    gimli::DebugRngLists::new(&debug_rnglists, endian)
                ),
                locations: gimli::LocationLists::new(
                    gimli::DebugLoc::new(&[], endian),
                    gimli::DebugLocLists::new(&debug_loclists, endian)
                ),
                ..Default::default()
            };

            // Test finding goid field
            let goid_offset = find_field_offset(&dwarf, "runtime.g", "goid");
            assert!(goid_offset.is_ok());
            assert_eq!(goid_offset.unwrap(), 152);

            // Test error case: non-existent field
            let result = find_field_offset(&dwarf, "runtime.g", "nonexistent_field");
            assert!(result.is_err());
            match result {
                Err(DwarfError::FieldNotFound { struct_name, field }) => {
                    assert_eq!(struct_name, "runtime.g");
                    assert_eq!(field, "nonexistent_field");
                }
                _ => panic!("Expected FieldNotFound error"),
            }
        }
    }
}
