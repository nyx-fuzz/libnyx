use std::time::Duration;
use crate::{config::{Config, FuzzRunnerConfig, QemuNyxRole}, QemuProcess};

pub struct QemuParams {
    pub cmd: Vec<String>,
    pub qemu_aux_buffer_filename: String,
    pub control_filename: String,
    pub workdir: String,
    pub qemu_id: usize,
    pub bitmap_size: usize,
    pub payload_size: usize,

    pub dump_python_code_for_inputs: bool,
    pub write_protected_input_buffer: bool,
    pub cow_primary_size: Option<u64>,
    pub hprintf_fd: Option<i32>,

    pub aux_buffer_size: usize,
    pub time_limit: Duration,
}

impl QemuParams {

    pub fn new(sharedir: String, fuzzer_config: &Config) -> QemuParams {

        let mut cmd = vec![];
        let qemu_id =  fuzzer_config.runtime.worker_id();
        
        let workdir = &fuzzer_config.fuzz.workdir_path;

        let debug = fuzzer_config.runtime.debug_mode();

        let qemu_aux_buffer_filename = format!("{}/aux_buffer_{}", workdir, qemu_id);
        let control_filename = format!("{}/interface_{}", workdir, qemu_id);

        match fuzzer_config.runner.clone(){
            FuzzRunnerConfig::QemuKernel(x) => {
                cmd.push(x.qemu_binary.to_string());
                cmd.push("-kernel".to_string());
                cmd.push(x.kernel.to_string());
        
                cmd.push("-initrd".to_string());
                cmd.push(x.ramfs.to_string());
        
                cmd.push("-append".to_string());
                cmd.push("nokaslr oops=panic nopti ignore_rlimit_data".to_string());
            },
            FuzzRunnerConfig::QemuSnapshot(x) => {
                cmd.push(x.qemu_binary.to_string());
                cmd.push("-drive".to_string());
                cmd.push(format!("file={},index=0,media=disk", x.hda.to_string()));
            },
        }

        /* generic QEMU-Nyx parameters */
        if !debug{
            cmd.push("-display".to_string());
            cmd.push("none".to_string());
        } else {
            cmd.push("-vnc".to_string());
            cmd.push(format!(":{}",qemu_id));
        }

        cmd.push("-serial".to_string());
        if debug {
            cmd.push("mon:stdio".to_string());
        } else {
            match fuzzer_config.runner {
                FuzzRunnerConfig::QemuKernel(_) => {
                    cmd.push("none".to_string());
                }
                FuzzRunnerConfig::QemuSnapshot(_) => {
                    cmd.push("stdio".to_string());
                }
            }
        }

        cmd.push("-enable-kvm".to_string());

        cmd.push("-net".to_string());
        cmd.push("none".to_string());

        cmd.push("-k".to_string());
        cmd.push("de".to_string());

        cmd.push("-m".to_string());
        cmd.push(fuzzer_config.fuzz.mem_limit.to_string());

        cmd.push("-chardev".to_string());
        cmd.push(format!(
            "socket,server,path={},id=nyx_interface",
            control_filename
        ));
    
        cmd.push("-device".to_string());
        let mut nyx_ops = format!("nyx,chardev=nyx_interface");
        nyx_ops += &format!(",bitmap_size={}", fuzzer_config.fuzz.bitmap_size);
        nyx_ops += &format!(",input_buffer_size={}", fuzzer_config.fuzz.input_buffer_size);
        nyx_ops += &format!(",worker_id={}", qemu_id);
        nyx_ops += &format!(",workdir={}", workdir);
        nyx_ops += &format!(",sharedir={}", sharedir);
        nyx_ops += &format!(",aux_buffer_size={}", fuzzer_config.runtime.aux_buffer_size());

        let mut i = 0;
        for filter in fuzzer_config.fuzz.ipt_filters{
            if filter.a != 0 && filter.b != 0 {
                nyx_ops += &format!(",ip{}_a={},ip{}_b={}", i, filter.a, i, filter.b);
            i += 1;
            }
        }

        if fuzzer_config.fuzz.cow_primary_size.is_some(){
            nyx_ops += &format!(",cow_primary_size={}", fuzzer_config.fuzz.cow_primary_size.unwrap());
        }

        cmd.push(nyx_ops);

        cmd.push("-machine".to_string());
        cmd.push("kAFL64-v1".to_string());

        cmd.push("-cpu".to_string());
        cmd.push("kAFL64-Hypervisor-v1".to_string());


        if fuzzer_config.runtime.reuse_root_snapshot_path().is_some() {
            cmd.push("-fast_vm_reload".to_string());
            cmd.push(format!("path={},load=on", fuzzer_config.runtime.reuse_root_snapshot_path().unwrap()));
        }
        else{
            match fuzzer_config.runner.clone(){
                FuzzRunnerConfig::QemuKernel(_) => {

                    match fuzzer_config.runtime.process_role() {
                        QemuNyxRole::StandAlone => {
                            cmd.push("-fast_vm_reload".to_string());
                            cmd.push(format!("path={}/snapshot/,load=off,skip_serialization=on", workdir));
                        },
                        QemuNyxRole::Parent => {
                            cmd.push("-fast_vm_reload".to_string());
                            cmd.push(format!("path={}/snapshot/,load=off", workdir));
                        },
                        QemuNyxRole::Child => {
                            cmd.push("-fast_vm_reload".to_string());
                            cmd.push(format!("path={}/snapshot/,load=on", workdir));
                        },
                    };
                },
                FuzzRunnerConfig::QemuSnapshot(x) => {

                    match fuzzer_config.runtime.process_role() {
                        QemuNyxRole::StandAlone => {
                            cmd.push("-fast_vm_reload".to_string());
                            if x.presnapshot.is_empty() {
                                cmd.push(format!("path={}/snapshot/,load=off,skip_serialization=on", workdir));
                            } else {
                                cmd.push(format!("path={}/snapshot/,load=off,pre_path={},skip_serialization=on", workdir, x.presnapshot));
                            }
                        },
                        QemuNyxRole::Parent => {
                            cmd.push("-fast_vm_reload".to_string());
                            cmd.push(format!("path={}/snapshot/,load=off,pre_path={}", workdir, x.presnapshot));
                        },
                        QemuNyxRole::Child => {
                            cmd.push("-fast_vm_reload".to_string());
                            cmd.push(format!("path={}/snapshot/,load=on", workdir));
                        },
                    };
                },
            }
        }

        match fuzzer_config.runtime.process_role() {
            QemuNyxRole::StandAlone | QemuNyxRole::Parent => {
                assert!(qemu_id == 0);
                QemuProcess::prepare_workdir(workdir, fuzzer_config.fuzz.seed_path.clone());
            },
            QemuNyxRole::Child => {
                QemuProcess::wait_for_workdir(workdir);
            },
        };


        return QemuParams {
            cmd,
            qemu_aux_buffer_filename,
            control_filename,
            workdir: workdir.to_string(),
            qemu_id,
            bitmap_size: fuzzer_config.fuzz.bitmap_size,
            payload_size: fuzzer_config.fuzz.input_buffer_size,
            dump_python_code_for_inputs: match fuzzer_config.fuzz.dump_python_code_for_inputs{
                None => false,
                Some(x) => x,
            },
            write_protected_input_buffer: fuzzer_config.fuzz.write_protected_input_buffer,
            cow_primary_size: fuzzer_config.fuzz.cow_primary_size,
            hprintf_fd: fuzzer_config.runtime.hprintf_fd(),
            aux_buffer_size: fuzzer_config.runtime.aux_buffer_size(),
            time_limit: fuzzer_config.fuzz.time_limit
        }
    }

}
