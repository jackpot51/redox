use alloc::arc::{Arc, Weak};
use alloc::boxed::Box;

use collections::{BTreeMap, String};
use collections::string::ToString;

use core::cell::Cell;
use core::mem::size_of;

use scheduler::context::context_switch;

use schemes::{Result, Resource, ResourceSeek, KScheme, Url};

use sync::Intex;

use system::error::{Error, EBADF, EFAULT, EINVAL, ENOENT, ESPIPE, ESRCH};
use system::scheme::Packet;
use system::syscall::{SYS_CLOSE, SYS_FSYNC, SYS_FTRUNCATE,
                    SYS_LSEEK, SEEK_SET, SEEK_CUR, SEEK_END,
                    SYS_OPEN, SYS_READ, SYS_WRITE, SYS_UNLINK};

struct SchemeInner {
    next_id: Cell<usize>,
    todo: Intex<BTreeMap<usize, (usize, usize, usize, usize)>>,
    done: Intex<BTreeMap<usize, (usize, usize, usize, usize)>>,
}

impl SchemeInner {
    fn new() -> SchemeInner {
        SchemeInner {
            next_id: Cell::new(1),
            todo: Intex::new(BTreeMap::new()),
            done: Intex::new(BTreeMap::new()),
        }
    }

    fn call(inner: &Weak<SchemeInner>, a: usize, b: usize, c: usize, d: usize) -> Result<usize> {
        let id;
        if let Some(scheme) = inner.upgrade() {
            id = scheme.next_id.get();

            //TODO: What should be done about collisions in self.todo or self.done?
            let mut next_id = id + 1;
            if next_id <= 0 {
                next_id = 1;
            }
            scheme.next_id.set(next_id);

            scheme.todo.lock().insert(id, (a, b, c, d));
        } else {
            return Err(Error::new(EBADF));
        }

        loop {
            if let Some(scheme) = inner.upgrade() {
                if let Some(regs) = scheme.done.lock().remove(&id) {
                    return Error::demux(regs.0);
                }
            } else {
                return Err(Error::new(EBADF));
            }

            unsafe { context_switch(false) } ;
        }
    }
}

pub struct SchemeResource {
    inner: Weak<SchemeInner>,
    file_id: usize,
}

impl SchemeResource {
    fn call(&self, a: usize, b: usize, c: usize, d: usize) -> Result<usize> {
        SchemeInner::call(&self.inner, a, b, c, d)
    }
}

impl Resource for SchemeResource {
    /// Duplicate the resource
    fn dup(&self) -> Result<Box<Resource>> {
        Err(Error::new(EBADF))
    }

    /// Return the url of this resource
    fn url(&self) -> Url {
        Url::new()
    }

    /// Read data to buffer
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let contexts = ::env().contexts.lock();
        if let Some(current) = contexts.current() {
            if let Some(translated) = unsafe { current.translate(buf.as_mut_ptr() as usize) } {
                self.call(SYS_READ, self.file_id, translated, buf.len())
            } else {
                Err(Error::new(EFAULT))
            }
        } else {
            Err(Error::new(ESRCH))
        }
    }

    /// Write to resource
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let contexts = ::env().contexts.lock();
        if let Some(current) = contexts.current() {
            if let Some(translated) = unsafe { current.translate(buf.as_ptr() as usize) } {
                self.call(SYS_WRITE, self.file_id, translated, buf.len())
            } else {
                Err(Error::new(EFAULT))
            }
        } else {
            Err(Error::new(ESRCH))
        }
    }

    /// Seek
    fn seek(&mut self, pos: ResourceSeek) -> Result<usize> {
        let (whence, offset) = match pos {
            ResourceSeek::Start(offset) => (SEEK_SET, offset as usize),
            ResourceSeek::Current(offset) => (SEEK_CUR, offset as usize),
            ResourceSeek::End(offset) => (SEEK_END, offset as usize)
        };

        self.call(SYS_LSEEK, self.file_id, offset, whence)
    }

    /// Sync the resource
    fn sync(&mut self) -> Result<()> {
        self.call(SYS_FSYNC, self.file_id, 0, 0).and(Ok(()))
    }

    fn truncate(&mut self, len: usize) -> Result<()> {
        self.call(SYS_FTRUNCATE, self.file_id, len, 0).and(Ok(()))
    }
}

