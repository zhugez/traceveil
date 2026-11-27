use traceveil::{Error, Result};
pub fn ask<T, E>(run: impl FnOnce() -> std::result::Result<T, E>) -> Result<T> {
    let guard = Guard::new()?;
    let result = run().map_err(|_| Error::new("CANCELLED", "Selection cancelled."));
    drop(guard);
    result
}
#[cfg(unix)]
mod unix {
    use super::*;
    use std::sync::atomic::{AtomicPtr, Ordering};
    static TERMINAL: AtomicPtr<libc::termios> = AtomicPtr::new(std::ptr::null_mut());
    pub struct Guard {
        terminal: Box<libc::termios>,
        action: libc::sigaction,
    }
    extern "C" fn cancel(_: libc::c_int) {
        let terminal = TERMINAL.load(Ordering::Acquire);
        let message = b"\x1b[?25h\nCancelled.\n";
        unsafe {
            if !terminal.is_null() {
                libc::tcsetattr(0, libc::TCSANOW, terminal);
            }
            libc::write(2, message.as_ptr().cast(), message.len());
            libc::_exit(130);
        }
    }
    fn block() -> Result<libc::sigset_t> {
        let mut set = std::mem::MaybeUninit::uninit();
        let mut old = std::mem::MaybeUninit::uninit();
        unsafe {
            libc::sigemptyset(set.as_mut_ptr());
            libc::sigaddset(set.as_mut_ptr(), libc::SIGINT);
            if libc::pthread_sigmask(libc::SIG_BLOCK, set.as_ptr(), old.as_mut_ptr()) != 0 {
                return Err(Error::new(
                    "CANCELLED",
                    "The terminal interrupt mask is unavailable.",
                ));
            }
            Ok(old.assume_init())
        }
    }
    impl Guard {
        pub fn new() -> Result<Self> {
            let mut terminal = std::mem::MaybeUninit::uninit();
            if unsafe { libc::tcgetattr(0, terminal.as_mut_ptr()) } != 0 {
                return Err(Error::new(
                    "CANCELLED",
                    "The interactive terminal is unavailable.",
                ));
            }
            let mut terminal = Box::new(unsafe { terminal.assume_init() });
            let mask = block()?;
            if TERMINAL
                .compare_exchange(
                    std::ptr::null_mut(),
                    &mut *terminal,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                unsafe {
                    libc::pthread_sigmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut());
                }
                return Err(Error::new(
                    "CANCELLED",
                    "Another interactive prompt is active.",
                ));
            }
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = cancel as *const () as usize;
            unsafe {
                libc::sigemptyset(&mut action.sa_mask);
            }
            let mut old = std::mem::MaybeUninit::uninit();
            let status = unsafe { libc::sigaction(libc::SIGINT, &action, old.as_mut_ptr()) };
            if status != 0 {
                TERMINAL.store(std::ptr::null_mut(), Ordering::Release);
                unsafe {
                    libc::pthread_sigmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut());
                }
                return Err(Error::new(
                    "CANCELLED",
                    "The terminal interrupt handler is unavailable.",
                ));
            }
            let guard = Self {
                terminal,
                action: unsafe { old.assume_init() },
            };
            unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut());
            }
            Ok(guard)
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            let mask = block().ok();
            unsafe {
                libc::tcsetattr(0, libc::TCSANOW, &*self.terminal);
            }
            let term = console::Term::stderr();
            let _ = term.show_cursor();
            let _ = term.flush();
            unsafe {
                libc::sigaction(libc::SIGINT, &self.action, std::ptr::null_mut());
            }
            TERMINAL.store(std::ptr::null_mut(), Ordering::Release);
            if let Some(mask) = mask {
                unsafe {
                    libc::pthread_sigmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut());
                }
            }
        }
    }
}
#[cfg(unix)]
use unix::Guard;
#[cfg(not(unix))]
struct Guard;
#[cfg(not(unix))]
impl Guard {
    fn new() -> Result<Self> {
        Ok(Self)
    }
}
#[cfg(not(unix))]
impl Drop for Guard {
    fn drop(&mut self) {
        let term = console::Term::stderr();
        let _ = term.show_cursor();
        let _ = term.flush();
    }
}
