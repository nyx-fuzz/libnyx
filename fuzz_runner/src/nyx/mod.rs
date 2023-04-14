pub mod aux_buffer;
pub mod ijon_data;
pub mod mem_barrier;
pub mod params;
pub mod qemu_process;

pub use qemu_process::QemuProcess;

use std::fs;
use std::path::PathBuf;

extern crate config;

fn into_absolute_path(sharedir: &str) -> String{

    let srcdir = PathBuf::from(&sharedir);

    if srcdir.is_relative(){
        return fs::canonicalize(&srcdir).unwrap().to_str().unwrap().to_string();
    }
    else{
        return sharedir.to_string();
    }
}

pub fn qemu_process_new(sharedir: String, cfg: &config::Config) -> Result<QemuProcess, String> {


    let qemu_params = params::QemuParams::new(into_absolute_path(&sharedir), cfg);
    return qemu_process::QemuProcess::new(qemu_params);
}
