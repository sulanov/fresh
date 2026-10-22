//! Cross-thread event wakeup mechanism.
//!
//! Provides a lightweight notification pipe (on Unix) or atomic signal (on other
//! platforms) that allows background threads (LSP clients, terminal processes,
//! plugins, and local control channels) to interrupt a sleeping event loop
//! immediately when new async messages arrive, without requiring high-frequency
//! periodic polling.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::io::RawFd;

#[derive(Debug)]
struct Inner {
    #[cfg(unix)]
    read_fd: RawFd,
    #[cfg(unix)]
    write_fd: RawFd,
    notified: AtomicBool,
}

#[cfg(unix)]
impl Inner {
    fn new() -> io::Result<Self> {
        let mut fds = [0 as RawFd; 2];
        let res = unsafe {
            #[cfg(target_os = "linux")]
            {
                libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK)
            }
            #[cfg(not(target_os = "linux"))]
            {
                let ret = libc::pipe(fds.as_mut_ptr());
                if ret == 0 {
                    for fd in fds {
                        let flags = libc::fcntl(fd, libc::F_GETFL);
                        if flags >= 0 {
                            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                        }
                        let fd_flags = libc::fcntl(fd, libc::F_GETFD);
                        if fd_flags >= 0 {
                            libc::fcntl(fd, libc::F_SETFD, fd_flags | libc::FD_CLOEXEC);
                        }
                    }
                }
                ret
            }
        };
        if res != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            read_fd: fds[0],
            write_fd: fds[1],
            notified: AtomicBool::new(false),
        })
    }
}

#[cfg(not(unix))]
impl Inner {
    fn new() -> io::Result<Self> {
        Ok(Self {
            notified: AtomicBool::new(false),
        })
    }
}

#[cfg(unix)]
impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.read_fd);
            libc::close(self.write_fd);
        }
    }
}

/// A cloneable handle that background threads use to wake up the main event loop.
#[derive(Clone, Debug)]
pub struct WakeupNotifier {
    inner: Arc<Inner>,
}

impl WakeupNotifier {
    /// Signal the wakeup receiver. If a notification is already pending,
    /// this coalesces and avoids redundant writes.
    pub fn notify(&self) {
        if !self.inner.notified.swap(true, Ordering::Release) {
            #[cfg(unix)]
            unsafe {
                let byte = 1u8;
                let _ = libc::write(
                    self.inner.write_fd,
                    std::ptr::addr_of!(byte).cast::<libc::c_void>(),
                    1,
                );
            }
        }
    }
}

/// The receiver side of the wakeup mechanism, owned by the main thread / event loop.
#[derive(Clone, Debug)]
pub struct WakeupReceiver {
    inner: Arc<Inner>,
}

impl WakeupReceiver {
    /// Drain any pending wakeup signal and reset the notification state.
    pub fn drain(&self) {
        self.inner.notified.store(false, Ordering::Release);
        #[cfg(unix)]
        unsafe {
            let mut buf = [0u8; 64];
            while libc::read(
                self.inner.read_fd,
                buf.as_mut_ptr().cast::<libc::c_void>(),
                buf.len(),
            ) > 0
            {}
        }
    }

    /// The raw file descriptor of the read end of the pipe (Unix only).
    #[cfg(unix)]
    pub fn raw_fd(&self) -> RawFd {
        self.inner.read_fd
    }

    /// Whether a notification is currently pending.
    pub fn is_notified(&self) -> bool {
        self.inner.notified.load(Ordering::Acquire)
    }

    /// Create a new notifier linked to this receiver.
    pub fn notifier(&self) -> WakeupNotifier {
        WakeupNotifier {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Create a linked (WakeupNotifier, WakeupReceiver) pair.
pub fn create_wakeup_pair() -> io::Result<(WakeupNotifier, WakeupReceiver)> {
    let inner = Arc::new(Inner::new()?);
    Ok((
        WakeupNotifier {
            inner: Arc::clone(&inner),
        },
        WakeupReceiver { inner },
    ))
}

pub use create_wakeup_pair as wakeup_channel;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wakeup_notification_and_drain() {
        let (notifier, receiver) = create_wakeup_pair().expect("create_wakeup_pair");
        assert!(!receiver.is_notified());

        notifier.notify();
        assert!(receiver.is_notified());

        // Redundant notify coalesces
        notifier.notify();
        assert!(receiver.is_notified());

        receiver.drain();
        assert!(!receiver.is_notified());
    }
}
