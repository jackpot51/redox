use alloc::arc::{Arc, Weak};
use collections::{BTreeMap, VecDeque};
use core::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use spin::{Mutex, Once, RwLock, RwLockReadGuard, RwLockWriteGuard};
use scheme::{AtomicSchemeId, ATOMIC_SCHEMEID_INIT};

use sync::WaitCondition;
use syscall::error::{Error, Result, EBADF, EPIPE};
use syscall::flag::O_NONBLOCK;
use syscall::scheme::Scheme;

/// Pipes list
pub static PIPE_SCHEME_ID: AtomicSchemeId = ATOMIC_SCHEMEID_INIT;
static PIPE_NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;
static PIPES: Once<RwLock<(BTreeMap<usize, PipeRead>, BTreeMap<usize, PipeWrite>)>> = Once::new();

/// Initialize pipes, called if needed
fn init_pipes() -> RwLock<(BTreeMap<usize, PipeRead>, BTreeMap<usize, PipeWrite>)> {
    RwLock::new((BTreeMap::new(), BTreeMap::new()))
}

/// Get the global pipes list, const
fn pipes() -> RwLockReadGuard<'static, (BTreeMap<usize, PipeRead>, BTreeMap<usize, PipeWrite>)> {
    PIPES.call_once(init_pipes).read()
}

/// Get the global schemes list, mutable
fn pipes_mut() -> RwLockWriteGuard<'static, (BTreeMap<usize, PipeRead>, BTreeMap<usize, PipeWrite>)> {
    PIPES.call_once(init_pipes).write()
}

pub fn pipe(flags: usize) -> (usize, usize) {
    let mut pipes = pipes_mut();
    let read_id = PIPE_NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let write_id = PIPE_NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let read = PipeRead::new(flags);
    let write = PipeWrite::new(&read);
    pipes.0.insert(read_id, read);
    pipes.1.insert(write_id, write);
    (read_id, write_id)
}

pub struct PipeScheme;

impl Scheme for PipeScheme {
    fn dup(&self, id: usize, _buf: &[u8]) -> Result<usize> {
        let mut pipes = pipes_mut();

        let read_option = pipes.0.get(&id).map(|pipe| pipe.clone());
        if let Some(pipe) = read_option {
            let pipe_id = PIPE_NEXT_ID.fetch_add(1, Ordering::SeqCst);
            pipes.0.insert(pipe_id, pipe);
            return Ok(pipe_id);
        }

        let write_option = pipes.1.get(&id).map(|pipe| pipe.clone());
        if let Some(pipe) = write_option {
            let pipe_id = PIPE_NEXT_ID.fetch_add(1, Ordering::SeqCst);
            pipes.1.insert(pipe_id, pipe);
            return Ok(pipe_id);
        }

        Err(Error::new(EBADF))
    }

    fn read(&self, id: usize, buf: &mut [u8]) -> Result<usize> {
        let pipe_option = {
            let pipes = pipes();
            pipes.0.get(&id).map(|pipe| pipe.clone())
        };

        if let Some(pipe) = pipe_option {
            pipe.read(buf)
        } else {
            Err(Error::new(EBADF))
        }
    }

    fn write(&self, id: usize, buf: &[u8]) -> Result<usize> {
        let pipe_option = {
            let pipes = pipes();
            pipes.1.get(&id).map(|pipe| pipe.clone())
        };

        if let Some(pipe) = pipe_option {
            pipe.write(buf)
        } else {
            Err(Error::new(EBADF))
        }
    }

    fn fsync(&self, _id: usize) -> Result<usize> {
        Ok(0)
    }

    fn close(&self, id: usize) -> Result<usize> {
        let mut pipes = pipes_mut();

        drop(pipes.0.remove(&id));
        drop(pipes.1.remove(&id));

        Ok(0)
    }
}

/// Read side of a pipe
#[derive(Clone)]
pub struct PipeRead {
    flags: usize,
    condition: Arc<WaitCondition>,
    vec: Arc<Mutex<VecDeque<u8>>>
}

impl PipeRead {
    pub fn new(flags: usize) -> Self {
        PipeRead {
            flags: flags,
            condition: Arc::new(WaitCondition::new()),
            vec: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        loop {
            {
                let mut vec = self.vec.lock();

                let mut i = 0;
                while i < buf.len() {
                    if let Some(b) = vec.pop_front() {
                        buf[i] = b;
                        i += 1;
                    } else {
                        break;
                    }
                }

                if i > 0 {
                    return Ok(i);
                }
            }

            if self.flags & O_NONBLOCK == O_NONBLOCK || Arc::weak_count(&self.vec) == 0 {
                return Ok(0);
            } else {
                self.condition.wait();
            }
        }
    }
}

/// Read side of a pipe
#[derive(Clone)]
pub struct PipeWrite {
    condition: Arc<WaitCondition>,
    vec: Weak<Mutex<VecDeque<u8>>>
}

impl PipeWrite {
    pub fn new(read: &PipeRead) -> Self {
        PipeWrite {
            condition: read.condition.clone(),
            vec: Arc::downgrade(&read.vec),
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        if let Some(vec_lock) = self.vec.upgrade() {
            let mut vec = vec_lock.lock();

            for &b in buf.iter() {
                vec.push_back(b);
            }

            self.condition.notify();

            Ok(buf.len())
        } else {
            Err(Error::new(EPIPE))
        }
    }
}

impl Drop for PipeWrite {
    fn drop(&mut self) {
        self.condition.notify();
    }
}
