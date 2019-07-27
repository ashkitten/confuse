use crate::{data::ConfuseData, file_handle::FileHandleMap};
use fuse_mt::{
    DirectoryEntry, FileAttr, FileType, FilesystemMT, RequestInfo, ResultEmpty, ResultEntry,
    ResultOpen, ResultReaddir,
};
use inotify::{Inotify, WatchMask};
use libc::c_int;
use log::info;
use std::{
    cell::RefCell,
    cmp,
    ffi::OsStr,
    fs::File,
    ops::Deref,
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};
use time::Timespec;

pub struct Confuse {
    path: PathBuf,
    data: Arc<Mutex<RefCell<Arc<ConfuseData>>>>,
    file_handles: Arc<Mutex<RefCell<FileHandleMap>>>,
}

impl Confuse {
    pub fn new(path: PathBuf) -> Self {
        let data = serde_yaml::from_reader(File::open(&path).unwrap()).unwrap();
        let file_handles = Arc::new(Mutex::new(RefCell::new(FileHandleMap::new())));

        let path_moved = path.clone();
        let data_moved: Arc<Mutex<RefCell<_>>> = Arc::clone(&data);
        let file_handles_moved = Arc::clone(&file_handles);
        thread::spawn(move || loop {
            let mut inotify = Inotify::init().unwrap();
            inotify.add_watch(&path_moved, WatchMask::MODIFY).unwrap();
            inotify.read_events_blocking(&mut [0; 4096]).unwrap();

            info!("Detected file change, reloading filesystem");

            (*file_handles_moved.lock().unwrap())
                .borrow_mut()
                .drop_all();

            (*data_moved.lock().unwrap())
                .replace(serde_yaml::from_reader(File::open(&path_moved).unwrap()).unwrap());
        });

        Self {
            path,
            data,
            file_handles,
        }
    }

    fn get_data(&self, path: &Path, fh: Option<u64>) -> Result<Arc<ConfuseData>, c_int> {
        if let Some(fh) = fh {
            if let Some(handle) = (*self.file_handles.lock().unwrap()).borrow().get_handle(fh) {
                return Ok(Arc::clone(&handle.data));
            } else {
                return Err(libc::EBADF);
            }
        }

        let mut current_data: Arc<ConfuseData> =
            Arc::clone(&*Arc::clone(&self.data).lock().unwrap().borrow());

        for component in path.iter().skip(1) {
            match current_data.deref() {
                ConfuseData::List(list) => {
                    if component == OsStr::new(".list") {
                        return Ok(Arc::new(ConfuseData::Marker));
                    }

                    if let Some(data) = list.get(
                        str::parse::<usize>(component.to_str().ok_or(libc::ENOENT)?)
                            .map_err(|_| libc::ENOENT)?,
                    ) {
                        current_data = Arc::clone(data);
                    } else {
                        return Err(libc::ENOENT);
                    }
                }

                ConfuseData::Map(map) => {
                    if let Some(data) = map.get(component.to_str().ok_or(libc::ENOENT)?) {
                        current_data = Arc::clone(data);
                    } else {
                        return Err(libc::ENOENT);
                    }
                }

                _ => {
                    return Err(libc::ENOENT);
                }
            }
        }

        Ok(current_data)
    }
}

impl FilesystemMT for Confuse {
    fn init(&self, _req: RequestInfo) -> ResultEmpty {
        Ok(())
    }

    fn getattr(&self, _req: RequestInfo, path: &Path, fh: Option<u64>) -> ResultEntry {
        let data = self.get_data(path, fh)?;

        let size = match data.deref() {
            ConfuseData::List(list) => list.len() + 3, // ., .., .list
            ConfuseData::Map(map) => map.len() + 2,    // ., ..
            ConfuseData::Value(_) => data.to_string().len(),
            ConfuseData::Marker => 0,
        } as u64;

        let file = File::open(&self.path).unwrap();
        let stat = nix::sys::stat::fstat(file.as_raw_fd()).unwrap();

        Ok((
            Timespec::new(0, 0),
            FileAttr {
                size,
                blocks: 0,
                atime: Timespec::new(stat.st_atime, stat.st_atime_nsec as i32),
                mtime: Timespec::new(stat.st_mtime, stat.st_mtime_nsec as i32),
                ctime: Timespec::new(stat.st_ctime, stat.st_ctime_nsec as i32),
                crtime: Timespec::new(0, 0),
                kind: data.deref().into(),
                perm: (stat.st_mode & 0o0777) as u16,
                nlink: 0,
                uid: stat.st_uid,
                gid: stat.st_gid,
                rdev: 0,
                flags: 0,
            },
        ))
    }

    fn open(&self, _req: RequestInfo, path: &Path, flags: u32) -> ResultOpen {
        Ok((
            (*self.file_handles.lock().unwrap())
                .get_mut()
                .new_handle(self.get_data(path, None)?, flags),
            0,
        ))
    }

    fn release(
        &self,
        _req: RequestInfo,
        _path: &Path,
        fh: u64,
        _flags: u32,
        _lock_owner: u64,
        _flush: bool,
    ) -> ResultEmpty {
        (*self.file_handles.lock().unwrap())
            .get_mut()
            .remove_handle(fh);

        Ok(())
    }

    fn read(
        &self,
        _req: RequestInfo,
        path: &Path,
        fh: u64,
        offset: u64,
        size: u32,
        result: impl FnOnce(Result<&[u8], c_int>),
    ) {
        match self.get_data(path, Some(fh)) {
            Ok(data) => match data.deref() {
                ConfuseData::Marker => result(Ok(&[])),
                ConfuseData::Value(_) => {
                    let buf = data.to_string().into_bytes();
                    let buf = &buf[offset as usize..cmp::min(size as usize, buf.len())];
                    result(Ok(buf))
                }
                _ => result(Err(libc::EISDIR)),
            },
            _ => result(Err(libc::ENOENT)),
        }
    }

    fn opendir(&self, _req: RequestInfo, path: &Path, flags: u32) -> ResultOpen {
        Ok((
            (*self.file_handles.lock().unwrap())
                .get_mut()
                .new_handle(self.get_data(path, None)?, flags),
            0,
        ))
    }

    fn readdir(&self, _req: RequestInfo, path: &Path, fh: u64) -> ResultReaddir {
        match self.get_data(path, Some(fh))?.deref() {
            ConfuseData::List(list) => Ok(list
                .iter()
                .enumerate()
                .map(|(index, item)| DirectoryEntry {
                    name: index.to_string().into(),
                    kind: item.deref().into(),
                })
                .chain(vec![DirectoryEntry {
                    name: ".list".into(),
                    kind: FileType::RegularFile,
                }])
                .collect()),
            ConfuseData::Map(map) => Ok(map
                .iter()
                .map(|(name, item)| DirectoryEntry {
                    name: name.into(),
                    kind: item.deref().into(),
                })
                .collect()),
            _ => Err(libc::ENOSYS),
        }
    }

    fn releasedir(&self, _req: RequestInfo, _path: &Path, fh: u64, _flags: u32) -> ResultEmpty {
        (*self.file_handles.lock().unwrap())
            .get_mut()
            .remove_handle(fh);

        Ok(())
    }
}
