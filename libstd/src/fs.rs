use io::{Read, Result, Write, Seek, SeekFrom};
use path::PathBuf;
use str;
use string::{String, ToString};
use vec::Vec;

use system::error::{Error, EACCES};
use system::syscall::{sys_open, sys_dup, sys_close, sys_fpath, sys_ftruncate, sys_read,
              sys_write, sys_lseek, sys_fsync, sys_mkdir, sys_unlink};
use system::syscall::{O_RDWR, O_CREAT, O_TRUNC, SEEK_SET, SEEK_CUR, SEEK_END};

/// A Unix-style file
pub struct File {
    /// The id for the file
    fd: usize,
}

impl File {
    pub unsafe fn from_fd(fd_muxed: usize) -> Result<File> {
        match Error::demux(fd_muxed) {
            Ok(fd) => Ok(File { fd: fd }),
            Err(err) => Err(err),
        }
    }

    /// Open a new file using a path
    pub fn open(path: &str) -> Result<File> {
        let path_c = path.to_string() + "\0";
        unsafe { File::from_fd(sys_open(path_c.as_ptr(), O_RDWR, 0)) }
    }

    /// Create a new file using a path
    pub fn create(path: &str) -> Result<File> {
        let path_c = path.to_string() + "\0";
        unsafe { File::from_fd(sys_open(path_c.as_ptr(), O_CREAT | O_RDWR | O_TRUNC, 0)) }
    }

    /// Duplicate the file
    pub fn dup(&self) -> Result<File> {
        unsafe { File::from_fd(sys_dup(self.fd)) }
    }

    /// Get the canonical path of the file
    pub fn path(&self) -> Result<PathBuf> {
        let mut buf: [u8; 4096] = [0; 4096];
        match Error::demux(unsafe { sys_fpath(self.fd, buf.as_mut_ptr(), buf.len()) }) {
            Ok(count) => {
                Ok(PathBuf::from(unsafe { String::from_utf8_unchecked(Vec::from(&buf[0..count])) }))
            }
            Err(err) => Err(err),
        }
    }

    /// Flush the file data and metadata
    pub fn sync_all(&mut self) -> Result<()> {
        match Error::demux(unsafe { sys_fsync(self.fd) }) {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// Flush the file data
    pub fn sync_data(&mut self) -> Result<()> {
        match Error::demux(unsafe { sys_fsync(self.fd) }) {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// Truncates the file
    pub fn set_len(&mut self, size: usize) -> Result<()> {
        match Error::demux(unsafe { sys_ftruncate(self.fd, size) }) {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Error::demux(unsafe { sys_read(self.fd, buf.as_mut_ptr(), buf.len()) })
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Error::demux(unsafe { sys_write(self.fd, buf.as_ptr(), buf.len()) })
    }
}

impl Seek for File {
    /// Seek a given position
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let (whence, offset) = match pos {
            SeekFrom::Start(offset) => (SEEK_SET, offset as isize),
            SeekFrom::Current(offset) => (SEEK_CUR, offset as isize),
            SeekFrom::End(offset) => (SEEK_END, offset as isize),
        };

        match Error::demux(unsafe { sys_lseek(self.fd, offset, whence) }) {
            Ok(position) => Ok(position as u64),
            Err(err) => Err(err),
        }
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe {
            sys_close(self.fd);
        }
    }
}

pub struct FileType {
    dir: bool,
    file: bool,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.dir
    }

    pub fn is_file(&self) -> bool {
        self.file
    }
}

pub struct DirEntry {
    path: PathBuf,
    dir: bool,
    file: bool,
}

impl DirEntry {
    pub fn file_name(&self) -> &PathBuf {
        &self.path
    }

    pub fn file_type(&self) -> Result<FileType> {
        Ok(FileType {
            dir: self.dir,
            file: self.file,
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

pub struct ReadDir {
    file: File,
}

impl Iterator for ReadDir {
    type Item = Result<DirEntry>;
    fn next(&mut self) -> Option<Result<DirEntry>> {
        let mut path = String::new();
        let mut buf: [u8; 1] = [0; 1];
        loop {
            match self.file.read(&mut buf) {
                Ok(0) => break,
                Ok(count) => {
                    if buf[0] == 10 {
                        break;
                    } else {
                        path.push_str(unsafe { str::from_utf8_unchecked(&buf[..count]) });
                    }
                }
                Err(_err) => break,
            }
        }
        if path.is_empty() {
            None
        } else {
            let dir = path.ends_with('/');
            if dir {
                path.pop();
            }
            Some(Ok(DirEntry {
                path: PathBuf::from(path),
                dir: dir,
                file: !dir,
            }))
        }
    }
}

/// Create a new directory, using a path
/// The default mode of the directory is 744
pub fn create_dir(path: &str) -> Result<()> {
    let path_c = path.to_string() + "\0";
    match Error::demux(unsafe { sys_mkdir(path_c.as_ptr(), 755) }) {
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    }
}

pub fn read_dir(path: &str) -> Result<ReadDir> {
    let file_result = if path.is_empty() || path.ends_with('/') {
        File::open(path)
    } else {
        File::open(&(path.to_string() + "/"))
    };

    match file_result {
        Ok(file) => Ok(ReadDir { file: file }),
        Err(err) => Err(err),
    }
}

pub fn remove_dir(_path: &str) -> Result<()> {
    Err(Error::new(EACCES))
}

pub fn remove_file(path: &str) -> Result<()> {
    let path_c = path.to_string() + "\0";
    match Error::demux(unsafe { sys_unlink(path_c.as_ptr()) }) {
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    }
}
