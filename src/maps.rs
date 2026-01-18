use crate::error::MapsError;

struct MemMap<'a> {
    address: (u64, u64),
    readable: bool,
    writable: bool,
    executable: bool,
    shared: bool,
    private: bool,
    offset: u64,
    device: (u64, u64),
    inode: u64,
    pathname: Option<&'a str>,
}

impl<'a> TryFrom<&'a str> for MemMap<'a> {
    type Error = MapsError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
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
                    Some(parts[5])
                } else {
                    None
                },
            });
        }
        Err(MapsError::ParseError)
    }
}

// bufは/proc/self/mapの結果
// 最初の実行可能セグメントのベースアドレスを返す
pub fn get_exec_base(buf: &str) -> Result<u64, MapsError> {
    for l in buf.lines() {
        if let Ok(m) = TryInto::<MemMap>::try_into(l)
            && m.executable
        {
            return Ok(m.address.0);
        }
    }
    Err(MapsError::NotFound)
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
        assert!(m.readable);
        assert!(!m.writable);
        assert!(m.executable);
        assert!(!m.shared);
        assert!(m.private);
        assert_eq!(m.offset, 0);
        assert_eq!(m.device.0, 0x08);
        assert_eq!(m.device.1, 0x02);
        assert_eq!(m.inode, 173521);
        assert_eq!(m.pathname, Some("/usr/bin/dbus-daemon"));
    }
    #[test]
    fn test_memmap_parse_without_pathname() {
        let line = "7fff12340000-7fff12360000 rw-p 00000000 00:00 0";
        let m: MemMap = line.try_into().unwrap();
        assert!(m.pathname.is_none());
    }

    #[test]
    fn test_get_exec_base() {
        let result = get_exec_base(SAMPLE_LINE).unwrap();
        assert_eq!(result, 0x00400000);
    }
}
