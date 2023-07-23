/* 
    libnyx FFI API

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
use libc::c_char;
use std::ffi::CStr;
use std::ffi::c_void;

use fuzz_runner::nyx::aux_buffer::{NYX_CRASH, NYX_HPRINTF, NYX_ABORT};
use super::*;

/* Helper function to load a C string pointer and return a Rust string. */
fn __load_c_string_ptr(pointer: *const c_char) -> String {
    let c_str = unsafe {
        assert!(!pointer.is_null());
        CStr::from_ptr(pointer)
    };

    c_str.to_str().unwrap().to_string()
}

/* Helper function to check if the config pointer is valid.
 * Turns the pointer into a reference to a config object.
 */
fn __nyx_config_check_ptr(config: * mut c_void) -> *mut NyxConfig {
    assert!(!config.is_null());
    assert!((config as usize) % std::mem::align_of::<NyxConfig>() == 0);

    config as *mut NyxConfig
}

/* Loads a given Nyx share-dir and returns a raw pointer to the Nyx config object.
 * The pointer is later used to access the config object in other FFI config functions. 
 */
#[no_mangle]
pub extern "C" fn nyx_config_load(sharedir: *const c_char) -> *mut c_void {
    let sharedir_r_str = __load_c_string_ptr(sharedir);

    let cfg: NyxConfig = match NyxConfig::load(&sharedir_r_str){
        Ok(x) => x,
        Err(msg) => {
            println!("[!] libnyx config reader error: {}", msg);
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(cfg)) as *mut c_void
}

/* Simple debug function to print the entire config object to stdout. */
#[no_mangle]
pub extern "C" fn nyx_config_debug(config: * mut c_void) {
    let cfg = __nyx_config_check_ptr(config);

    unsafe{
        println!("{}", *cfg);
    }
}

/* Simple debug function to print a subset of configurable options to stdout. */
#[no_mangle]
pub extern "C" fn nyx_config_print(config: * mut c_void) {
    unsafe{
        NyxConfig::print(&*__nyx_config_check_ptr(config));
    }
}

/* FFI function to set the workdir path in the config object. */
#[no_mangle]
pub extern "C" fn nyx_config_set_workdir_path(config: * mut c_void, workdir: *const c_char) {
    let workidr_r_str = __load_c_string_ptr(workdir);
    let cfg = __nyx_config_check_ptr(config);

    unsafe{
        NyxConfig::set_workdir_path(&mut *cfg, workidr_r_str.to_string());
    }
}

/* FFI function to set the input buffer size in the config object. */
#[no_mangle]
pub extern "C" fn nyx_config_set_input_buffer_size(config: * mut c_void, input_buffer_size: u32) {
    let cfg = __nyx_config_check_ptr(config);

    assert_eq!(input_buffer_size > 0, true);
    unsafe{
        NyxConfig::set_input_buffer_size(&mut *cfg, input_buffer_size as usize);
    }
}

/* FFI function to set the input buffer write protection in the config object. */
#[no_mangle]
pub extern "C" fn nyx_config_set_input_buffer_write_protection(config: * mut c_void, input_buffer_write_protection: bool) {
    let cfg = __nyx_config_check_ptr(config);

    unsafe{
        NyxConfig::set_input_buffer_write_protection(&mut *cfg, input_buffer_write_protection);
    }
}

/* FFI function to set the hprintf file descriptor in the config object. */
#[no_mangle]
pub extern "C" fn nyx_config_set_hprintf_fd(config: * mut c_void, hprintf_fd: i32) {
    let cfg = __nyx_config_check_ptr(config);

    unsafe{
        NyxConfig::set_hprintf_fd(&mut *cfg, hprintf_fd);
    }
}

/* FFI function to set the fuzz runner role in the config object. */
#[no_mangle]
pub extern "C" fn nyx_config_set_process_role(config: * mut c_void, role: NyxProcessRole) {
    let cfg = __nyx_config_check_ptr(config);

    unsafe{
        NyxConfig::set_process_role(&mut *cfg, role);
    }
}

/* Enable snapshot reuse by setting the path to the snapshot folder. */
#[no_mangle]
pub extern "C" fn nyx_config_set_reuse_snapshot_path(config: * mut c_void, reuse_snapshot_path: *const c_char) {
    let reuse_snapshot_path_r_str = __load_c_string_ptr(reuse_snapshot_path);
    let cfg = __nyx_config_check_ptr(config);

    unsafe{
        NyxConfig::set_reuse_snapshot_path(&mut *cfg, reuse_snapshot_path_r_str.to_string());
    }
}

/* FFI function to set the aux_buffer size */
#[no_mangle]
pub extern "C" fn nyx_config_set_aux_buffer_size(config: * mut c_void, aux_buffer_size: u32) -> bool {
    let cfg = __nyx_config_check_ptr(config);

    assert_eq!(aux_buffer_size > 0, true);
    unsafe{
        return NyxConfig::set_aux_buffer_size(&mut *cfg, aux_buffer_size as usize);
    }
}

#[no_mangle]
pub extern "C" fn nyx_new(config: * mut c_void, worker_id: u32) -> * mut NyxProcess {
    
    let cfg = __nyx_config_check_ptr(config);

    match NyxProcess::new(unsafe {&mut *(cfg)}, worker_id as usize) {
        Ok(x) => Box::into_raw(Box::new(x)),
        Err(msg) => {
            println!("[!] libnyx failed to initialize QEMU-Nyx: {}", msg);
            std::ptr::null_mut() as *mut NyxProcess
        },
    }
}

/* Helper function to check if the NyxProcess pointer is valid.
 * Turns the pointer into a reference to the NyxProcess object.
 */
fn __nyx_process_check_ptr(nyx_process: * mut NyxProcess) -> *mut NyxProcess {
    assert!(!nyx_process.is_null());
    assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);
    nyx_process as *mut NyxProcess
} 