impl Drop for SchemeResource {
    fn drop(&mut self) {
        let _ = self.call(SYS_CLOSE, self.file_id, 0, 0);
    }
}

pub struct SchemeServerResource {
    path: String,
    inner: Arc<SchemeInner>,
}

impl Resource for SchemeServerResource {
    /// Duplicate the resource
    fn dup(&self) -> Result<Box<Resource>> {
        Ok(box SchemeServerResource {
            path: self.path.clone(),
            inner: self.inner.clone()
        })
    }

    /// Return the url of this resource
    fn url(&self) -> Url {
        Url::from_str(&self.path)
    }

    // TODO: Make use of Write and Read trait
    /// Read data to buffer
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if buf.len() == size_of::<Packet>() {
            let packet_ptr: *mut Packet = buf.as_mut_ptr() as *mut Packet;
            let packet = unsafe { &mut *packet_ptr };
            loop {
                let mut todo = self.inner.todo.lock();

                packet.id = if let Some(id) = todo.keys().next() {
                    *id
                } else {
                    0
                };

                if packet.id > 0 {
                    if let Some(regs) = todo.remove(&packet.id) {
                        packet.a = regs.0;
                        packet.b = regs.1;
                        packet.c = regs.2;
                        packet.d = regs.3;
                        return Ok(size_of::<Packet>())
                    }
                }

                unsafe { context_switch(false) };
            }
        } else {
            return Err(Error::new(EINVAL))
        }
    }

    /// Write to resource
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if buf.len() == size_of::<Packet>() {
            let packet_ptr: *const Packet = buf.as_ptr() as *const Packet;
            let packet = unsafe { & *packet_ptr };
            self.inner.done.lock().insert(packet.id, (packet.a, packet.b, packet.c, packet.d));
            return Ok(size_of::<Packet>())
        } else {
            return Err(Error::new(EINVAL))
        }
    }

    /// Seek
    fn seek(&mut self, pos: ResourceSeek) -> Result<usize> {
        Err(Error::new(ESPIPE))
    }

    /// Sync the resource
    fn sync(&mut self) -> Result<()> {
        Err(Error::new(EINVAL))
    }

    fn truncate(&mut self, len: usize) -> Result<()> {
        Err(Error::new(EINVAL))
    }
}

/// Scheme has to be wrapped
pub struct Scheme {
    name: String,
    inner: Weak<SchemeInner>
}

impl Scheme {
    pub fn new(name: String) -> (Box<Scheme>, Box<Resource>) {
        let server = box SchemeServerResource {
            path: ":".to_string() + &name,
            inner: Arc::new(SchemeInner::new())
        };
        let scheme = box Scheme {
            name: name,
            inner: Arc::downgrade(&server.inner)
        };
        (scheme, server)
    }

    fn call(&self, a: usize, b: usize, c: usize, d: usize) -> Result<usize> {
        SchemeInner::call(&self.inner, a, b, c, d)
    }
}

impl KScheme for Scheme {
    fn on_irq(&mut self, irq: u8) {

    }

    fn on_poll(&mut self) {

    }

    fn scheme(&self) -> &str {
        &self.name
    }

    fn open(&mut self, url: &Url, flags: usize) -> Result<Box<Resource>> {
        let c_str = url.string.clone() + "\0";
        match self.call(SYS_OPEN, c_str.as_ptr() as usize, flags, 0) {
            Ok(file_id) => Ok(box SchemeResource {
                inner: self.inner.clone(),
                file_id: file_id,
            }),
            Err(err) => Err(err)
        }
    }

    fn unlink(&mut self, url: &Url) -> Result<()> {
        let c_str = url.string.clone() + "\0";
        self.call(SYS_UNLINK, c_str.as_ptr() as usize, 0, 0).and(Ok(()))
    }
}
