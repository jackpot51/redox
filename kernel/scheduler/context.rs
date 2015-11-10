use common::get_slice::GetSlice;

use alloc::boxed::{Box, FnBox};
use alloc::rc::Rc;

use collections::string::{String, ToString};
use collections::vec::Vec;

use core::cell::UnsafeCell;
use core::{mem, ptr};

use common::memory;
use common::paging::Page;
use scheduler::{self, Regs, TSS};

use schemes::Resource;

use syscall::common::{CLONE_FILES, CLONE_FS, CLONE_VM};

pub const CONTEXT_STACK_SIZE: usize = 1024 * 1024;
pub const CONTEXT_STACK_ADDR: usize = 0xC0000000 - CONTEXT_STACK_SIZE;

pub static mut contexts_ptr: *mut Vec<Box<Context>> = 0 as *mut Vec<Box<Context>>;
pub static mut context_i: usize = 0;
pub static mut context_enabled: bool = false;

/// Switch context
///
/// Unsafe due to interrupt disabling, raw pointers, and unsafe Context functions
pub unsafe fn context_switch(interrupted: bool) {
    let reenable = scheduler::start_no_ints();

    let contexts = &mut *contexts_ptr;
    if context_enabled {
        let current_i = context_i;
        context_i += 1;
        // The only garbage collection in Redox
        loop {
            if context_i >= contexts.len() {
                context_i -= contexts.len();
            }

            let mut remove = false;
            if let Some(next) = contexts.get(context_i) {
                if next.exited {
                    remove = true;
                }
            }

            if remove {
                drop(contexts.remove(context_i));
            } else {
                break;
            }
        }

        if context_i >= contexts.len() {
            context_i -= contexts.len();
        }

        if context_i != current_i {
            if let Some(current) = contexts.get(current_i) {
                if let Some(next) = contexts.get(context_i) {
                    let current_ptr: *mut Box<Context> = mem::transmute(current as *const Box<Context>);
                    let next_ptr: *mut Box<Context> = mem::transmute(next as *const Box<Context>);

                    (*current_ptr).interrupted = interrupted;
                    (*next_ptr).interrupted = false;

                    (*current_ptr).save();
                    (*current_ptr).unmap();

                    if (*next_ptr).kernel_stack > 0 {
                        (*::tss_ptr).esp0 = ((*next_ptr).kernel_stack + CONTEXT_STACK_SIZE - 128) as u32;
                    } else {
                        (*::tss_ptr).esp0 = 0x200000 - 128;
                    }

                    (*next_ptr).map();
                    (*next_ptr).restore();
                }
            }
        }
    }

    scheduler::end_no_ints(reenable);
}

//TODO: To clean up memory leak, current must be destroyed!
/// Exit context
///
/// Unsafe due to interrupt disabling and raw pointers
pub unsafe fn context_exit() {
    let reenable = scheduler::start_no_ints();

    if let Some(mut current) = Context::current_mut() {
        current.exited = true;
    }

    scheduler::end_no_ints(reenable);

    context_switch(false);
}

