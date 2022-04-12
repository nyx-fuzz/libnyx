extern crate libc;

use config::{Config, FuzzRunnerConfig};

use fuzz_runner::nyx::qemu_process_new_from_kernel;
use fuzz_runner::nyx::qemu_process_new_from_snapshot;
use fuzz_runner::nyx::qemu_process::QemuProcess;
use fuzz_runner::nyx::aux_buffer::{NYX_SUCCESS, NYX_CRASH, NYX_TIMEOUT, NYX_INPUT_WRITE, NYX_ABORT};

use std::fmt;

pub mod ffi;

#[repr(C)]
#[derive(Debug)]
pub enum NyxReturnValue {
    Normal,
    Crash,
    Asan,
    Timeout,
    InvalidWriteToPayload,
    Error,
    IoError,    // QEMU process has died for some reason
    Abort,      // Abort hypercall called
}

impl fmt::Display for NyxReturnValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        let nyx_return_value_str = match self {
            NyxReturnValue::Normal                => "Normal",
            NyxReturnValue::Crash                 => "Crash",
            NyxReturnValue::Timeout               => "Timeout",
            NyxReturnValue::InvalidWriteToPayload => "InvalidWriteToPayload",
            NyxReturnValue::Abort                 => "Abort",
            NyxReturnValue::Error                 => "Error",
            _                                     => "Unknown",
        };

        write!(f, "{}", nyx_return_value_str)
    }
}

pub struct NyxProcess {
    process: QemuProcess,
}

#[derive(Clone, Debug)]
pub struct NyxConfig {
    config: Config,
}

impl NyxConfig {
    pub fn load(sharedir: &str) -> Result<NyxConfig, String> {
        match Config::new_from_sharedir(&sharedir){
            Ok(x) => Ok(NyxConfig{
                config: x
            }),
            Err(x) => Err(x),
        }
    }

    pub fn qemu_binary_path(&self) -> Option<String>{
        let process_cfg= match self.config.runner.clone() {
            FuzzRunnerConfig::QemuKernel(cfg) => cfg,
            _ => return None,
        };
        return Some(process_cfg.qemu_binary);
    }

    pub fn kernel_image_path(&self) -> Option<String>{
        let process_cfg= match self.config.runner.clone() {
            FuzzRunnerConfig::QemuKernel(cfg) => cfg,
            _ => return None,
        };
        return Some(process_cfg.kernel);
    }

    pub fn ramfs_image_path(&self) -> Option<String>{
        let process_cfg= match self.config.runner.clone() {
            FuzzRunnerConfig::QemuKernel(cfg) => cfg,
            _ => return None,
        };
        return Some(process_cfg.ramfs);
    }

    pub fn timeout(&self) -> std::time::Duration {
        self.config.fuzz.time_limit
    }

    pub fn spec_path(&self) -> String{
        self.config.fuzz.spec_path.clone()
    }

    pub fn bitmap_size(&self) -> usize{
        self.config.fuzz.bitmap_size
    }

    pub fn workdir_path(&self) -> &str {
        &self.config.fuzz.workdir_path
    }

    pub fn set_workdir_path(&mut self, path: String) {
        self.config.fuzz.workdir_path = path;
    }

    pub fn dict(&self) -> Vec<Vec<u8>> {
        self.config.fuzz.dict.clone()
    }
}

impl fmt::Display for NyxConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:#?})", self.config)
    }
}

impl NyxProcess {

    fn start_process(sharedir: &str, workdir: &str, fuzzer_config: Config,  worker_id: u32) -> Result<QemuProcess, String> {

        let mut config = fuzzer_config.fuzz;
        let runner_cfg = fuzzer_config.runner;
    
        config.workdir_path = format!("{}", workdir);
    
        let sdir = sharedir.clone();
    
        if worker_id == 0 {
            QemuProcess::prepare_workdir(&config.workdir_path, config.seed_path.clone());
        }
        else{
            QemuProcess::wait_for_workdir(&config.workdir_path);
        }
    
        match runner_cfg.clone() {
            FuzzRunnerConfig::QemuSnapshot(cfg) => {
                qemu_process_new_from_snapshot(sdir.to_string(), &cfg, &config)            
            },
            FuzzRunnerConfig::QemuKernel(cfg) => {
                qemu_process_new_from_kernel(sdir.to_string(), &cfg, &config)
            }
        }
    }

    fn process_start(sharedir: &str, workdir: &str, worker_id: u32, cpu_id: u32, create_snapshot: bool, input_buffer_size: Option<u32>, input_buffer_write_protection: bool) -> Result<NyxProcess, String> {
        let mut cfg: Config = match Config::new_from_sharedir(&sharedir){
            Ok(x) => x,
            Err(msg) => {
                return Err(format!("[!] libnyx config reader error: {}", msg));
            }
        };
    
        cfg.fuzz.write_protected_input_buffer = input_buffer_write_protection;
    
        /* todo: add sanity check */
        cfg.fuzz.cpu_pin_start_at = cpu_id as usize;
    
        match input_buffer_size{
            Some(x) => { cfg.fuzz.input_buffer_size = x as usize; },
            None => {},
        }
    
        cfg.fuzz.thread_id = worker_id as usize;
        cfg.fuzz.threads = if create_snapshot { 2 as usize } else { 1 as usize };
            
        cfg.fuzz.workdir_path = format!("{}", workdir);

        match Self::start_process(sharedir, workdir, cfg,  worker_id){
            Ok(x) => Ok(NyxProcess{
                process: x,
            }),
            Err(x) => Err(x),
        }
    }

