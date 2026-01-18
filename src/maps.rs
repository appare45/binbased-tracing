use crate::error::MapsError;

pub struct MemMap {
    pub address: (u64, u64),
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub shared: bool,
    pub private: bool,
    pub offset: u64,
    pub device: (u64, u64),
    pub inode: u64,
    pub pathname: Option<String>,
}

impl TryFrom<String> for MemMap {
    type Error = MapsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
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
                readable: parts[1].contains('r'),
                writable: parts[1].contains('w'),
                executable: parts[1].contains('x'),
                shared: parts[1].contains('s'),
                private: parts[1].contains('p'),
                offset: u64::from_str_radix(parts[2], 16)?,
                device: {
                    let mut range = parts[3].split(':');
                    (
                        u64::from_str_radix(range.next().ok_or(MapsError::ParseError)?, 16)?,
                        u64::from_str_radix(range.next().ok_or(MapsError::ParseError)?, 16)?,
                    )
                },
                inode: u64::from_str_radix(parts[4], 10)?,
                pathname: if parts.len() >= 6 {
                    Some(parts[5].to_string())
                } else {
                    None
                },
            });
        }
        Err(MapsError::ParseError)
    }
}

/// 行のイテレータを受け取り、パースできたMemMapのイテレータを返す
pub fn parse_maps<I>(lines: I) -> impl Iterator<Item = MemMap>
where
    I: Iterator<Item = String>,
{
    lines.filter_map(|line| MemMap::try_from(line).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LINE: &str =
        "00400000-00452000 r-xp 00000000 08:02 173521      /usr/bin/dbus-daemon";

    #[test]
    fn test_memmap_parse_address() {
        let m: MemMap = SAMPLE_LINE.to_string().try_into().unwrap();
        assert_eq!(m.address.0, 0x00400000);
        assert_eq!(m.address.1, 0x00452000);
        assert!(m.readable);
        assert!(!m.writable);
        assert!(m.executable);
        assert!(!m.shared);
        assert!(m.private);
        assert_eq!(m.offset, 0);
        assert_eq!(m.device.0, 0x08);
        assert_eq!(m.device.1, 0x02);
        assert_eq!(m.inode, 173521);
        assert_eq!(m.pathname, Some("/usr/bin/dbus-daemon".to_string()));
    }

    #[test]
    fn test_memmap_parse_without_pathname() {
        let line = "7fff12340000-7fff12360000 rw-p 00000000 00:00 0".to_string();
        let m: MemMap = line.try_into().unwrap();
        assert!(m.pathname.is_none());
    }
}
