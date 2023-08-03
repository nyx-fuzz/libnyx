/* 
    libnyx Rust API

    Copyright (C) 2021 Sergej Schumilo
    This file is part of libnyx.

    libnyx is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.
    libnyx is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.
    You should have received a copy of the GNU General Public License
    along with libnyx.  If not, see <http://www.gnu.org/licenses/>.
 */
extern crate libc;

use config::{Config, FuzzRunnerConfig, QemuNyxRole};

use fuzz_runner::nyx::qemu_process::QemuProcess;
use fuzz_runner::nyx::aux_buffer::{NYX_SUCCESS, NYX_CRASH, NYX_TIMEOUT, NYX_INPUT_WRITE, NYX_ABORT};
use libc::fcntl;

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

#[repr(C)]
#[derive(Debug)]
pub enum NyxProcessRole {
    StandAlone,
    Parent,
    Child,
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
    sharedir_path: String,
}

impl NyxConfig {

    /* Loads a given Nyx share-dir and returns a result object containing the config object.
     * The config object is later used to access the config object via specific config functions.
     */
    pub fn load(sharedir: &str) -> Result<NyxConfig, String> {
        /* TODO: perform some additional sanity checks on the sharedir (such as checking if the bootstrap scripts exist) */
        match Config::new_from_sharedir(&sharedir){
            Ok(x) => Ok(NyxConfig{
                config: x,
                sharedir_path: sharedir.to_string()
            }),
            Err(x) => Err(x),
        }
    }

    /* Simple debug function to print the entire config object to stdout. */
    pub fn print(&self){
        println!("[*] Nyx config (share-dir: {}):", self.sharedir_path());
        println!("  - workdir_path                  -> {}", self.workdir_path());
        println!("  - input_buffer_size             -> {}", self.input_buffer_size());
        println!("  - input_buffer_write_protection -> {}", self.input_buffer_write_protection());
        println!("  - hprintf_fd                    -> {}", self.hprintf_fd());
        println!("  - process_role:                 -> {:?}", self.process_role());         

    }

    /* Returns the path to the actual sharedir. */
    pub fn sharedir_path(&self) -> String {
        self.sharedir_path.clone()
    }

    /* Returns the path to the configured qemu binary. */
    pub fn qemu_binary_path(&self) -> Option<String>{
        let process_cfg= match self.config.runner.clone() {
            FuzzRunnerConfig::QemuKernel(cfg) => cfg,
            _ => return None,
        };
        return Some(process_cfg.qemu_binary);
    }

    /* Returns the path to the configured kernel image (if Nyx is configured to run in kernel mode). */
    pub fn kernel_image_path(&self) -> Option<String>{
        let process_cfg= match self.config.runner.clone() {
            FuzzRunnerConfig::QemuKernel(cfg) => cfg,
            _ => return None,
        };
        return Some(process_cfg.kernel);
    }

    /* Returns the path to the configured initrd image (if Nyx is configured to run in kernel mode). */
    pub fn ramfs_image_path(&self) -> Option<String>{
        let process_cfg= match self.config.runner.clone() {
            FuzzRunnerConfig::QemuKernel(cfg) => cfg,
            _ => return None,
        };
        return Some(process_cfg.ramfs);
    }

    /* Returns the configured timeout threshold as a std::time::Duration object. */
    pub fn timeout(&self) -> std::time::Duration {
        self.config.fuzz.time_limit
    }

    /* Returns the configured spec path (deprecated). */
    pub fn spec_path(&self) -> String{
        self.config.fuzz.spec_path.clone()
    }

    /* Returns the configured trace bitmap size (might be reconfigured later by the agent). */
    pub fn bitmap_size(&self) -> usize{
        self.config.fuzz.bitmap_size
    }

    /* Returns the configured workdir path. */
    pub fn workdir_path(&self) -> &str {
        &self.config.fuzz.workdir_path
    }

    /* Returns the actual size of the input buffer. */
    pub fn input_buffer_size(&self) -> usize {
        self.config.fuzz.input_buffer_size
    }

    /* Returns the config value of the input buffer write protection of the agent (guest). */
    pub fn input_buffer_write_protection(&self) -> bool {
        self.config.fuzz.write_protected_input_buffer
    }

    /* Sets the path to the workdir. */
    pub fn set_workdir_path(&mut self, path: String) {
        self.config.fuzz.workdir_path = path;
    }

    /* Set the size of the input buffer (must be a multiple of x86_64_PAGE_SIZE -> 4096). */
    pub fn set_input_buffer_size(&mut self, size: usize) {
        if size % 0x1000 != 0 {
            /* TODO: return error */
            panic!("[ ] Input buffer size must be a multiple of x86_64_PAGE_SIZE (4096)!");
        }
        self.config.fuzz.input_buffer_size = size;
    }

    /* Set the input buffer write protection of the agent (guest). */
    pub fn set_input_buffer_write_protection(&mut self, write_protected: bool) {
        self.config.fuzz.write_protected_input_buffer = write_protected;
    }

