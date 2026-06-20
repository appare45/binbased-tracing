use std::io::{BufRead, BufReader};

use crate::error::MapsError;

#[derive(Debug)]
pub struct MemMap {
    pub address: (u64, u64),
    pub _readable: bool,
    pub _writable: bool,
    pub executable: bool,
    pub _shared: bool,
    pub _private: bool,
    pub _offset: u64,
    pub _device: (u64, u64),
    pub _inode: u64,
    pub _pathname: Option<String>,
}

impl TryFrom<&str> for MemMap {
    type Error = MapsError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() >= 5 {
            return Ok(MemMap {
                address: {
                    let mut range = parts[0].split('-');
                    (
                        u64::from_str_radix(range.next().ok_or(MapsError::ParseError)?, 16)?,
                        u64::from_str_radix(range.next().ok_or(MapsError::ParseError)?, 16)?,
                    )
                },
                _readable: parts[1].contains('r'),
                _writable: parts[1].contains('w'),
                executable: parts[1].contains('x'),
                _shared: parts[1].contains('s'),
                _private: parts[1].contains('p'),
                _offset: u64::from_str_radix(parts[2], 16)?,
                _device: {
                    let mut range = parts[3].split(':');
                    (
                        u64::from_str_radix(range.next().ok_or(MapsError::ParseError)?, 16)?,
                        u64::from_str_radix(range.next().ok_or(MapsError::ParseError)?, 16)?,
                    )
                },
                _inode: parts[4].parse::<u64>()?,
                _pathname: if parts.len() >= 6 {
                    Some(parts[5].to_string())
                } else {
                    None
                },
            });
        }
        Err(MapsError::ParseError)
    }
}

/// hint_addrの近くで空き領域を探す。見つけた領域をregionsに追加して次回の衝突を防ぐ。
pub fn find_free_region(regions: &mut Vec<(u64, u64)>, hint_addr: u64, size: u64) -> u64 {
    let page = 0x1000u64;
    regions.sort_unstable_by_key(|r| r.0);

    let mut candidate = (hint_addr + page - 1) & !(page - 1);
    for &(start, end) in regions.iter() {
        if candidate + size <= start {
            break;
        }
        if end > candidate {
            candidate = (end + page - 1) & !(page - 1);
        }
    }
    regions.push((candidate, candidate + size));
    candidate
}

/// 行のイテレータを受け取り、パースできたMemMapのイテレータを返す
pub fn parse_maps<R: BufRead>(reader: R) -> impl Iterator<Item = MemMap> {
    BufReader::new(reader)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| MemMap::try_from(line.as_str()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LINE: &str =
        "00400000-00452000 r-xp 00000000 08:02 173521      /usr/bin/dbus-daemon";

    #[test]
    fn test_memmap_parse_address() {
        let m: MemMap = SAMPLE_LINE.try_into().unwrap();
        assert_eq!(m.address.0, 0x00400000);
        assert_eq!(m.address.1, 0x00452000);
        assert!(m._readable);
        assert!(!m._writable);
        assert!(m.executable);
        assert!(!m._shared);
        assert!(m._private);
        assert_eq!(m._offset, 0);
        assert_eq!(m._device.0, 0x08);
        assert_eq!(m._device.1, 0x02);
        assert_eq!(m._inode, 173521);
        assert_eq!(m._pathname, Some("/usr/bin/dbus-daemon".to_string()));
    }
}
