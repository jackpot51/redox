use common::get_slice::GetSlice;

use alloc::boxed::Box;

use collections::slice;
use collections::string::{String, ToString};
use collections::vec::Vec;

use core::cmp;

use disk::Disk;
use disk::ide::Extent;

use fs::redoxfs::{FileSystem, Node, NodeData};

use common::debug;
use common::memory::Memory;

use schemes::{Result, KScheme, Resource, ResourceSeek, Url, VecResource};

use sync::Intex;

use syscall::{Error, O_CREAT, ENOENT, EIO};

/// A file resource
pub struct FileResource {
    pub scheme: *mut FileScheme,
    pub node: Node,
    pub vec: Vec<u8>,
    pub seek: usize,
    pub dirty: bool,
}

impl Resource for FileResource {
    fn dup(&self) -> Result<Box<Resource>> {
        Ok(box FileResource {
            scheme: self.scheme,
            node: self.node.clone(),
            vec: self.vec.clone(),
            seek: self.seek,
            dirty: self.dirty,
        })
    }

    fn url(&self) -> Url {
        Url::from_string("file:/".to_string() + &self.node.name)
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut i = 0;
        while i < buf.len() && self.seek < self.vec.len() {
            match self.vec.get(self.seek) {
                Some(b) => buf[i] = *b,
                None => (),
            }
            self.seek += 1;
            i += 1;
        }
        Ok(i)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut i = 0;
        while i < buf.len() && self.seek < self.vec.len() {
            self.vec[self.seek] = buf[i];
            self.seek += 1;
            i += 1;
        }
        while i < buf.len() {
            self.vec.push(buf[i]);
            self.seek += 1;
            i += 1;
        }
        if i > 0 {
            self.dirty = true;
        }
        Ok(i)
    }

    fn seek(&mut self, pos: ResourceSeek) -> Result<usize> {
        match pos {
            ResourceSeek::Start(offset) => self.seek = offset,
            ResourceSeek::Current(offset) =>
                self.seek = cmp::max(0, self.seek as isize + offset) as usize,
            ResourceSeek::End(offset) =>
                self.seek = cmp::max(0, self.vec.len() as isize + offset) as usize,
        }
        while self.vec.len() < self.seek {
            self.vec.push(0);
        }
        Ok(self.seek)
    }

    // TODO: Check to make sure proper amount of bytes written. See Disk::write
    // TODO: Allow reallocation
    fn sync(&mut self) -> Result<()> {
        if self.dirty {
            let mut node_dirty = false;
            let mut pos = 0;
            let mut remaining = self.vec.len() as isize;
            for ref mut extent in &mut self.node.extents {
                if remaining > 0 && extent.empty() {
                    debug::d("Reallocate file, extra: ");
                    debug::ds(remaining);
                    debug::dl();

                    unsafe {
                        let _intex = Intex::static_lock();

                        let sectors = ((remaining + 511) / 512) as u64;
                        if (*self.scheme).fs.header.free_space.length >= sectors * 512 {
                            extent.block = (*self.scheme).fs.header.free_space.block;
                            extent.length = remaining as u64;
                            (*self.scheme).fs.header.free_space.block = (*self.scheme)
                                                                            .fs
                                                                            .header
                                                                            .free_space
                                                                            .block +
                                                                        sectors;
                            (*self.scheme).fs.header.free_space.length = (*self.scheme)
                                                                             .fs
                                                                             .header
                                                                             .free_space
                                                                             .length -
                                                                         sectors * 512;

                            node_dirty = true;
                        }
                    }
                }

                // Make sure it is a valid extent
                if !extent.empty() {
                    let current_sectors = (extent.length as usize + 511) / 512;
                    let max_size = current_sectors * 512;

                    let size = cmp::min(remaining as usize, max_size);

                    if size as u64 != extent.length {
                        extent.length = size as u64;
                        node_dirty = true;
                    }

                    while self.vec.len() < pos + max_size {
                        self.vec.push(0);
                    }

                    unsafe {
                        let _ = (*self.scheme).fs.disk.write(extent.block, &self.vec[pos .. pos + max_size]);
                    }

                    self.vec.truncate(pos + size);

                    pos += size;
                    remaining -= size as isize;
                }
            }

            if node_dirty {
                debug::d("Node dirty, rewrite\n");

                if self.node.block > 0 {
                    unsafe {
                        if let Some(mut node_data) = Memory::<NodeData>::new(1) {
                            node_data.write(0, self.node.data());

                            let mut buffer = slice::from_raw_parts(node_data.address() as *mut u8, 512);
                            let _ = (*self.scheme).fs.disk.write(self.node.block, &mut buffer);

                            debug::d("Renode\n");

                            {
                                let _intex = Intex::static_lock();

                                for mut node in (*self.scheme).fs.nodes.iter_mut() {
                                    if node.block == self.node.block {
                                        *node = self.node.clone();
                                    }
                                }
                            }
                        }
                    }
                } else {
                    debug::d("Need to place Node block\n");
                }
            }

            self.dirty = false;

            if remaining > 0 {
                debug::d("Need to defragment file, extra: ");
                debug::ds(remaining);
                debug::dl();
                return Err(Error::new(EIO));
            }
        }
        Ok(())
    }

