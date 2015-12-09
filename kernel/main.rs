#![crate_type="staticlib"]
#![feature(alloc)]
#![feature(allocator)]
#![feature(arc_counts)]
#![feature(asm)]
#![feature(box_syntax)]
#![feature(collections)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![feature(core_str_ext)]
#![feature(core_slice_ext)]
#![feature(fnbox)]
#![feature(fundamental)]
#![feature(lang_items)]
#![feature(no_std)]
#![feature(unboxed_closures)]
#![feature(unsafe_no_drop_flag)]
#![feature(unwind_attributes)]
#![feature(vec_push_all)]
#![no_std]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate collections;

use alloc::boxed::Box;

use collections::string::{String, ToString};
use collections::vec::Vec;

use core::cell::UnsafeCell;
use core::{mem, usize};
use core::slice::SliceExt;

use common::debug;
use common::event::{self, EVENT_KEY, EventOption};
use common::memory;
use common::paging::Page;
use common::time::Duration;

use drivers::pci::*;
use drivers::pio::*;
use drivers::ps2::*;
use drivers::rtc::*;
use drivers::serial::*;

use env::Environment;

pub use externs::*;

use graphics::display;

use programs::executor::execute;
use programs::scheme::*;

use scheduler::{Context, Regs, TSS};
use scheduler::context::{context_enabled, context_switch, context_i, context_pid};

use schemes::Url;
use schemes::arp::*;
use schemes::context::*;
use schemes::debug::*;
use schemes::ethernet::*;
use schemes::icmp::*;
use schemes::ip::*;
use schemes::memory::*;
// use schemes::display::*;

use syscall::handle::*;

/// Common std-like functionality
#[macro_use]
pub mod common;
/// Allocation
pub mod alloc_system;
/// Audio
pub mod audio;
/// Various drivers
/// TODO: Move out of kernel space (like other microkernels)
pub mod drivers;
/// Environment
pub mod env;
/// Externs
pub mod externs;
/// Various graphical methods
pub mod graphics;
/// Network
pub mod network;
/// Panic
pub mod panic;
/// Programs
pub mod programs;
/// Schemes
pub mod schemes;
/// Scheduling
pub mod scheduler;
/// Sync primatives
pub mod sync;
/// System calls
pub mod syscall;
/// USB input/output
pub mod usb;

pub static mut TSS_PTR: Option<&'static mut TSS> = None;
pub static mut ENV_PTR: Option<&'static mut Environment> = None;

pub fn env() -> &'static Environment {
    unsafe {
        match ENV_PTR {
            Some(&mut ref p) => p,
            None => unreachable!(),
        }
    }
}

/// Pit duration
static PIT_DURATION: Duration = Duration {
    secs: 0,
    nanos: 2250286,
};

/// Idle loop (active while idle)
unsafe fn idle_loop() -> ! {
    loop {
        asm!("cli" : : : : "intel", "volatile");

        let mut halt = true;

        {
            let contexts = ::env().contexts.lock();
            for i in 1..contexts.len() {
                if let Some(context) = contexts.get(i) {
                    if context.interrupted {
                        halt = false;
                        break;
                    }
                }
            }
        }

        if halt {
            asm!("sti" : : : : "intel", "volatile");
            asm!("hlt" : : : : "intel", "volatile");
        } else {
            asm!("sti" : : : : "intel", "volatile");
        }

        context_switch(false);
    }
}

/// Event poll loop
fn poll_loop() -> ! {
    loop {
        env().on_poll();

        unsafe { context_switch(false) };
    }
}

