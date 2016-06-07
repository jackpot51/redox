pub use system::error::*;
pub use system::syscall::*;

use arch::regs::Regs;
use arch::context::context_switch;

pub mod debug;
pub mod execute;
pub mod fs;
pub mod memory;
pub mod process;
pub mod time;

/// Handle the syscall defined by the given registers.
///
/// AX defines which syscall to use. The arguments are provided in other registers, as specified by
/// the specific sycall.
///
/// The return value is placed in AX, unless otherwise specified.
pub fn handle(regs: &mut Regs) {
    {
        let mut contexts = ::env().contexts.lock();
        if let Ok(cur) = contexts.current_mut() {
            cur.current_syscall = Some((regs.ip, regs.ax, regs.bx, regs.cx, regs.dx));
            //serial_log(&format!("PID {}: {} @ {:X}: {} {:X} {:X} {:X}\n", cur.pid, cur.name, regs.ip, regs.ax, regs.bx, regs.cx, regs.dx).as_bytes());
            if cur.supervised {
                // Block the process.
                cur.blocked_syscall = true;
                cur.blocked = true;
                // Clear the timer.
                cur.wake = None;

                loop {
                    if cur.blocked {
                        unsafe { context_switch() };
                    } else {
                        return;
                    }
                }
            }
        }
    }

    regs.ax = Error::mux(match regs.ax {
        // These are arranged in such a way that the most frequent syscalls preceeds less frequent
        // once, to acheive the best performance.

        SYS_YIELD => process::sched_yield(),
        SYS_WRITE => fs::write(regs.bx, regs.cx as *mut u8, regs.dx),
        SYS_READ => fs::read(regs.bx, regs.cx as *mut u8, regs.dx),
        SYS_LSEEK => fs::lseek(regs.bx, regs.cx as isize, regs.dx),
        SYS_OPEN => fs::open(regs.bx as *const u8, regs.cx),
        SYS_CLOSE => fs::close(regs.bx),
        SYS_CLONE => process::clone(regs),
        SYS_MKDIR => fs::mkdir(regs.bx as *const u8, regs.cx),
        SYS_NANOSLEEP => time::nanosleep(regs.bx as *const TimeSpec, regs.cx as *mut TimeSpec),
        SYS_FPATH => fs::fpath(regs.bx, regs.cx as *mut u8, regs.dx),
        SYS_FSTAT => fs::fstat(regs.bx, regs.cx as *mut Stat),
        SYS_FSYNC => fs::fsync(regs.bx),
        SYS_FTRUNCATE => fs::ftruncate(regs.bx, regs.cx),
        SYS_DEBUG => debug::debug(regs.bx as *const u8, regs.cx),
        SYS_DUP => fs::dup(regs.bx),
        SYS_IOPL => process::iopl(regs),
        SYS_CLOCK_GETTIME => time::clock_gettime(regs.bx, regs.cx as *mut TimeSpec),
        SYS_EXECVE => process::execve(regs.bx as *const u8, regs.cx as *const *const u8),
        SYS_EXIT => process::exit(regs.bx),
        SYS_GETPID => process::getpid(),
        // TODO: link
        SYS_PIPE2 => fs::pipe2(regs.bx as *mut usize, regs.cx),
        SYS_RMDIR => fs::rmdir(regs.bx as *const u8),
        SYS_STAT => fs::stat(regs.bx as *const u8, regs.cx as *mut Stat),
        SYS_UNLINK => fs::unlink(regs.bx as *const u8),
        SYS_WAITPID => process::waitpid(regs.bx as isize, regs.cx as *mut usize, regs.dx),
        SYS_BRK => memory::brk(regs.bx),
        SYS_CHDIR => fs::chdir(regs.bx as *const u8),
        SYS_SUPERVISE => process::supervise(regs.bx),
        _ => Err(Error::new(ENOSYS)),
    });


    {
        let mut contexts = ::env().contexts.lock();
        if let Ok(cur) = contexts.current_mut() {
            cur.current_syscall = None;
        }
    }
}