    /* Returns the current configured FD to redirect hprintf() calls to (returns -1 if None is set). */
    pub fn hprintf_fd(&self) -> i32 {
        /* TODO: fix me */
        match self.config.runtime.hprintf_fd() {
            Some(fd) => fd,
            None => -1,
        }
    }

    /* Sets the FD to redirect hprintf() calls to (must be a valid FD and must not be closed after this call). */
    pub fn set_hprintf_fd(&mut self, fd: i32) {
        self.config.runtime.set_hpintf_fd(fd);
    }

    /* Sets the process role of the fuzz runner */
    pub fn set_process_role(&mut self, role: NyxProcessRole) {
        let _role = match role {
            NyxProcessRole::Parent => QemuNyxRole::Parent,
            NyxProcessRole::Child => QemuNyxRole::Child,
            NyxProcessRole::StandAlone => QemuNyxRole::StandAlone,
        };

        self.config.runtime.set_process_role(_role);
    }
    
    /* Configures the path to the snapshot file to be reused (optional). */
    pub fn set_reuse_snapshot_path(&mut self, path: String) {
        self.config.runtime.set_reuse_snapshot_path(path);
    }

    /* Returns the currently configured process role of the fuzz runner. */
    pub fn process_role(&self) -> &QemuNyxRole {
        self.config.runtime.process_role()
    }

    /* Returns the current QEMU-Nyx worker ID. */
    pub fn worker_id(&self) -> usize {
        self.config.runtime.worker_id()
    }

    /* Sets the QEMU-Nyx worker ID. */
    pub fn set_worker_id(&mut self, worker_id: usize) {
        self.config.runtime.set_worker_id(worker_id);
    }

    /* Sets the QEMU-Nyx aux buffer size (must be a multiple of 4KB; default value is 4KB). */
    pub fn set_aux_buffer_size(&mut self, size: usize) -> bool{
        return self.config.runtime.set_aux_buffer_size(size);
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

    pub fn new(config: &mut NyxConfig, worker_id: usize) -> Result<NyxProcess, String> {

        let sharedir = config.sharedir_path();
        config.set_worker_id(worker_id);

        match fuzz_runner::nyx::qemu_process_new(sharedir.to_string(), &config.config){
            Ok(x) => Ok(NyxProcess{
                process: x,
            }),
            Err(x) => Err(x),
        }
    }


    pub fn aux_buffer_as_mut_ptr(&self) -> *mut u8 {
        std::ptr::addr_of!(self.process.aux_buffer().header.magic) as *mut u8
    }

    pub fn aux_buffer_size(&self) -> usize {
        self.process.aux_buffer().size()
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
        self.process.aux_buffer_mut().config.reload_mode = if enable {1} else {0};
    }

    pub fn option_set_redqueen_mode(&mut self, enable: bool) {
        self.process.aux_buffer_mut().config.redqueen_mode = if enable {1} else {0};
    }

    pub fn option_set_trace_mode(&mut self, enable: bool) {
        self.process.aux_buffer_mut().config.trace_mode = if enable {1} else {0};
    }

    pub fn option_set_delete_incremental_snapshot(&mut self, enable: bool) {
        self.process.aux_buffer_mut().config.discard_tmp_snapshot = if enable {1} else {0};
    }

    pub fn option_set_timeout(&mut self, timeout_sec: u8, timeout_usec: u32) {
        self.process.aux_buffer_mut().config.timeout_sec = timeout_sec;
        self.process.aux_buffer_mut().config.timeout_usec = timeout_usec;
    }
    
    pub fn option_apply(&mut self) {
        self.process.aux_buffer_mut().config.changed = 1;
    }

    pub fn aux_misc(&self) -> Vec<u8>{
        self.process.aux_buffer().misc_slice().to_vec()
    }

    pub fn aux_data_misc(&self) -> Vec<u8>{
        self.process.aux_buffer().misc_data_slice().to_vec()
    }

    pub fn aux_tmp_snapshot_created(&self) -> bool {
        self.process.aux_buffer().result.tmp_snapshot_created != 0
    }

    pub fn aux_string(&self) -> String {
        let len = self.process.aux_buffer().misc.len;
        String::from_utf8_lossy(&self.process.aux_buffer().misc_data_slice()[0..len as usize]).to_string()
    }
     
    pub fn exec(&mut self) -> NyxReturnValue {
        match self.process.send_payload(){
            Err(_) =>  NyxReturnValue::IoError,
            Ok(_) => {
                match self.process.aux_buffer().result.exec_result_code {
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

    pub fn set_hprintf_fd(&mut self, fd: i32) {

        /* sanitiy check to prevent invalid file descriptors via F_GETFD */
        unsafe { 
            assert!(fcntl(fd, libc::F_GETFD) != -1); 
        };

        self.process.set_hprintf_fd(fd);
    }

}

pub fn remove_work_dir(workdir: &str) -> Result<(), String> {
    fuzz_runner::nyx::qemu_process::remove_workdir_safe(workdir)
}