/// Clone context
///
/// Unsafe due to interrupt disabling, C memory handling, and raw pointers
pub unsafe extern "cdecl" fn context_clone(parent_ptr: *const Context, flags: usize){
    let reenable = scheduler::start_no_ints();

    let kernel_stack = memory::alloc(CONTEXT_STACK_SIZE + 512);
    if kernel_stack > 0 {
        let parent = &*parent_ptr;

        ::memcpy(kernel_stack as *mut u8, parent.kernel_stack as *const u8, CONTEXT_STACK_SIZE + 512);

        let mut context = box Context {
            interrupted: parent.interrupted,
            exited: parent.exited,

            kernel_stack: kernel_stack,
            sp: parent.sp - parent.kernel_stack + kernel_stack,
            flags: parent.flags,
            fx: kernel_stack + CONTEXT_STACK_SIZE,
            stack: if let Some(ref entry) = parent.stack {
                let physical_address = memory::alloc(entry.virtual_size);
                if physical_address > 0 {
                    ::memcpy(physical_address as *mut u8, entry.physical_address as *const u8, entry.virtual_size);
                    Some(ContextMemory {
                        physical_address: physical_address,
                        virtual_address: entry.virtual_address,
                        virtual_size: entry.virtual_size,
                    })
                } else {
                    None
                }
            } else {
                None
            },
            loadable: parent.loadable,

            args: parent.args.clone(),
            cwd: if flags & CLONE_FS == CLONE_FS {
                parent.cwd.clone()
            } else {
                Rc::new(UnsafeCell::new((*parent.cwd.get()).clone()))
            },
            memory: if flags & CLONE_VM == CLONE_VM {
                parent.memory.clone()
            } else {
                let mut mem: Vec<ContextMemory> = Vec::new();
                for entry in (*parent.memory.get()).iter() {
                    let physical_address = memory::alloc(entry.virtual_size);
                    if physical_address > 0 {
                        ::memcpy(physical_address as *mut u8, entry.physical_address as *const u8, entry.virtual_size);
                        mem.push(ContextMemory {
                            physical_address: physical_address,
                            virtual_address: entry.virtual_address,
                            virtual_size: entry.virtual_size,
                        });
                    }
                }
                Rc::new(UnsafeCell::new(mem))
            },
            files: if flags & CLONE_FILES == CLONE_FILES {
                parent.files.clone()
            }else {
                let mut files: Vec<ContextFile> = Vec::new();
                for file in (*parent.files.get()).iter() {
                    if let Some(resource) = file.resource.dup() {
                        files.push(ContextFile {
                            fd: file.fd,
                            resource: resource
                        });
                    }
                }
                Rc::new(UnsafeCell::new(files))
            },
        };

        let contexts = &mut *contexts_ptr;
        contexts.push(context);
    }

    scheduler::end_no_ints(reenable);
}

