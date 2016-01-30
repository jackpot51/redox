use alloc::boxed::Box;

use collections::string::{String, ToString};
use collections::vec::Vec;

use common::get_slice::GetSlice;
use common::memory::Memory;

use core::{cmp, ptr, slice};

use disk::Disk;

pub use self::header::Header;
pub use self::node::{Node, NodeData};

pub mod header;
pub mod node;

/// A file system
pub struct FileSystem {
    pub disk: Box<Disk>,
    pub header: Header,
    pub nodes: Vec<Node>,
}

impl FileSystem {
    /// Create a file system from a disk
    pub fn from_disk(mut disk: Box<Disk>) -> Option<Self> {
        if let Some(data) = Memory::<u8>::new(512) {
            let mut buffer = unsafe { slice::from_raw_parts_mut(data.ptr, 512) };
            if let Err(_) = disk.read(1, &mut buffer) {
                return None;
            }

            let header = unsafe { ptr::read(data.ptr as *const Header) };
            if header.valid() {
                debugln!("{}: Redox Filesystem", disk.name());

                let mut nodes = Vec::new();
                for extent in &header.extents {
                    if extent.block > 0 && extent.length > 0 {
                        let current_sectors = (extent.length as usize + 511) / 512;
                        let max_size = current_sectors * 512;

                        let size = cmp::min(extent.length as usize, max_size);

                        if let Some(data) = Memory::<u8>::new(max_size) {
                            let mut buffer = unsafe {
                                slice::from_raw_parts_mut(data.ptr, max_size)
                            };
                            let _ = disk.read(extent.block, &mut buffer);

                            for i in 0..size / 512 {
                                nodes.push(Node::new(extent.block + i as u64, unsafe {
                                    &*(data.ptr.offset(i as isize * 512) as *const NodeData)
                                }));
                            }
                        }
                    }
                }

                return Some(FileSystem {
                    disk: disk,
                    header: header,
                    nodes: nodes,
                });
            } else {
                debugln!("{}: Unknown Filesystem", disk.name());
            }
        }

        None
    }

    /// Get node with a given filename
    pub fn node(&self, filename: &str) -> Option<Node> {
        for node in self.nodes.iter() {
            if node.name == filename {
                return Some(node.clone());
            }
        }

        None
    }

    /// List nodes in a given directory
    pub fn list(&self, directory: &str) -> Vec<String> {
        let mut ret = Vec::new();

        for node in self.nodes.iter() {
            if node.name.starts_with(directory) {
                ret.push(node.name.get_slice(directory.len()..).to_string());
            }
        }

        ret
    }
}
