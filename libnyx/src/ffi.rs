use libc::c_char;
use std::ffi::CStr;
use std::ffi::c_void;

use fuzz_runner::nyx::aux_buffer::{NYX_CRASH, NYX_HPRINTF, NYX_ABORT};
use super::*;

#[no_mangle]
pub extern "C" fn nyx_load_config(sharedir: *const c_char) -> *mut c_void {
    let sharedir_c_str = unsafe {
        assert!(!sharedir.is_null());
        CStr::from_ptr(sharedir)
    };

    let sharedir_r_str = sharedir_c_str.to_str().unwrap();

    let cfg: NyxConfig = match NyxConfig::load(sharedir_r_str){
        Ok(x) => x,
        Err(msg) => {
            println!("[!] libnyx config reader error: {}", msg);
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(cfg)) as *mut c_void
}

#[no_mangle]
pub extern "C" fn nyx_print_config(config: * mut c_void) {
    unsafe{
        assert!(!config.is_null());
        assert!((config as usize) % std::mem::align_of::<NyxConfig>() == 0);

        let cfg = config as *mut NyxConfig;
        println!("{}", *cfg);
    }
}

fn nyx_process_start(sharedir: *const c_char, workdir: *const c_char, worker_id: u32, cpu_id: u32, create_snapshot: bool, input_buffer_size: Option<u32>, input_buffer_write_protection: bool) -> * mut NyxProcess {
    let sharedir_c_str = unsafe {
        assert!(!sharedir.is_null());
        CStr::from_ptr(sharedir)
    };
    
    let workdir_c_str = unsafe {
        assert!(!workdir.is_null());
        CStr::from_ptr(workdir)
    };

    let sharedir_r_str = sharedir_c_str.to_str().unwrap();
    let workdir_r_str = workdir_c_str.to_str().unwrap();
    
    match NyxProcess::process_start(sharedir_r_str, workdir_r_str, worker_id, cpu_id, create_snapshot, input_buffer_size, input_buffer_write_protection) {
        Ok(x) => Box::into_raw(Box::new(x)),
        Err(msg) => {
            println!("[!] libnyx failed to initialize QEMU-Nyx: {}", msg);
            std::ptr::null_mut() as *mut NyxProcess
        },
    }
}

#[no_mangle]
pub extern "C" fn nyx_new(sharedir: *const c_char, workdir: *const c_char, cpu_id: u32, input_buffer_size: u32, input_buffer_write_protection: bool) -> * mut NyxProcess {
    nyx_process_start(sharedir, workdir, 0, cpu_id, false, Some(input_buffer_size), input_buffer_write_protection)
}


#[no_mangle]
pub extern "C" fn nyx_new_parent(sharedir: *const c_char, workdir: *const c_char, cpu_id: u32, input_buffer_size: u32, input_buffer_write_protection: bool) -> * mut NyxProcess {
    nyx_process_start(sharedir, workdir, 0, cpu_id, true, Some(input_buffer_size), input_buffer_write_protection)
}

#[no_mangle]
pub extern "C" fn nyx_new_child(sharedir: *const c_char, workdir: *const c_char, cpu_id: u32, worker_id: u32) -> * mut NyxProcess {
    if worker_id == 0 {
        println!("[!] libnyx failed -> worker_id=0 cannot be used for child processes");
        std::ptr::null_mut() as *mut NyxProcess
    }
    else{
        nyx_process_start(sharedir, workdir, worker_id, cpu_id, true, None, false)
    }
}
 

#[no_mangle]
pub extern "C" fn nyx_get_aux_buffer(nyx_process: * mut NyxProcess) -> *mut u8 {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        return (*nyx_process).aux_buffer_as_mut_ptr();
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_input_buffer(nyx_process: * mut NyxProcess) -> *mut u8 {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        return (*nyx_process).input_buffer_mut().as_mut_ptr();
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_bitmap_buffer(nyx_process: * mut NyxProcess) -> *mut u8 {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        return (*nyx_process).bitmap_buffer_mut().as_mut_ptr();
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_bitmap_buffer_size(nyx_process: * mut NyxProcess) -> usize {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);
        //return (*nyx_process).process.bitmap.len();
        return (*nyx_process).bitmap_buffer_size();
    }
}

#[no_mangle]
pub extern "C" fn nyx_shutdown(nyx_process: * mut NyxProcess) {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        (*nyx_process).shutdown();
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_set_reload_mode(nyx_process: * mut NyxProcess, enable: bool) {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        (*nyx_process).option_set_reload_mode(enable);
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_set_timeout(nyx_process: * mut NyxProcess, timeout_sec: u8, timeout_usec: u32) {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        (*nyx_process).option_set_timeout(timeout_sec, timeout_usec);
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_apply(nyx_process: * mut NyxProcess) {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        (*nyx_process).option_apply();
    }
}

#[no_mangle]
pub extern "C" fn nyx_exec(nyx_process: * mut NyxProcess) -> NyxReturnValue {
    
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        (*nyx_process).exec()
    }
}

#[no_mangle]
pub extern "C" fn nyx_set_afl_input(nyx_process: * mut NyxProcess, buffer: *mut u8, size: u32) {

    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);
        assert!((buffer as usize) % std::mem::align_of::<u8>() == 0);

        (*nyx_process).set_input_ptr(buffer, size);
   }
}


#[no_mangle]
pub extern "C" fn nyx_print_aux_buffer(nyx_process: * mut NyxProcess) {
    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);

        print!("{}", format!("{:#?}", (*nyx_process).process.aux.result));

        match (*nyx_process).process.aux.result.exec_result_code {
            NYX_CRASH | NYX_ABORT | NYX_HPRINTF => {
                println!("{}", std::str::from_utf8(&(*nyx_process).process.aux.misc.data).unwrap());
            },
            _ => {},
        }
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_aux_string(nyx_process: * mut NyxProcess, buffer: *mut u8, size: u32) -> u32 {

    unsafe{
        assert!(!nyx_process.is_null());
        assert!((nyx_process as usize) % std::mem::align_of::<NyxProcess>() == 0);
        assert!((buffer as usize) % std::mem::align_of::<u8>() == 0);

        let len = std::cmp::min( (*nyx_process).process.aux.misc.len as usize, size as usize);
        std::ptr::copy((*nyx_process).process.aux.misc.data.as_mut_ptr(), buffer, len);
        len as u32
    }
}
