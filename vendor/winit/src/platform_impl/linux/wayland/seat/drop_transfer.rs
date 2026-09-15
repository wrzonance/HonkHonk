use std::io::{self, Read};
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const LIMIT: usize = 1024 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(5);

pub fn accepts_copy(version: u32, copy_selected: bool) -> bool {
    version < 3 || copy_selected
}

pub fn paths(bytes: &[u8]) -> Vec<PathBuf> {
    bytes
        .split(|b| *b == b'\n')
        .filter_map(local_path)
        .take(1000)
        .collect()
}

fn local_path(line: &[u8]) -> Option<PathBuf> {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let rest = line.strip_prefix(b"file://")?;
    let path = rest.strip_prefix(b"localhost").unwrap_or(rest);
    if !path.starts_with(b"/") || path.contains(&b'?') || path.contains(&b'#') {
        return None;
    }
    let mut decoded = Vec::new();
    let mut index = 0;
    while index < path.len() {
        let byte = if path[index] == b'%' {
            let hex = std::str::from_utf8(path.get(index + 1..index + 3)?).ok()?;
            index += 2;
            u8::from_str_radix(hex, 16).ok()?
        } else {
            path[index]
        };
        if byte == 0 || byte.is_ascii_control() {
            return None;
        }
        decoded.push(byte);
        index += 1;
    }
    Some(std::ffi::OsString::from_vec(decoded).into())
}

pub struct Transfer {
    bytes: Vec<u8>,
    started: Instant,
}

impl Transfer {
    pub fn new(started: Instant) -> Self {
        Self {
            bytes: Vec::new(),
            started,
        }
    }

    pub fn poll(
        &mut self,
        reader: &mut impl Read,
        now: Instant,
    ) -> io::Result<Option<Vec<PathBuf>>> {
        if now.saturating_duration_since(self.started) >= TIMEOUT {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let mut buffer = [0; 4096];
        for _ in 0..8 {
            match reader.read(&mut buffer) {
                Ok(0) => return Ok(Some(paths(&self.bytes))),
                Ok(count) => {
                    if self.bytes.len() + count > LIMIT {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    self.bytes.extend_from_slice(&buffer[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }
}
