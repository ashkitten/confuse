use crate::data::ConfuseData;
use std::{collections::BTreeMap, sync::Arc};

pub struct FileHandle {
    pub data: Arc<ConfuseData>,
    pub flags: u32,
}

pub struct FileHandleMap {
    file_handles: BTreeMap<u64, FileHandle>,
    counter: u64,
}

impl FileHandleMap {
    pub fn new() -> Self {
        Self {
            file_handles: BTreeMap::new(),
            counter: 0,
        }
    }

    pub fn new_handle(&mut self, data: Arc<ConfuseData>, flags: u32) -> u64 {
        let id = self.counter;
        self.counter += 1;
        self.file_handles.insert(id, FileHandle { data, flags });
        id
    }

    pub fn remove_handle(&mut self, id: u64) {
        self.file_handles.remove(&id);
    }

    pub fn get_handle(&self, id: u64) -> Option<&FileHandle> {
        self.file_handles.get(&id)
    }

    /// drop all file handles without resetting the monotonic counter
    pub fn drop_all(&mut self) {
        self.file_handles = BTreeMap::new();
    }
}
