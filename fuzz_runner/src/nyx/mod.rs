pub mod aux_buffer;
pub mod ijon_data;
pub mod mem_barrier;
pub mod params;
pub mod qemu_process;

pub use qemu_process::QemuProcess;

use std::fs;
use std::path::PathBuf;

extern crate config;
use crate::config::{QemuKernelConfig, QemuSnapshotConfig, FuzzerConfig, SnapshotPath};

fn into_absolute_path(sharedir: &str) -> String{

    let srcdir = PathBuf::from(&sharedir);

    if srcdir.is_relative(){
        return fs::canonicalize(&srcdir).unwrap().to_str().unwrap().to_string();
    }
    else{
        return sharedir.to_string();
    }
}

pub fn qemu_process_new_from_kernel(sharedir: String, cfg: &QemuKernelConfig, fuzz_cfg: &FuzzerConfig) -> Result<QemuProcess, String> {
    let params = params::KernelVmParams {
        qemu_binary: cfg.qemu_binary.to_string(),
        kernel: cfg.kernel.to_string(),
        sharedir: into_absolute_path(&sharedir),
        ramfs: cfg.ramfs.to_string(),
        ram_size: fuzz_cfg.mem_limit,
        bitmap_size: fuzz_cfg.bitmap_size,
        debug: cfg.debug,
        dump_python_code_for_inputs: match fuzz_cfg.dump_python_code_for_inputs{
            None => false,
            Some(x) => x,
        },
        write_protected_input_buffer: fuzz_cfg.write_protected_input_buffer,
        cow_primary_size: fuzz_cfg.cow_primary_size, 
        ipt_filters: fuzz_cfg.ipt_filters,
        input_buffer_size: fuzz_cfg.input_buffer_size,
    };
    let qemu_id =  fuzz_cfg.thread_id;
    let qemu_params = params::QemuParams::new_from_kernel(&fuzz_cfg.workdir_path, qemu_id, &params, fuzz_cfg.threads > 1);
   
    /*
    if qemu_id == 0{
        qemu_process::QemuProcess::prepare_workdir(&fuzz_cfg.workdir_path, fuzz_cfg.seed_pattern.clone());
    }
    */
    return qemu_process::QemuProcess::new(qemu_params);
}

pub fn qemu_process_new_from_snapshot(sharedir: String, cfg: &QemuSnapshotConfig,  fuzz_cfg: &FuzzerConfig) -> Result<QemuProcess, String> {

    let snapshot_path = match &cfg.snapshot_path{
        SnapshotPath::Create(_x) => panic!(),
        SnapshotPath::Reuse(x) => SnapshotPath::Reuse(x.to_string()),
        SnapshotPath::DefaultPath => {
            if fuzz_cfg.thread_id == 0 {
                SnapshotPath::Create(format!("{}/snapshot/",fuzz_cfg.workdir_path))
            } else {
                SnapshotPath::Reuse(format!("{}/snapshot/",fuzz_cfg.workdir_path))
            }
        }
    };

    let params = params::SnapshotVmParams {
        qemu_binary: cfg.qemu_binary.to_string(),
        hda: cfg.hda.to_string(),
        sharedir: into_absolute_path(&sharedir),
        presnapshot: cfg.presnapshot.to_string(),
        ram_size: fuzz_cfg.mem_limit,
        bitmap_size: fuzz_cfg.bitmap_size,
        debug: cfg.debug,
        snapshot_path,
        dump_python_code_for_inputs: match fuzz_cfg.dump_python_code_for_inputs{
            None => false,
            Some(x) => x,
        },
        write_protected_input_buffer: fuzz_cfg.write_protected_input_buffer,
        cow_primary_size: fuzz_cfg.cow_primary_size, 
        ipt_filters: fuzz_cfg.ipt_filters,
        input_buffer_size: fuzz_cfg.input_buffer_size,
    };
    let qemu_id = fuzz_cfg.thread_id;
    let qemu_params = params::QemuParams::new_from_snapshot(&fuzz_cfg.workdir_path, qemu_id, fuzz_cfg.cpu_pin_start_at, &params, fuzz_cfg.threads > 1);

    return qemu_process::QemuProcess::new(qemu_params);
}


#[cfg(test)]
mod tests {
    //use crate::aux_buffer::*;
    use super::params::*;
    use super::qemu_process::*;
    //use std::{thread, time};

    #[test]
    fn it_works() {
        let workdir = "/tmp/workdir_test";
        let params = KernelVmParams {
            qemu_binary: "/home/kafl/NEW2/QEMU-PT_4.2.0/x86_64-softmmu/qemu-system-x86_64"
                .to_string(),
            kernel: "/home/kafl/Target-Components/linux_initramfs/bzImage-linux-4.15-rc7"
                .to_string(),
            ramfs: "/home/kafl/Target-Components/linux_initramfs/init.cpio.gz".to_string(),
            sharedir: "foo! invalid".to_string(),
            ram_size: 1000,
            bitmap_size: 0x1 << 16,
            debug: false,
            dump_python_code_for_inputs: false,
            write_protected_input_buffer: false,
        };
        let qemu_id = 1;
        let qemu_params = QemuParams::new_from_kernel(workdir, qemu_id, &params);

        QemuProcess::prepare_workdir(&workdir, None);

        let mut qemu_process = QemuProcess::new(qemu_params);

        for _i in 0..100 {
            qemu_process.send_payload();
        }
        println!("test done");
    }
}