    fn truncate(&mut self, len: usize) -> Result<()> {
        while len > self.vec.len() {
            self.vec.push(0);
        }
        self.vec.truncate(len);
        self.seek = cmp::min(self.seek, self.vec.len());
        self.dirty = true;
        Ok(())
    }
}

impl Drop for FileResource {
    fn drop(&mut self) {
        let _ = self.sync();
    }
}

/// A file scheme (pci + fs)
pub struct FileScheme {
    fs: FileSystem,
}

impl FileScheme {
    /// Create a new file scheme from an array of Disks
    pub fn new(mut disks: Vec<Box<Disk>>) -> Option<Box<Self>> {
        while ! disks.is_empty() {
            if let Some(fs) = FileSystem::from_disk(disks.remove(0)) {
                return Some(box FileScheme { fs: fs });
            }
        }

        None
    }
}

impl KScheme for FileScheme {
    fn on_irq(&mut self, _irq: u8) {
        /*if irq == self.fs.disk.irq {
            self.on_poll();
        }*/
    }

    fn on_poll(&mut self) {
        //self.fs.disk.on_poll();
    }

    fn scheme(&self) -> &str {
        "file"
    }

    fn open(&mut self, url: &Url, flags: usize) -> Result<Box<Resource>> {
        let mut path = url.reference();
        while path.starts_with('/') {
            path = &path[1..];
        }
        if path.is_empty() || path.ends_with('/') {
            let mut list = String::new();
            let mut dirs: Vec<String> = Vec::new();

            for file in self.fs.list(path).iter() {
                let mut line = String::new();
                match file.find('/') {
                    Some(index) => {
                        let dirname = file.get_slice(..index + 1).to_string();
                        let mut found = false;
                        for dir in dirs.iter() {
                            if dirname == *dir {
                                found = true;
                                break;
                            }
                        }
                        if found {
                            line.clear();
                        } else {
                            line = dirname.clone();
                            dirs.push(dirname);
                        }
                    }
                    None => line = file.clone(),
                }
                if !line.is_empty() {
                    if !list.is_empty() {
                        list = list + "\n" + &line;
                    } else {
                        list = line;
                    }
                }
            }

            if list.len() > 0 {
                Ok(box VecResource::new(url.clone(), list.into_bytes()))
            } else {
                Err(Error::new(ENOENT))
            }
        } else {
            match self.fs.node(path) {
                Some(node) => {
                    let mut vec: Vec<u8> = Vec::new();
                    for extent in &node.extents {
                        if extent.block > 0 && extent.length > 0 {
                            let current_sectors = (extent.length as usize + 511) / 512;
                            let max_size = current_sectors * 512;

                            let size = cmp::min(extent.length as usize, max_size);

                            let pos = vec.len();

                            while vec.len() < pos + max_size {
                                vec.push(0);
                            }

                            let _ = self.fs.disk.read(extent.block, &mut vec[pos..pos + max_size]);

                            vec.truncate(pos + size);
                        }
                    }

                    Ok(box FileResource {
                        scheme: self,
                        node: node,
                        vec: vec,
                        seek: 0,
                        dirty: false,
                    })
                }
                None => {
                    if flags & O_CREAT == O_CREAT {
                        // TODO: Create file
                        let mut node = Node {
                            block: 0,
                            name: path.to_string(),
                            extents: [Extent {
                                block: 0,
                                length: 0,
                            }; 16],
                        };

                        if self.fs.header.free_space.length >= 512 {
                            node.block = self.fs.header.free_space.block;
                            self.fs.header.free_space.block = self.fs.header.free_space.block + 1;
                            self.fs.header.free_space.length = self.fs.header.free_space.length -
                                                               512;
                        }

                        self.fs.nodes.push(node.clone());

                        Ok(box FileResource {
                            scheme: self,
                            node: node,
                            vec: Vec::new(),
                            seek: 0,
                            dirty: false,
                        })
                    } else {
                        Err(Error::new(ENOENT))
                    }
                }
            }
        }
    }

    fn unlink(&mut self, url: &Url) -> Result<()> {
        let mut ret = Err(Error::new(ENOENT));

        let mut path = url.reference();
        while path.starts_with('/') {
            path = &path[1..];
        }

        let mut i = 0;
        while i < self.fs.nodes.len() {
            let mut remove = false;

            if let Some(node) = self.fs.nodes.get(i) {
                remove = node.name == path;
            }

            if remove {
                self.fs.nodes.remove(i);
                ret = Ok(());
            } else {
                i += 1;
            }
        }

        ret
    }
}