/// Event loop
fn event_loop() -> ! {
    let mut cmd = String::new();
    loop {
        loop {
            let mut console = env().console.lock();
            match env().events.lock().pop_front() {
                Some(event) => {
                    if console.draw {
                        match event.to_option() {
                            EventOption::Key(key_event) => {
                                if key_event.pressed {
                                    match key_event.scancode {
                                        event::K_F2 => {
                                            console.draw = false;
                                        }
                                        event::K_BKSP => {
                                            if !cmd.is_empty() {
                                                console.write(&[8]);
                                                cmd.pop();
                                            }
                                        }
                                        _ => {
                                            match key_event.character {
                                                '\0' => (),
                                                '\n' => {
                                                    console.command = Some(cmd.clone());

                                                    cmd.clear();
                                                    console.write(&[10]);
                                                }
                                                _ => {
                                                    cmd.push(key_event.character);
                                                    console.write(&[key_event.character as u8]);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => (),
                        }
                    } else {
                        if event.code == EVENT_KEY && event.b as u8 == event::K_F1 && event.c > 0 {
                            console.draw = true;
                            console.redraw = true;
                        } else {
                            // TODO: Magical orbital hack
                            unsafe {
                                let reenable = scheduler::start_no_ints();
                                for scheme in env().schemes.iter() {
                                    if (*scheme.get()).scheme() == "orbital" {
                                        (*scheme.get()).event(&event);
                                        break;
                                    }
                                }
                                scheduler::end_no_ints(reenable);
                            }
                        }
                    }
                }
                None => break,
            }
        }

        {
            let mut console = env().console.lock();
            if console.draw && console.redraw {
                console.redraw = false;
                console.display.flip();
            }
        }

        unsafe { context_switch(false) };
    }
}

/// Initialize kernel
unsafe fn init(font_data: usize, tss_data: usize) {
    Page::init();
    memory::cluster_init();
    // Unmap first page to catch null pointer errors (after reading memory map)
    Page::new(0).unmap();

    sync::intex::intex_count = 0;

    display::fonts = font_data;
    TSS_PTR = Some(&mut *(tss_data as *mut TSS));
    ENV_PTR = Some(&mut *Box::into_raw(Environment::new()));

    context_pid = 1;
    context_i = 0;
    context_enabled = false;

    match ENV_PTR {
        Some(ref mut env) => {
            env.contexts.lock().push(Context::root());
            env.console.lock().draw = true;

            debug!("Redox ");
            debug::dd(mem::size_of::<usize>() * 8);
            debug!(" bits");
            debug::dl();

            env.clock_realtime = Rtc::new().time();

            env.schemes.push(UnsafeCell::new(Ps2::new()));
            env.schemes.push(UnsafeCell::new(Serial::new(0x3F8, 0x4)));

            pci_init(env);

            env.schemes.push(UnsafeCell::new(DebugScheme::new()));
            env.schemes.push(UnsafeCell::new(box ContextScheme));
            env.schemes.push(UnsafeCell::new(box MemoryScheme));
            // session.items.push(box RandomScheme);
            // session.items.push(box TimeScheme);

            env.schemes.push(UnsafeCell::new(box EthernetScheme));
            env.schemes.push(UnsafeCell::new(box ArpScheme));
            env.schemes.push(UnsafeCell::new(box IcmpScheme));
            env.schemes.push(UnsafeCell::new(box IpScheme { arp: Vec::new() }));
            // session.items.push(box DisplayScheme);

            Context::spawn("kpoll".to_string(),
                           box move || {
                               poll_loop();
                           });

            Context::spawn("kevent".to_string(),
                           box move || {
                               event_loop();
                           });

            Context::spawn("karp".to_string(),
                           box move || {
                               ArpScheme::reply_loop();
                           });

            Context::spawn("kicmp".to_string(),
                           box move || {
                               IcmpScheme::reply_loop();
                           });

            context_enabled = true;

            if let Some(mut resource) = Url::from_str("file:/schemes/").open() {
                let mut vec: Vec<u8> = Vec::new();
                resource.read_to_end(&mut vec);

                for folder in String::from_utf8_unchecked(vec).lines() {
                    if folder.ends_with('/') {
                        let scheme_item = SchemeItem::from_url(&Url::from_string("file:/schemes/"
                                                                                     .to_string() +
                                                                                 &folder));

                        let reenable = scheduler::start_no_ints();
                        env.schemes.push(UnsafeCell::new(scheme_item));
                        scheduler::end_no_ints(reenable);
                    }
                }
            }

            Context::spawn("kinit".to_string(),
                           box move || {
                               let wd_c = "file:/\0";
                               do_sys_chdir(wd_c.as_ptr());

                               let stdio_c = "debug:\0";
                               do_sys_open(stdio_c.as_ptr(), 0);
                               do_sys_open(stdio_c.as_ptr(), 0);
                               do_sys_open(stdio_c.as_ptr(), 0);

                               let path_string = "file:/apps/login/main.bin";
                               let path = Url::from_str(path_string);

                               debug!("INIT: Executing {}\n", path_string);
                               execute(path, Vec::new());
                               debug!("INIT: Failed to execute\n");

                               loop {
                                   context_switch(false);
                               }
                           });
        }
        None => unreachable!(),
    }
}

#[cold]
#[inline(never)]
#[no_mangle]
/// Take regs for kernel calls and exceptions
pub extern "cdecl" fn kernel(interrupt: usize, mut regs: &mut Regs) {
    macro_rules! exception_inner {
        ($name:expr) => ({
            unsafe {
                if let Some(context) = Context::current() {
                    debugln!("PID {}: {}", context.pid, context.name);
                }
            }

            debugln!("  INT {:X}: {}", interrupt, $name);
            debugln!("    CS:  {:08X}    IP:  {:08X}    FLG: {:08X}", regs.cs, regs.ip, regs.flags);
            debugln!("    SS:  {:08X}    SP:  {:08X}    BP:  {:08X}", regs.ss, regs.sp, regs.bp);
            debugln!("    AX:  {:08X}    BX:  {:08X}    CX:  {:08X}    DX:  {:08X}", regs.ax, regs.bx, regs.cx, regs.dx);
            debugln!("    DI:  {:08X}    SI:  {:08X}", regs.di, regs.di);

            let cr0: usize;
            let cr2: usize;
            let cr3: usize;
            let cr4: usize;
            unsafe {
                asm!("mov $0, cr0" : "=r"(cr0) : : : "intel", "volatile");
                asm!("mov $0, cr2" : "=r"(cr2) : : : "intel", "volatile");
                asm!("mov $0, cr3" : "=r"(cr3) : : : "intel", "volatile");
                asm!("mov $0, cr4" : "=r"(cr4) : : : "intel", "volatile");
            }
            debugln!("    CR0: {:08X}    CR2: {:08X}    CR3: {:08X}    CR4: {:08X}", cr0, cr2, cr3, cr4);
        })
    };

    macro_rules! exception {
        ($name:expr) => ({
            exception_inner!($name);

            loop {
                unsafe { do_sys_exit(usize::MAX) };
            }
        })
    };

    macro_rules! exception_error {
        ($name:expr) => ({
            let error = regs.ip;
            regs.ip = regs.cs;
            regs.cs = regs.flags;
            regs.flags = regs.sp;
            regs.sp = regs.ss;
            regs.ss = 0;
            //regs.ss = regs.error;

            exception_inner!($name);
            debugln!("    ERR: {:08X}", error);

            loop {
                unsafe { do_sys_exit(usize::MAX) };
            }
        })
    };

    if interrupt >= 0x20 && interrupt < 0x30 {
        if interrupt >= 0x28 {
            unsafe { Pio8::new(0xA0).write(0x20) };
        }

        unsafe { Pio8::new(0x20).write(0x20) };
    }

    match interrupt {
        0x20 => unsafe {
            let reenable = scheduler::start_no_ints();

            match ENV_PTR {
                Some(ref mut env) => {
                    env.clock_realtime = env.clock_realtime + PIT_DURATION;
                    env.clock_monotonic = env.clock_monotonic + PIT_DURATION;

                    scheduler::end_no_ints(reenable);

                    let switch = if let Some(mut context) = Context::current_mut() {
                        context.slices -= 1;
                        context.slice_total += 1;
                        context.slices == 0
                    } else {
                        false
                    };

                    if switch {
                        context_switch(true);
                    }
                }
                None => unreachable!(),
            }
        },
        0x21 => env().on_irq(0x1), // keyboard
        0x23 => env().on_irq(0x3), // serial 2 and 4
        0x24 => env().on_irq(0x4), // serial 1 and 3
        0x25 => env().on_irq(0x5), //parallel 2
        0x26 => env().on_irq(0x6), //floppy
        0x27 => env().on_irq(0x7), //parallel 1 or spurious
        0x28 => env().on_irq(0x8), //RTC
        0x29 => env().on_irq(0x9), //pci
        0x2A => env().on_irq(0xA), //pci
        0x2B => env().on_irq(0xB), //pci
        0x2C => env().on_irq(0xC), //mouse
        0x2D => env().on_irq(0xD), //coprocessor
        0x2E => env().on_irq(0xE), //disk
        0x2F => env().on_irq(0xF), //disk
        0x80 => {
            if !unsafe { syscall_handle(regs) } {
                exception!("Unknown Syscall");
            }
        }
        0xFF => unsafe {
            init(regs.ax, regs.bx);
            idle_loop();
        },
        0x0 => exception!("Divide by zero exception"),
        0x1 => exception!("Debug exception"),
        0x2 => exception!("Non-maskable interrupt"),
        0x3 => exception!("Breakpoint exception"),
        0x4 => exception!("Overflow exception"),
        0x5 => exception!("Bound range exceeded exception"),
        0x6 => exception!("Invalid opcode exception"),
        0x7 => exception!("Device not available exception"),
        0x8 => exception_error!("Double fault"),
        0xA => exception_error!("Invalid TSS exception"),
        0xB => exception_error!("Segment not present exception"),
        0xC => exception_error!("Stack-segment fault"),
        0xD => exception_error!("General protection fault"),
        0xE => exception_error!("Page fault"),
        0x10 => exception!("x87 floating-point exception"),
        0x11 => exception_error!("Alignment check exception"),
        0x12 => exception!("Machine check exception"),
        0x13 => exception!("SIMD floating-point exception"),
        0x14 => exception!("Virtualization exception"),
        0x1E => exception_error!("Security exception"),
        _ => exception!("Unknown Interrupt"),
    }
}
