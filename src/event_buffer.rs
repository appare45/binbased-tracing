use std::num::NonZeroUsize;
use std::os::fd::{AsFd, AsRawFd, OwnedFd, RawFd};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nix::sys::memfd::{MFdFlags, memfd_create};
use nix::sys::mman::{MapFlags, ProtFlags, mmap, munmap};
use nix::unistd::ftruncate;

use crate::error::EventBufferError;
use crate::event::TraceEvent;

const BUFFER_SIZE: usize = 4096;
pub const BUFFER_CAPACITY: u64 = 128;
const BUFFER_HEADER_SIZE: usize = 64;
const TRACE_EVENT_SIZE: usize = 24;

pub struct EventBuffer {
    ptr: ptr::NonNull<u8>,
    fd: OwnedFd,
    stop: Arc<AtomicBool>,
    reader: Option<JoinHandle<u64>>,
}

unsafe impl Send for EventBuffer {}

impl EventBuffer {
    pub fn create() -> Result<Self, EventBufferError> {
        // O_CLOEXEC なしで作成し、子プロセスに fd を引き継ぐ
        let fd = memfd_create(c"tracer_shm", MFdFlags::empty()).map_err(EventBufferError::MemfdCreateFailed)?;

        ftruncate(&fd, BUFFER_SIZE as nix::libc::off_t).map_err(EventBufferError::FtruncateFailed)?;

        let ptr = unsafe {
            mmap(
                None,
                NonZeroUsize::new(BUFFER_SIZE).unwrap(),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                &fd,
                0,
            )
            .map_err(EventBufferError::MmapFailed)?
        };

        unsafe { ptr::write_volatile(ptr.cast().as_ptr(), 0u64) };

        Ok(Self {
            ptr: ptr.cast(),
            fd,
            stop: Arc::new(AtomicBool::new(false)),
            reader: None,
        })
    }

    pub fn fd(&self) -> RawFd {
        self.fd.as_fd().as_raw_fd()
    }

    pub fn start_reader(&mut self, tx: Sender<TraceEvent>) {
        let ptr_addr = self.ptr.as_ptr() as usize;
        let stop = Arc::clone(&self.stop);

        self.reader = Some(thread::spawn(move || {
            let ptr = ptr_addr as *mut u8;
            let write_pos_atomic = unsafe { &*(ptr as *const AtomicU64) };
            let mut read_pos: u64 = 0;
            let mut counter: u64 = 0;

            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }

                let write_pos = write_pos_atomic.load(Ordering::Acquire);

                while read_pos < write_pos {
                    let idx = (read_pos % BUFFER_CAPACITY) as usize;
                    let event_ptr = unsafe {
                        ptr.add(BUFFER_HEADER_SIZE + idx * TRACE_EVENT_SIZE) as *const TraceEvent
                    };
                    let event = unsafe { ptr::read_volatile(event_ptr) };

                    counter += 1;
                    if tx.send(event).is_err() {
                        return counter;
                    }
                    read_pos += 1;
                }

                thread::sleep(Duration::from_micros(100));
            }

            counter
        }));
    }
}

impl Drop for EventBuffer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
        unsafe {
            let _ = munmap(self.ptr.cast(), BUFFER_SIZE);
        }
    }
}