//Must have absolutely no pushes or pops
#[cfg(target_arch = "x86")]
pub unsafe extern "cdecl" fn context_userspace(ip: usize, cs: usize, flags: usize, sp: usize, ss: usize) {
    asm!("mov eax, [esp + 16]
    mov ds, eax
    mov es, eax
    mov fs, eax
    mov gs, eax
    iretd" : : : "memory" : "intel", "volatile");
}

//Must have absolutely no pushes or pops
#[cfg(target_arch = "x86_64")]
pub unsafe extern "cdecl" fn context_userspace(ip: usize, cs: usize, flags: usize, sp: usize, ss: usize) {
    asm!("mov rax, [esp + 32]
    mov ds, rax
    mov es, rax
    mov fs, rax
    mov gs, rax
    iretq" : : : "memory" : "intel", "volatile");
}

/// Reads a Boxed function and executes it
///
/// Unsafe due to raw memory handling and FnBox
pub unsafe extern "cdecl" fn context_box(box_fn_ptr: usize) {
    let box_fn = ptr::read(box_fn_ptr as *mut Box<FnBox()>);
    memory::unalloc(box_fn_ptr);
    box_fn();
}

pub struct ContextMemory {
    pub physical_address: usize,
    pub virtual_address: usize,
    pub virtual_size: usize,
}

impl ContextMemory {
    pub unsafe fn map(&mut self) {
        for i in 0..(self.virtual_size + 4095) / 4096 {
            Page::new(self.virtual_address + i * 4096).map(self.physical_address + i * 4096);
        }
    }
    pub unsafe fn unmap(&mut self) {
        for i in 0..(self.virtual_size + 4095) / 4096 {
            Page::new(self.virtual_address + i * 4096).map_identity();
        }
    }
}

impl Drop for ContextMemory {
    fn drop(&mut self) {
        unsafe { memory::unalloc(self.physical_address) };
    }
}

pub struct ContextFile {
    pub fd: usize,
    pub resource: Box<Resource>,
}

pub struct Context {
    /* These members are used for control purposes by the scheduler { */
        /// Indicates that the context was interrupted, used for prioritizing active contexts
        pub interrupted: bool,
        /// Indicates that the context exited
        pub exited: bool,
    /* } */

    /* These members control the stack and registers and are unique to each context { */
        /// The kernel stack
        pub kernel_stack: usize,
        /// The current kernel stack pointer
        pub sp: usize,
        /// The current kernel flags
        pub flags: usize,
        /// The location used to save and load SSE and FPU registers
        pub fx: usize,
        /// The context stack
        pub stack: Option<ContextMemory>,
        /// Indicates that registers can be loaded (they must be saved first)
        pub loadable: bool,
    /* } */

    /* These members are cloned for threads, copied or created for processes { */
        /// Program arguments, cloned for threads, copied or created for processes. It is usually read-only, but is modified by execute
        pub args: Rc<UnsafeCell<Vec<String>>>,
        /// Program working directory, cloned for threads, copied or created for processes. Modified by chdir
        pub cwd: Rc<UnsafeCell<String>>,
        /// Program memory, cloned for threads, copied or created for processes. Modified by memory allocation
        pub memory: Rc<UnsafeCell<Vec<ContextMemory>>>,
        /// Program files, cloned for threads, copied or created for processes. Modified by file operations
        pub files: Rc<UnsafeCell<Vec<ContextFile>>>,
    /* } */
}

impl Context {
    pub unsafe fn root() -> Box<Self> {
       box Context {
           interrupted: false,
           exited: false,

           kernel_stack: 0,
           sp: 0,
           flags: 0,
           fx: memory::alloc(512),
           stack: None,
           loadable: false,

           args: Rc::new(UnsafeCell::new(Vec::new())),
           cwd: Rc::new(UnsafeCell::new(String::new())),
           memory: Rc::new(UnsafeCell::new(Vec::new())),
           files: Rc::new(UnsafeCell::new(Vec::new())),
       }
    }

    pub unsafe fn new(call: usize, args: &Vec<usize>, userspace: bool) -> Box<Self> {
        let kernel_stack = memory::alloc(CONTEXT_STACK_SIZE + 512);

        let mut ret = box Context {
            interrupted: false,
            exited: false,

            kernel_stack: kernel_stack,
            sp: kernel_stack + CONTEXT_STACK_SIZE - 128,
            flags: 0,
            fx: kernel_stack + CONTEXT_STACK_SIZE,
            stack: Some(ContextMemory {
                physical_address: memory::alloc(CONTEXT_STACK_SIZE),
                virtual_address: CONTEXT_STACK_ADDR,
                virtual_size: CONTEXT_STACK_SIZE
            }),
            loadable: false,

            args: Rc::new(UnsafeCell::new(Vec::new())),
            cwd: Rc::new(UnsafeCell::new(String::new())),
            memory: Rc::new(UnsafeCell::new(Vec::new())),
            files: Rc::new(UnsafeCell::new(Vec::new())),
        };

        for arg in args.iter() {
            ret.push(*arg);
        }

        if userspace {
            ret.push(0x20 | 3);
            ret.push(CONTEXT_STACK_ADDR + CONTEXT_STACK_SIZE - 128);
            ret.push(1 << 9);
            ret.push(0x18 | 3);
            ret.push(call);
            ret.push(context_userspace as usize);
        } else {
            ret.push(call);
        }

        ret
    }

    pub fn spawn(box_fn: Box<FnBox()>) {
       unsafe {
           let box_fn_ptr: *mut Box<FnBox()> = memory::alloc_type();
           ptr::write(box_fn_ptr, box_fn);

           let mut context_box_args: Vec<usize> = Vec::new();
           context_box_args.push(box_fn_ptr as usize);
           context_box_args.push(context_exit as usize);

           let reenable = scheduler::start_no_ints();
           if contexts_ptr as usize > 0 {
               (*contexts_ptr).push(Context::new(context_box as usize, &context_box_args, false));
           }
           scheduler::end_no_ints(reenable);
       }
   }

    pub unsafe fn current_i() -> usize {
        return context_i;
    }

    pub unsafe fn current<'a>() -> Option<&'a Box<Context>> {
        if context_enabled {
            let contexts = &mut *contexts_ptr;
            contexts.get(context_i)
        }else{
            None
        }
    }

    pub unsafe fn current_mut<'a>() -> Option<&'a mut Box<Context>> {
        if context_enabled {
            let contexts = &mut *contexts_ptr;
            contexts.get_mut(context_i)
        }else{
            None
        }
    }

    pub unsafe fn canonicalize(&self, path: &str) -> String {
        if path.find(':').is_none() {
            let cwd = &*self.cwd.get();
            if path == "../" {
                cwd.get_slice(None, Some(cwd.get_slice(None, Some(cwd.len() - 1)).rfind('/').map_or(cwd.len(), |i| i + 1))).to_string()
            } else if path == "./" {
                cwd.to_string()
            } else if path.starts_with('/') {
                cwd.get_slice(None, Some(cwd.find(':').map_or(1, |i| i + 1))).to_string() + &path
            } else {
                cwd.to_string() + &path
            }
        }else{
            path.to_string()
        }
    }

    pub unsafe fn get_file<'a>(&self, fd: usize) -> Option<&'a Box<Resource>> {
        for file in (*self.files.get()).iter() {
            if file.fd == fd {
                return Some(& file.resource);
            }
        }

        None
    }

    pub unsafe fn get_file_mut<'a>(&mut self, fd: usize) -> Option<&'a mut Box<Resource>> {
        for file in (*self.files.get()).iter_mut() {
            if file.fd == fd {
                return Some(&mut file.resource);
            }
        }

        None
    }

    pub unsafe fn next_fd(&self) -> usize {
        let mut next_fd = 0;

        let mut collision = true;
        while collision {
            collision = false;
            for file in (*self.files.get()).iter() {
                if next_fd == file.fd {
                    next_fd = file.fd + 1;
                    collision = true;
                    break;
                }
            }
        }

        return next_fd;
    }

    pub unsafe fn push(&mut self, data: usize) {
        self.sp -= mem::size_of::<usize>();
        ptr::write(self.sp as *mut usize, data);
    }

    pub unsafe fn map(&mut self) {
        if let Some(ref mut stack) = self.stack {
            stack.map();
        }
        for entry in (*self.memory.get()).iter_mut() {
            entry.map();
        }
    }

    pub unsafe fn unmap(&mut self) {
        for entry in (*self.memory.get()).iter_mut() {
            entry.unmap();
        }
        if let Some(ref mut stack) = self.stack {
            stack.unmap();
        }
    }

    //This function must not push or pop
    #[cold]
    #[inline(never)]
    pub unsafe fn save(&mut self) {
        asm!("pushfd
            pop $0"
            : "=r"(self.flags)
            :
            : "memory"
            : "intel", "volatile");

        asm!(""
            : "={esp}"(self.sp)
            :
            : "memory"
            : "intel", "volatile");

        asm!("fxsave [$0]"
            :
            : "r"(self.fx)
            : "memory"
            : "intel", "volatile");

        self.loadable = true;
    }

    //This function must not push or pop
    #[cold]
    #[inline(never)]
    pub unsafe fn restore(&mut self) {
        if self.loadable {
            asm!("fxrstor [$0]"
                :
                : "r"(self.fx)
                : "memory"
                : "intel", "volatile");
        }

        asm!(""
            :
            : "{esp}"(self.sp)
            : "memory"
            : "intel", "volatile");


        asm!("push $0
            popfd"
            :
            : "r"(self.flags)
            : "memory"
            : "intel", "volatile");
    }
}

impl Drop for Context {
    fn drop(&mut self){
        if self.kernel_stack > 0 {
            unsafe { memory::unalloc(self.kernel_stack) };
        }
    }
}
