use alloc::arc::Arc;

use collections::string::{String, ToString};
use collections::vec::Vec;

use core::cell::UnsafeCell;
use core::ops::DerefMut;
use core::{mem, ptr};

use common::elf::Elf;
use common::memory;

use scheduler;
use scheduler::context::{CONTEXT_STACK_SIZE, CONTEXT_STACK_ADDR, context_switch,
                         context_userspace, Context, ContextMemory};

use schemes::Url;

/// Execute an executable
pub fn execute(url: Url, mut args: Vec<String>) {
    unsafe {



        if let Some(current) = Context::current_mut() {

            let reenable = scheduler::start_no_ints();
            let cptr = current.deref_mut();
            scheduler::end_no_ints(reenable);

            Context::spawn("kexec ".to_string() + &url.string,
                           box move || {
                               if let Some(mut resource) = url.open() {
                                   let mut vec: Vec<u8> = Vec::new();
                                   resource.read_to_end(&mut vec);

                                   let executable = Elf::from_data(vec.as_ptr() as usize);
                                   let entry = executable.entry();
                                   let mut memory = Vec::new();
                                   for segment in executable.load_segment().iter() {
                                       let virtual_address = segment.vaddr as usize;
                                       let virtual_size = segment.mem_len as usize;
                                       let physical_address = memory::alloc(virtual_size);

                                       if physical_address > 0 {
                                           // Copy progbits
                                           ::memcpy(physical_address as *mut u8,
                                     (executable.data + segment.off as usize) as *const u8,
                                     segment.file_len as usize);
                                           // Zero bss
                                           ::memset((physical_address + segment.file_len as usize) as *mut u8,
                            0,
                            segment.mem_len as usize - segment.file_len as usize);

                                           memory.push(ContextMemory {
                                               physical_address: physical_address,
                                               virtual_address: virtual_address,
                                               virtual_size: virtual_size,
                                               writeable: segment.flags & 2 == 2,
                                           });
                                       }
                                   }

                                   if entry > 0 && !memory.is_empty() {
                                       args.insert(0, url.to_string());

                                       let mut context_args: Vec<usize> = Vec::new();
                                       context_args.push(0); // ENVP
                                       context_args.push(0); // ARGV NULL
                                       let mut argc = 0;
                                       for i in 0..args.len() {
                                           if let Some(arg) = args.get(args.len() - i - 1) {
                                               context_args.push(arg.as_ptr() as usize);
                                               argc += 1;
                                           }
                                       }
                                       context_args.push(argc);

                                       let reenable = scheduler::start_no_ints();

                                       let context = cptr;

                                       context.name = url.to_string();

                                       context.sp = context.kernel_stack + CONTEXT_STACK_SIZE - 128;

                                       context.stack = Some(ContextMemory {
                                           physical_address: memory::alloc(CONTEXT_STACK_SIZE),
                                           virtual_address: CONTEXT_STACK_ADDR,
                                           virtual_size: CONTEXT_STACK_SIZE,
                                           writeable: true,
                                       });

                                       context.args = Arc::new(UnsafeCell::new(args));
                                       context.cwd = Arc::new(UnsafeCell::new((*context.cwd
                                                                                       .get())
                                                                                  .clone()));
                                       context.memory = Arc::new(UnsafeCell::new(memory));

                                       let user_sp = if let Some(ref stack) = context.stack {
                                           let mut sp = stack.physical_address +
                                                        stack.virtual_size -
                                                        128;
                                           for arg in context_args.iter() {
                                               sp -= mem::size_of::<usize>();
                                               ptr::write(sp as *mut usize, *arg);
                                           }
                                           sp - stack.physical_address + stack.virtual_address
                                       } else {
                                           0
                                       };

                                       context.push(0x20 | 3);
                                       context.push(user_sp);
                                       context.push(1 << 9);
                                       context.push(0x18 | 3);
                                       context.push(entry);
                                       context.push(context_userspace as usize);

                                       scheduler::end_no_ints(reenable);
                                   } else {
                                       debug!("{}: Invalid memory or entry\n", url.string);
                                   }
                               } else {
                                   debug!("{}: Failed to open\n", url.string);
                               }
                           });

            loop {
                context_switch(false);
            }
        }
    }
}
