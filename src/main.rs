use clap::{crate_version, App, Arg};
use fuse_mt::FuseMT;
use log::info;
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
        .arg(
            Arg::with_name("options")
                .short("o")
                .help("Pass additional options to FUSE")
                .takes_value(true)
                .multiple(true),
        )
        .get_matches();

    let file = matches.value_of("file").unwrap();
    let mountpoint = matches.value_of("mountpoint").unwrap();
    let extra_options = matches
        .values_of_os("options")
        .unwrap_or(Default::default());

    let options = ["auto_unmount", "default_permissions"]
        .iter()
        .map(OsStr::new)
        .chain(extra_options)
        .flat_map(|option| vec![OsStr::new("-o"), option])
        .collect::<Vec<_>>();

    info!("Starting up with FUSE options: {:?}", options);

    let confuse = FuseMT::new(Confuse::new(file.into()), 0);
    fuse_mt::mount(confuse, &mountpoint, &options).unwrap();
}
