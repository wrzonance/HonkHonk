#[path = "../vendor/winit/src/platform_impl/linux/wayland/seat/drop_transfer.rs"]
mod transfer;

use std::io::{self, Cursor, Read};
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn only_local_file_uris_are_accepted() {
    let data = b"# comment\r\nfile:///tmp/a%20b\r\nfile://localhost/tmp/c\nfile://remote/tmp/d\nhttps://x/a\nfile:///tmp/bad%ZZ\nfile:///tmp/a%00b\nfile:///tmp/a?query\n";
    assert_eq!(
        transfer::paths(data),
        vec![PathBuf::from("/tmp/a b"), PathBuf::from("/tmp/c")]
    );
}

#[test]
fn completed_transfer_returns_paths_once_eof_arrives() {
    let now = Instant::now();
    let mut state = transfer::Transfer::new(now);
    assert_eq!(
        state
            .poll(&mut Cursor::new(b"file:///tmp/a\n"), now)
            .unwrap(),
        Some(vec!["/tmp/a".into()])
    );
}

struct Waiting;
impl Read for Waiting {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}

#[test]
fn stalled_transfer_yields_then_times_out() {
    let now = Instant::now();
    let mut state = transfer::Transfer::new(now);
    assert!(state.poll(&mut Waiting, now).unwrap().is_none());
    assert_eq!(
        state
            .poll(&mut Waiting, now + transfer::TIMEOUT)
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
}

#[test]
fn oversized_transfer_is_rejected_without_unbounded_reads() {
    let now = Instant::now();
    let mut state = transfer::Transfer::new(now);
    let mut reader = io::repeat(b'x');
    for _ in 0..100 {
        if let Err(error) = state.poll(&mut reader, now) {
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            return;
        }
    }
    panic!("oversized transfer was not rejected");
}

#[test]
fn path_count_is_bounded_and_non_utf8_paths_are_preserved() {
    use std::os::unix::ffi::OsStrExt;
    assert_eq!(
        transfer::paths(b"file:///tmp/%FF")[0]
            .as_os_str()
            .as_bytes(),
        b"/tmp/\xff"
    );
    assert!(transfer::paths("file:///tmp/a\n".repeat(1500).as_bytes()).len() <= 1000);
}

#[test]
fn protocol_v3_requires_copy_negotiation_before_finishing_a_drop() {
    assert!(transfer::accepts_copy(2, false));
    assert!(transfer::accepts_copy(3, true));
    assert!(!transfer::accepts_copy(3, false));
}
