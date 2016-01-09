use core::{fmt, ptr, slice, str};
use panic::panic_impl;
use env::{args_init, args_destroy};
use vec::Vec;

pub fn begin_unwind(string: &'static str, file_line: &(&'static str, u32)) -> ! {
    let &(file, line) = file_line;
    panic_impl(format_args!("{}", string), file, line)
}

pub fn begin_unwind_fmt(fmt: fmt::Arguments, file_line: &(&'static str, u32)) -> ! {
    let &(file, line) = file_line;
    panic_impl(fmt, file, line)
}

extern "C" {
    fn main();
}

#[no_mangle]
#[inline(never)]
pub unsafe extern "C" fn _start_stack(stack: *const usize) {
    let mut args: Vec<&'static str> = Vec::new();
    // TODO: Fix issue with stack not being in context VM space
    let argc = ptr::read(stack);
    for i in 0..argc as isize {
        let arg = ptr::read(stack.offset(1 + i)) as *const u8;
        if arg as usize > 0 {
            let mut len = 0;
            for j in 0..4096 {
                len = j;
                if ptr::read(arg.offset(j)) == 0 {
                    break;
                }
            }
            let utf8: &'static [u8] = slice::from_raw_parts(arg, len as usize);
            args.push(str::from_utf8_unchecked(utf8));
        }
    }

    args_init(args);
    main();
    args_destroy();
}

// #[lang = "start"]
// fn lang_start(main: *const u8, argc: isize, argv: *const *const u8) -> isize {
// unsafe {
// let mut args: Vec<&'static str> = Vec::new();
// TODO: Fix issue with stack not being in context VM space
// for i in 0..argc as isize {
// let arg = ptr::read(argv.offset(i)) as *const u8;
// if arg as usize > 0 {
// let mut len = 0;
// for j in 0..4096 /* Max arg length */ {
// len = j;
// if ptr::read(arg.offset(j)) == 0 {
// break;
// }
// }
// let utf8: &'static [u8] = slice::from_raw_parts(arg, len as usize);
// args.push(str::from_utf8_unchecked(utf8));
// }
// }
//
// args_init(args);
// mem::transmute::<_, fn()>(main)();
// args_destroy();
//
// 0
// }
// }
//
