//To use this, please install zfs-fuse
use redox::*;

use self::arcache::ArCache;
use self::dnode::{DNodePhys, ObjectSetPhys, ObjectType};
use self::block_ptr::BlockPtr;
use self::dsl_dataset::DslDatasetPhys;
use self::dsl_dir::DslDirPhys;
use self::from_bytes::FromBytes;
use self::nvpair::NvValue;
use self::space_map::SpaceMapPhys;
use self::uberblock::Uberblock;
use self::vdev::VdevLabel;

pub mod arcache;
pub mod avl;
pub mod block_ptr;
pub mod dnode;
pub mod dsl_dataset;
pub mod dsl_dir;
pub mod dvaddr;
pub mod from_bytes;
pub mod lzjb;
pub mod nvpair;
pub mod nvstream;
pub mod space_map;
pub mod uberblock;
pub mod vdev;
pub mod xdr;
pub mod zap;
pub mod zil_header;
pub mod zio;

pub struct ZfsReader {
    pub zio: zio::Reader,
    pub arc: ArCache,
}

impl ZfsReader {
    pub fn read_block(&mut self, block_ptr: &BlockPtr) -> Result<Vec<u8>, String> {
        let data = self.arc.read(&mut self.zio, &block_ptr.dvas[0]);
        match block_ptr.compression() {
            2 => {
                // compression off
                data
            },
            1 | 3 => {
                // lzjb compression
                let mut decompressed = vec![0; (block_ptr.lsize()*512) as usize];
                lzjb::decompress(& match data {
                                     Ok(data) => data,
                                     Err(e) => return Err(e),
                                 },
                                 &mut decompressed);
                Ok(decompressed)
            },
            u => Err(format!("Error: Unknown compression type {}", u)),
        }
    }

    pub fn read_type<T: FromBytes>(&mut self, block_ptr: &BlockPtr) -> Result<T, String> {
        let data = self.read_block(block_ptr);
        data.and_then(|data| T::from_bytes(&data[..]))
    }

    pub fn read_type_array<T: FromBytes>(&mut self, block_ptr: &BlockPtr, offset: usize) -> Result<T, String> {
        let data = self.read_block(block_ptr);
        data.and_then(|data| T::from_bytes(&data[offset*mem::size_of::<T>()..]))
    }

