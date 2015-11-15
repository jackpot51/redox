use super::syscall::sys_debug;

/// Debug new line to console
#[macro_export]
macro_rules! debugln {
    ($($arg:tt)*) => ({
        debug!($($arg)*);
        debug!("\n");
    });
}

/// Debug to console
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => ({
        $crate::debug::debug(&format!($($arg)*));
    });
}

pub fn debug(msg: &str) {
    unsafe {
        sys_debug(msg.as_ptr(), msg.len());
    }
}
