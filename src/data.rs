use fuse_mt::FileType;
use serde::Deserialize;
use serde_yaml::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ConfuseData {
    List(Vec<Arc<ConfuseData>>),
    Map(HashMap<String, Arc<ConfuseData>>),
    Value(Mutex<Value>),
}

// TODO: maybe a wrapper struct for Value to implement ToString
impl ToString for ConfuseData {
    fn to_string(&self) -> String {
        match self {
            ConfuseData::Value(val) => {
                let val = &*val.lock().unwrap();
                // TODO: this is a bit hacky
                match val {
                    Value::Null => "~".to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => format!("{:?}", s),
                    _ => unreachable!("Value should never be a Sequence or Mapping"),
                }
            }
            _ => unimplemented!("Converting lists or maps to string is not supported"),
        }
    }
}

impl Into<FileType> for &ConfuseData {
    fn into(self) -> FileType {
        match self {
            ConfuseData::Value(_) => FileType::RegularFile,
            ConfuseData::List(_) | ConfuseData::Map(_) => FileType::Directory,
        }
    }
}