    pub fn from_config(sharedir: &str, config: &NyxConfig, worker_id: u32, create_snapshot: bool) -> Result<NyxProcess, String>{
        let workdir = config.config.fuzz.workdir_path.clone();

        let mut config= config.clone();
        config.config.fuzz.threads = if create_snapshot { 2 as usize } else { 1 as usize };
        config.config.fuzz.thread_id = worker_id as usize;

        match Self::start_process(sharedir, &workdir, config.config.clone(), worker_id) {
            Ok(x) => Ok(NyxProcess{
                process: x,
            }),
            Err(x) => Err(x),
        }
    }

    pub fn new(sharedir: &str, workdir: &str, cpu_id: u32, input_buffer_size: u32, input_buffer_write_protection: bool) -> Result<NyxProcess, String> {
        Self::process_start(sharedir, workdir, 0, cpu_id, false, Some(input_buffer_size), input_buffer_write_protection)
    }
    
    pub fn new_parent(sharedir: &str, workdir: &str, cpu_id: u32, input_buffer_size: u32, input_buffer_write_protection: bool) -> Result<NyxProcess, String> {
        Self::process_start(sharedir, workdir, 0, cpu_id, true, Some(input_buffer_size), input_buffer_write_protection)
    }
    
    pub fn new_child(sharedir: &str, workdir: &str, cpu_id: u32, worker_id: u32) -> Result<NyxProcess, String> {
        if worker_id == 0 {
            println!("[!] libnyx failed -> worker_id=0 cannot be used for child processes");
            Err("worker_id=0 cannot be used for child processes".to_string())
        }
        else{
            Self::process_start(sharedir, workdir, worker_id, cpu_id, true, None, false)
        }
    }

    pub fn aux_buffer_as_mut_ptr(&self) -> *mut u8 {
        std::ptr::addr_of!(self.process.aux.header.magic) as *mut u8
    }
    
    pub fn input_buffer(&self) -> &[u8] {
        self.process.payload
    }
    
    pub fn input_buffer_mut(&mut self) -> &mut [u8] {
        self.process.payload
    }

    pub fn input_buffer_size(&mut self) -> usize {
        self.process.payload.len()
    }
    
    pub fn bitmap_buffer(&self) -> &[u8] {
        &self.process.bitmap[.. self.process.bitmap_size]
    }
    
    pub fn bitmap_buffer_mut(&mut self) -> &mut [u8] {
        &mut self.process.bitmap[.. self.process.bitmap_size]
    }

    pub fn bitmap_buffer_size(&self) -> usize {
        self.process.bitmap_size
    }

    pub fn ijon_buffer(&self) -> &[u8] {
        self.process.ijon_buffer
    }
    
    pub fn shutdown(&mut self) {
        self.process.shutdown();
    }
    
    pub fn option_set_reload_mode(&mut self, enable: bool) {
        self.process.aux.config.reload_mode = if enable {1} else {0};
    }

    pub fn option_set_redqueen_mode(&mut self, enable: bool) {
        self.process.aux.config.redqueen_mode = if enable {1} else {0};
    }

    pub fn option_set_trace_mode(&mut self, enable: bool) {
        self.process.aux.config.trace_mode = if enable {1} else {0};
    }

    pub fn option_set_delete_incremental_snapshot(&mut self, enable: bool) {
        self.process.aux.config.discard_tmp_snapshot = if enable {1} else {0};
    }

    pub fn option_set_timeout(&mut self, timeout_sec: u8, timeout_usec: u32) {
        self.process.aux.config.timeout_sec = timeout_sec;
        self.process.aux.config.timeout_usec = timeout_usec;
    }
    
    pub fn option_apply(&mut self) {
        self.process.aux.config.changed = 1;
    }

    pub fn aux_misc(&self) -> Vec<u8>{
        self.process.aux.misc.as_slice().to_vec()
    }

    pub fn aux_tmp_snapshot_created(&self) -> bool {
        self.process.aux.result.tmp_snapshot_created != 0
    }

    pub fn aux_string(&self) -> String {
        let len = self.process.aux.misc.len;
        String::from_utf8_lossy(&self.process.aux.misc.data[0..len as usize]).to_string()
    }
     
    pub fn exec(&mut self) -> NyxReturnValue {
        match self.process.send_payload(){
            Err(_) =>  NyxReturnValue::IoError,
            Ok(_) => {
                match self.process.aux.result.exec_result_code {
                    NYX_SUCCESS     => NyxReturnValue::Normal,
                    NYX_CRASH       => NyxReturnValue::Crash,
                    NYX_TIMEOUT     => NyxReturnValue::Timeout,
                    NYX_INPUT_WRITE => NyxReturnValue::InvalidWriteToPayload,
                    NYX_ABORT       => NyxReturnValue::Abort,
                    _                  => NyxReturnValue::Error,
                }
            }
        }
    }

    pub fn set_input_ptr(&mut self, buffer: *const u8, size: u32) {
        unsafe{
            std::ptr::copy(&size, self.process.payload.as_mut_ptr() as *mut u32, 1 as usize);
            std::ptr::copy(buffer, self.process.payload[std::mem::size_of::<u32>()..].as_mut_ptr(), std::cmp::min(size as usize, self.input_buffer_size()));
        }
    }
    
    pub fn set_input(&mut self, buffer: &[u8], size: u32) {
        self.set_input_ptr(buffer.as_ptr(), size);
    }
}
