use clap::{crate_version, App, Arg};
use fuse_mt::FuseMT;
use std::ffi::OsStr;

mod data;
mod file_handle;
mod fs;

use fs::Confuse;

fn main() {
    env_logger::builder().default_format_timestamp(false).init();

    let matches = App::new("ConFUSE")
        .version(crate_version!())
        .about("A YAML configuration FUSE filesystem")
        .arg(
            Arg::with_name("file")
                .help("YAML file to mount")
                .required(true),
        )
        .arg(
            Arg::with_name("mountpoint")
                .help("Path to mount")
                .required(true),
        )
        .get_matches();

    let file = matches.value_of("file").unwrap();
    let mountpoint = matches.value_of("mountpoint").unwrap();

    let confuse = FuseMT::new(Confuse::new(file.into()), 0);

    fuse_mt::mount(
        confuse,
        &mountpoint,
        &[OsStr::new("-o"), OsStr::new("auto_unmount")],
    )
    .unwrap();
}