    pub fn uber(&mut self, uberblocks: &[u8]) -> Result<Uberblock, String> {
        let mut newest_uberblock: Option<Uberblock> = None;
        for i in 0..128 {
            /*let ub_len = 2*512;
            let ub_start = i * ub_len;
            let ub_end = ub_start + ub_len;
            if let Ok(uberblock) = Uberblock::from_bytes(&uberblocks[ub_start..ub_end]) {*/
            if let Ok(uberblock) = Uberblock::from_bytes(&self.zio.read(256 + i * 2, 2)) {
                let newest =
                    match newest_uberblock {
                        Some(previous) => {
                            if uberblock.txg > previous.txg {
                                // Found a newer uberblock
                                true
                            } else {
                                false
                            }
                        }
                        // No uberblock yet, so first one we find is the newest
                        None => true,
                    };

                if newest {
                    newest_uberblock = Some(uberblock);
                }
            }
        }

        match newest_uberblock {
            Some(uberblock) => Ok(uberblock),
            None => Err("Failed to find valid uberblock".to_string()),
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum ZfsTraverse {
    ThisDir,
    Done,
}

pub struct Zfs {
    pub reader: ZfsReader,
    pub uberblock: Uberblock, // The active uberblock
    pub mos: ObjectSetPhys,
    fs_objset: ObjectSetPhys,
    master_node: DNodePhys,
    root: u64,
}

impl Zfs {
    pub fn new(disk: File) -> Result<Self, String> {
        let mut zfs_reader = ZfsReader { zio: zio::Reader { disk: disk }, arc: ArCache::new() };

        // Read vdev label
        let vdev_label = try!(VdevLabel::from_bytes(&zfs_reader.zio.read(0, 256 * 2)));
        //let mut xdr = xdr::MemOps::new(&mut vdev_label.nv_pairs);
        //let nv_list = try!(nvstream::decode_nv_list(&mut xdr).map_err(|e| format!("{:?}", e)));
        /*let vdev_tree =
            match nv_list.find("vdev_tree") {
                Some(vdev_tree) => {
                    vdev_tree
                },
                None => {
                    return Err("No vdev_tree in vdev label nvpairs".to_string());
                },
            };

        let vdev_tree =
            if let NvValue::NvList(ref vdev_tree) = *vdev_tree {
                vdev_tree
            } else {
                return Err("vdev_tree is not NvValue::NvList".to_string());
            };*/

        // Get the active uberblock
        let uberblock = try!(zfs_reader.uber(&vdev_label.uberblocks));

        //let mos_dva = uberblock.rootbp.dvas[0];
        let mos: ObjectSetPhys = try!(zfs_reader.read_type(&uberblock.rootbp));
        let mos_block_ptr1 = mos.meta_dnode.get_blockptr(0);

        // 2nd dnode in MOS points at the root dataset zap
        let dnode1: DNodePhys = try!(zfs_reader.read_type_array(&mos_block_ptr1, 1));

        let root_ds_dnode= dnode1.get_blockptr(0);
        let root_ds: zap::MZapWrapper = try!(zfs_reader.read_type(root_ds_dnode));

        let root_ds_dnode: DNodePhys =
            try!(zfs_reader.read_type_array(&mos_block_ptr1, root_ds.chunks[0].value as usize));

        let dsl_dir = try!(DslDirPhys::from_bytes(root_ds_dnode.get_bonus()));
        let head_ds_dnode: DNodePhys =
            try!(zfs_reader.read_type_array(&mos_block_ptr1, dsl_dir.head_dataset_obj as usize));

        let root_dataset = try!(DslDatasetPhys::from_bytes(head_ds_dnode.get_bonus()));

        let fs_objset: ObjectSetPhys = try!(zfs_reader.read_type(&root_dataset.bp));

        let mut indirect: BlockPtr = try!(zfs_reader.read_type_array(fs_objset.meta_dnode.get_blockptr(0), 0));
        while indirect.level() > 0 {
            indirect = try!(zfs_reader.read_type_array(&indirect, 0));
        }

        // Master node is always the second object in the object set
        let master_node: DNodePhys = try!(zfs_reader.read_type_array(&indirect, 1));
        let master_node_zap: zap::MZapWrapper = try!(zfs_reader.read_type(master_node.get_blockptr(0)));

        // Find the ROOT zap entry
        let mut root = None;
        for chunk in &master_node_zap.chunks {
            if chunk.name() == Some("ROOT") {
                root = Some(chunk.value);
                break;
            }
        }

        let root =
            match root {
                Some(root) => Ok(root),
                None => Err("Error: failed to get the ROOT".to_string()),
            };

        Ok(Zfs {
            reader: zfs_reader,
            uberblock: uberblock,
            mos: mos,
            fs_objset: fs_objset,
            master_node: master_node,
            root: try!(root),
        })
    }

    pub fn traverse<F, T>(&mut self, mut f: F) -> Option<T>
        where F: FnMut(&mut Self, &str, usize, &mut DNodePhys, &BlockPtr, &mut Option<T>) -> Option<ZfsTraverse>,
    {
        // Given the fs_objset and the object id of the root directory, we can traverse the
        // directory tree.
        // TODO: Cache object id of paths
        // TODO: Calculate path through objset blockptr tree to use
        let mut indirect: BlockPtr = self.reader.read_type_array(self.fs_objset.meta_dnode.get_blockptr(0), 0).unwrap();
        while indirect.level() > 0 {
            indirect = self.reader.read_type_array(&indirect, 0).unwrap();
        }
        // Set the cur_node to the root node, located at an L0 indirect block
        let root = self.root as usize;
        let mut cur_node: DNodePhys = self.reader.read_type_array(&indirect,
                                                                  self.root as usize).unwrap();
        let mut result = None;
        if f(self, "", root, &mut cur_node, &indirect, &mut result) == Some(ZfsTraverse::Done) {
            return result;
        }
        'traverse: loop {
            // Directory dnodes point at zap objects. File/directory names are mapped to their
            // fs_objset object ids.
            let dir_contents: zap::MZapWrapper = self.reader.read_type(cur_node.get_blockptr(0)).unwrap();
            let mut next_dir = None;
            for chunk in &dir_contents.chunks {
                match chunk.name() {
                    Some(chunk_name) => {
                        // Stop once we get to a null entry
                        if chunk_name.is_empty() {
                            break;
                        }

                        let traverse = f(self, chunk_name, chunk.value as usize,
                                         &mut cur_node, &indirect, &mut result);
                        if let Some(traverse) = traverse {
                            match traverse {
                                ZfsTraverse::ThisDir => {
                                    // Found the folder we were looking for
                                    next_dir = Some(chunk.value);
                                    break;
                                },
                                ZfsTraverse::Done => {
                                    break 'traverse;
                                },
                            }
                        }
                    },
                    None => {
                        // Invalid directory name
                        return None;
                    },
                }
            }
            if next_dir.is_none() {
                break;
            }
        }
        result
    }

    pub fn read_file(&mut self, path: &str) -> Option<Vec<u8>> {
        let path = path.trim_matches('/'); // Robust against different url styles
        let path_end_index = path.rfind('/').map(|i| i+1).unwrap_or(0);
        let path_end = &path[path_end_index..];
        let mut folder_iter = path.split('/');
        let mut folder = folder_iter.next();

        let file_contents =
            self.traverse(|zfs, name, node_id, node, indirect, result| {
                let mut this_dir = false;
                if let Some(folder) = folder {
                    if name == folder {
                        *node = zfs.reader.read_type_array(indirect,
                                                           node_id as usize).unwrap();
                        if name == path_end {
                            if node.object_type != ObjectType::PlainFileContents {
                                // Not a file
                                return Some(ZfsTraverse::Done);
                            }
                            // Found the file
                            let file_contents = zfs.reader.read_block(node.get_blockptr(0)).unwrap();
                            // TODO: Read file size from ZPL rather than look for terminating 0
                            let file_contents: Vec<u8> = file_contents.into_iter().take_while(|c| *c != 0).collect();
                            *result = Some(file_contents);
                            return Some(ZfsTraverse::Done);
                        }
                        this_dir = true;
                    }
                }
                if this_dir {
                    if node.object_type != ObjectType::DirectoryContents {
                        // Not a folder
                        return Some(ZfsTraverse::Done);
                    }
                    folder = folder_iter.next();
                    return Some(ZfsTraverse::ThisDir);
                }
                None
            });

        file_contents
    }

    pub fn ls(&mut self, path: &str) -> Option<Vec<String>> {
        let path = path.trim_matches('/'); // Robust against different url styles
        let path_end_index = path.rfind('/').map(|i| i+1).unwrap_or(0);
        let path_end = &path[path_end_index..];
        let mut folder_iter = path.split('/');
        let mut folder = folder_iter.next();

        let file_contents =
            self.traverse(|zfs, name, node_id, node, indirect, result| {
                let mut this_dir = false;
                if let Some(folder) = folder {
                    if name == folder {
                        if folder == path_end {
                            *node = zfs.reader.read_type_array(indirect,
                                                               node_id as usize).unwrap();
                            let dir_contents: zap::MZapWrapper = zfs.reader
                                                                    .read_type(node.get_blockptr(0))
                                                                    .unwrap();

                            let ls: Vec<String> =
                                dir_contents.chunks
                                            .iter()
                                            .map(|x| {
                                                if x.value & 0xF000000000000000 == 0x4000000000000000 {
                                                    x.name().unwrap().to_string() + "/"
                                                } else {
                                                    x.name().unwrap().to_string()
                                                }
                                            })
                                            .take_while(|x| !x.is_empty())
                                            .collect();
                            *result = Some(ls);
                            return Some(ZfsTraverse::Done);
                        }
                        this_dir = true;
                    }
                }
                if this_dir {
                    folder = folder_iter.next();
                    return Some(ZfsTraverse::ThisDir);
                }
                None
            });

        file_contents
    }
}

//TODO: Find a way to remove all the to_string's
pub fn main() {
    println!("Type open zfs.img to open the image file");

    let mut zfs_option: Option<Zfs> = None;

    while let Some(line) = readln!() {
        let mut args: Vec<String> = Vec::new();
        for arg in line.split(' ') {
            args.push(arg.to_string());
        }

        if let Some(command) = args.get(0) {
            println!("# {}", line);

            let mut close = false;
            match zfs_option {
                Some(ref mut zfs) => {
                    if command == "uber" {
                        let ref uberblock = zfs.uberblock;
                        //128 KB of ubers after 128 KB of other stuff
                        println!("Newest Uberblock {:X}", zfs.uberblock.magic);
                        println!("Version {}", uberblock.version);
                        println!("TXG {}", uberblock.txg);
                        println!("GUID {:X}", uberblock.guid_sum);
                        println!("Timestamp {}", uberblock.timestamp);
                        println!("ROOTBP[0] {:?}", uberblock.rootbp.dvas[0]);
                        println!("ROOTBP[1] {:?}", uberblock.rootbp.dvas[1]);
                        println!("ROOTBP[2] {:?}", uberblock.rootbp.dvas[2]);
                    } else if command == "vdev_label" {
                        match VdevLabel::from_bytes(&zfs.reader.zio.read(0, 256 * 2)) {
                            Ok(ref mut vdev_label) => {
                                let mut xdr = xdr::MemOps::new(&mut vdev_label.nv_pairs);
                                let nv_list = nvstream::decode_nv_list(&mut xdr).unwrap();
                                println!("Got nv_list:\n{:?}", nv_list);
                                match nv_list.find("vdev_tree") {
                                    Some(vdev_tree) => {
                                        println!("Got vdev_tree");

                                        let vdev_tree =
                                            if let NvValue::NvList(ref vdev_tree) = *vdev_tree {
                                                Some(vdev_tree)
                                            } else {
                                                None
                                            };

                                        match vdev_tree.unwrap().find("metaslab_array") {
                                            Some(metaslab_array) => {
                                                println!("Got metaslab_array");
                                                if let NvValue::Uint64(metaslab_array) = *metaslab_array {
                                                    // Get metaslab array dnode
                                                    let metaslab_array = metaslab_array as usize;
                                                    let ma_dnode: Result<DNodePhys, String> =
                                                        zfs.reader.read_type_array(zfs.mos.meta_dnode.get_blockptr(0), metaslab_array);
                                                    let ma_dnode = ma_dnode.unwrap(); // TODO

                                                    // Get a spacemap object id
                                                    let sm_id: Result<u64, String> =
                                                        zfs.reader.read_type_array(ma_dnode.get_blockptr(0), 0);
                                                    let sm_id = sm_id.unwrap(); // TODO

                                                    let sm_dnode: Result<DNodePhys, String> =
                                                        zfs.reader.read_type_array(zfs.mos.meta_dnode.get_blockptr(0), sm_id as usize);
                                                    let sm_dnode = sm_dnode.unwrap(); // TODO
                                                    let space_map_phys = SpaceMapPhys::from_bytes(sm_dnode.get_bonus()).unwrap(); // TODO
                                                    let space_map: Result<Vec<u8>, String> = zfs.reader
                                                                                                .read_block(sm_dnode.get_blockptr(0));

                                                    println!("got space map id: {:?}", sm_id);
                                                    println!("got space map dnode: {:?}", sm_dnode);
                                                    println!("got space map phys: {:?}", space_map_phys);
                                                    //println!("got space map: {:?}", &space_map.unwrap()[0..64]);

                                                    space_map::load_space_map_avl(&space_map::SpaceMap { size: 15 }, &space_map.unwrap());
                                                } else {
                                                    println!("Invalid metaslab_array NvValue type. Expected Uint64.");
                                                }
                                            },
                                            None => {
                                                println!("No `metaslab_array` in vdev_tree");
                                            },
                                        };
                                    },
                                    None => {
                                        println!("No `vdev_tree` in vdev_label nvpairs");
                                    },
                                }
                            },
                            Err(e) => { println!("Couldn't read vdev_label: {}", e); },
                        }
                    } else if command == "file" {
                        match args.get(1) {
                            Some(arg) => {
                                let file = zfs.read_file(arg);
                                match file {
                                    Some(file) => {
                                        println!("File contents: {}", str::from_utf8(&file).unwrap());
                                    },
                                    None => println!("Failed to read file"),
                                }
                            }
                            None => println!("Usage: file <path>"),
                        }
                    } else if command == "ls" {
                        match args.get(1) {
                            Some(arg) => {
                                let ls = zfs.ls(arg);
                                match ls {
                                    Some(ls) => {
                                        for item in &ls {
                                            print!("{}\t", item);
                                        }
                                    },
                                    None => println!("Failed to read directory"),
                                }
                            }
                            None => println!("Usage: ls <path>"),
                        }
                    } else if command == "mos" {
                        let ref uberblock = zfs.uberblock;
                        let mos_dva = uberblock.rootbp.dvas[0];
                        println!("DVA: {:?}", mos_dva);
                        println!("type: {:X}", uberblock.rootbp.object_type());
                        println!("checksum: {:X}", uberblock.rootbp.checksum());
                        println!("compression: {:X}", uberblock.rootbp.compression());
                        println!("Reading {} sectors starting at {}", mos_dva.asize(), mos_dva.sector());
                        let obj_set: Result<ObjectSetPhys, String> =
                            zfs.reader.read_type(&uberblock.rootbp);
                        if let Ok(ref obj_set) = obj_set {
                            println!("Got meta object set");
                            println!("os_type: {:X}", obj_set.os_type);
                            println!("meta dnode: {:?}\n", obj_set.meta_dnode);

                            println!("Reading MOS...");
                            let mos_block_ptr = obj_set.meta_dnode.get_blockptr(0);
                            let mos_array_dva = mos_block_ptr.dvas[0];

                            println!("DVA: {:?}", mos_array_dva);
                            println!("type: {:X}", mos_block_ptr.object_type());
                            println!("checksum: {:X}", mos_block_ptr.checksum());
                            println!("compression: {:X}", mos_block_ptr.compression());
                            println!("Reading {} sectors starting at {}", mos_array_dva.asize(), mos_array_dva.sector());
                            let dnode: Result<DNodePhys, String> =
                                zfs.reader.read_type_array(&mos_block_ptr, 1);
                            println!("Got MOS dnode array");
                            println!("dnode: {:?}", dnode);

                            if let Ok(ref dnode) = dnode {
                                println!("Reading object directory zap object...");
                                let zap_obj: Result<zap::MZapWrapper, String> =
                                    zfs.reader.read_type(dnode.get_blockptr(0));
                                println!("{:?}", zap_obj);
                            }
                        }
                    } else if command == "dump" {
                        match args.get(1) {
                            Some(arg) => {
                                let sector = arg.to_num();
                                println!("Dump sector: {}", sector);

                                let data = zfs.reader.zio.read(sector, 1);
                                for i in 0..data.len() {
                                    if i % 32 == 0 {
                                        print!("\n{:X}:", i);
                                    }
                                    if let Some(byte) = data.get(i) {
                                        print!(" {:X}", *byte);
                                    } else {
                                        println!(" !");
                                    }
                                }
                                print!("\n");
                            }
                            None => println!("No sector specified!"),
                        }
                    } else if command == "close" {
                        println!("Closing");
                        close = true;
                    } else {
                        println!("Commands: uber vdev_label mos file ls dump close");
                    }
                }
                None => {
                    if command == "open" {
                        match args.get(1) {
                            Some(arg) => {
                                match File::open(arg) {
                                    Some(file) => {
                                        let zfs = Zfs::new(file);
                                        if let Err(ref e) = zfs {
                                            println!("Error: {:?}", e);
                                        } else {
                                            println!("Open: {}", arg);
                                        }
                                        zfs_option = zfs.ok();
                                    },
                                    None => println!("File not found!"),
                                }
                            }
                            None => println!("No file specified!"),
                        }
                    } else {
                        println!("Commands: open");
                    }
                }
            }
            if close {
                zfs_option = None;
            }
        }
    }
}