#[no_mangle]
pub extern "C" fn nyx_get_aux_buffer(nyx_process: * mut NyxProcess) -> *mut u8 {
    unsafe{
        return (*__nyx_process_check_ptr(nyx_process)).aux_buffer_as_mut_ptr();
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_input_buffer(nyx_process: * mut NyxProcess) -> *mut u8 {
    unsafe{
        return (*__nyx_process_check_ptr(nyx_process)).input_buffer_mut().as_mut_ptr();
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_bitmap_buffer(nyx_process: * mut NyxProcess) -> *mut u8 {
    unsafe{
        return (*__nyx_process_check_ptr(nyx_process)).bitmap_buffer_mut().as_mut_ptr();
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_bitmap_buffer_size(nyx_process: * mut NyxProcess) -> usize {
    unsafe{
        return (*__nyx_process_check_ptr(nyx_process)).bitmap_buffer_size();
    }
}

#[no_mangle]
pub extern "C" fn nyx_shutdown(nyx_process: * mut NyxProcess) {
    unsafe{
        (*__nyx_process_check_ptr(nyx_process)).shutdown();
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_set_reload_mode(nyx_process: * mut NyxProcess, enable: bool) {
    unsafe{
        (*__nyx_process_check_ptr(nyx_process)).option_set_reload_mode(enable);
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_set_timeout(nyx_process: * mut NyxProcess, timeout_sec: u8, timeout_usec: u32) {
    unsafe{
        (*__nyx_process_check_ptr(nyx_process)).option_set_timeout(timeout_sec, timeout_usec);
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_apply(nyx_process: * mut NyxProcess) {
    unsafe{
        (*__nyx_process_check_ptr(nyx_process)).option_apply();
    }
}

#[no_mangle]
pub extern "C" fn nyx_exec(nyx_process: * mut NyxProcess) -> NyxReturnValue {
    
    unsafe{
        (*__nyx_process_check_ptr(nyx_process)).exec()
    }
}

#[no_mangle]
pub extern "C" fn nyx_set_afl_input(nyx_process: * mut NyxProcess, buffer: *mut u8, size: u32) {

    unsafe{
        assert!((buffer as usize) % std::mem::align_of::<u8>() == 0);
        (*__nyx_process_check_ptr(nyx_process)).set_input_ptr(buffer, size);
   }
}


#[no_mangle]
pub extern "C" fn nyx_print_aux_buffer(nyx_process: * mut NyxProcess) {
    unsafe{

        let nyx_process = __nyx_process_check_ptr(nyx_process);

        print!("{}", format!("{:#?}", (*nyx_process).process.aux_buffer().result));

        match (*nyx_process).process.aux_buffer().result.exec_result_code {
            NYX_CRASH | NYX_ABORT | NYX_HPRINTF => {
                println!("{}", std::str::from_utf8(&(*nyx_process).process.aux_buffer().misc.data).unwrap());
            },
            _ => {},
        }
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_aux_string(nyx_process: * mut NyxProcess, buffer: *mut u8, size: u32) -> u32 {

    unsafe{
        let nyx_process = __nyx_process_check_ptr(nyx_process);
        assert!((buffer as usize) % std::mem::align_of::<u8>() == 0);

        let len = std::cmp::min( (*nyx_process).process.aux_buffer().misc.len as usize, size as usize);
        std::ptr::copy((*nyx_process).process.aux_buffer_mut().misc.data.as_mut_ptr(), buffer, len);
        len as u32
    }
}


#[no_mangle]
pub extern "C" fn nyx_set_hprintf_fd(nyx_process: * mut NyxProcess, fd: i32) {
    unsafe{
        (*__nyx_process_check_ptr(nyx_process)).process.set_hprintf_fd(fd);
    }
}

/* Helper function to remove a given Nyx workdir safely.
 * This function will return an error if the path does not exist or it does 
 * not appear to be a Nyx workdir (e.g. specific sub directories are 
 * missing). */
#[no_mangle]
pub extern "C" fn nyx_remove_work_dir(workdir: *const c_char) -> bool {
    unsafe{
        let workdir = CStr::from_ptr(workdir).to_str().unwrap();

        match remove_work_dir(workdir) {
            Ok(_) => {
                true
            },
            Err(e) => {
                eprintln!("[!] libnyx failed to remove workdir: {}", e);
                false
            }
        }
    }
}