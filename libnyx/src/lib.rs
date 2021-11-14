extern crate libc;

use config::{Config, FuzzRunnerConfig};

use fuzz_runner::nyx::qemu_process_new_from_kernel;
use fuzz_runner::nyx::qemu_process_new_from_snapshot;
use fuzz_runner::nyx::qemu_process::QemuProcess;

use libc::c_char;
use std::ffi::CStr;

#[repr(C)]
pub enum NyxReturnValue {
    Normal,
    Crash,
    Asan,
    Timout,
    InvalidWriteToPayload,
    Error
}

#[no_mangle]
pub extern "C" fn nyx_new(sharedir: *const c_char, workdir: *const c_char, worker_id: u32, create_snapshot: bool) -> * mut QemuProcess {
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

    println!("r_str: {}", sharedir_r_str);
    let cfg: Config = Config::new_from_sharedir(&sharedir_r_str);
    println!("config {}", cfg.fuzz.bitmap_size);



    let mut config = cfg.fuzz;
    let runner_cfg = cfg.runner;


    /* todo: add sanity check */
    config.cpu_pin_start_at = worker_id as usize;

    config.thread_id = worker_id as usize;
    config.threads = if create_snapshot { 2 as usize } else { 1 as usize };

    
    config.workdir_path = format!("{}", workdir_r_str);

    let sdir = sharedir_r_str.clone();

    if worker_id == 0 {
        QemuProcess::prepare_workdir(&config.workdir_path, config.seed_path.clone());
    }

    match runner_cfg.clone() {
        FuzzRunnerConfig::QemuSnapshot(cfg) => {
            let runner = qemu_process_new_from_snapshot(sdir.to_string(), &cfg, &config);
            return Box::into_raw(Box::new(runner));
        }
        FuzzRunnerConfig::QemuKernel(cfg) => {
            let runner = qemu_process_new_from_kernel(sdir.to_string(), &cfg, &config);
            return Box::into_raw(Box::new(runner));
        }
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_aux_buffer(qemu_process: * mut QemuProcess) -> *mut u8 {
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        //return (*qemu_process).aux.get_raw_ptr();
        //return &((*qemu_process).aux.header).as_mut_ptr();
        return std::ptr::addr_of!((*qemu_process).aux.header.magic) as *mut u8;
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_payload_buffer(qemu_process: * mut QemuProcess) -> *mut u8 {
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        return (*qemu_process).payload.as_mut_ptr();
    }
}

#[no_mangle]
pub extern "C" fn nyx_get_bitmap_buffer(qemu_process: * mut QemuProcess) -> *mut u8 {
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        return (*qemu_process).bitmap.as_mut_ptr();
    }
}

#[no_mangle]
pub extern "C" fn nyx_shutdown(qemu_process: * mut QemuProcess) {
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        (*qemu_process).shutdown();
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_set_reload_mode(qemu_process: * mut QemuProcess, enable: bool) {
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        (*qemu_process).aux.config.reload_mode = if enable {1} else {0};
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_set_timeout(qemu_process: * mut QemuProcess, timeout_sec: u8, timeout_usec: u32) {
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        (*qemu_process).aux.config.timeout_sec = timeout_sec;
        (*qemu_process).aux.config.timeout_usec = timeout_usec;
    }
}

#[no_mangle]
pub extern "C" fn nyx_option_apply(qemu_process: * mut QemuProcess) {
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        (*qemu_process).aux.config.changed = 1;
    }
}

#[no_mangle]
pub extern "C" fn nyx_exec(qemu_process: * mut QemuProcess) -> NyxReturnValue {
    
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        (*qemu_process).send_payload();

        if (*qemu_process).aux.result.crash_found != 0 {
            return NyxReturnValue::Crash;
        }
        if (*qemu_process).aux.result.asan_found != 0 {
            return NyxReturnValue::Asan;
        }
        if (*qemu_process).aux.result.timeout_found != 0 {
            return NyxReturnValue::Timout;
        }
        if (*qemu_process).aux.result.payload_write_attempt_found != 0 {
            return NyxReturnValue::InvalidWriteToPayload;
        }
        if (*qemu_process).aux.result.success != 0 {
            return NyxReturnValue::Normal;
        }
        println!("unknown exeuction result!!");
        return NyxReturnValue::Error;
    }
}

#[no_mangle]
pub extern "C" fn nyx_set_afl_input(qemu_process: * mut QemuProcess, buffer: *mut u8, size: u32) {

    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);
        assert!((buffer as usize) % std::mem::align_of::<u8>() == 0);

        std::ptr::copy(&size, (*qemu_process).payload.as_mut_ptr() as *mut u32, 1 as usize);
        std::ptr::copy(buffer, (*qemu_process).payload[std::mem::size_of::<u32>()..].as_mut_ptr(), std::cmp::min(size as usize, 0x10000));
    }
}


#[no_mangle]
pub extern "C" fn nyx_print_aux_buffer(qemu_process: * mut QemuProcess) {
    unsafe{
        assert!(!qemu_process.is_null());
        assert!((qemu_process as usize) % std::mem::align_of::<QemuProcess>() == 0);

        print!("{}", format!("{:#?}", (*qemu_process).aux.result));
        if (*qemu_process).aux.result.crash_found != 0 || (*qemu_process).aux.result.asan_found != 0 || (*qemu_process).aux.result.hprintf != 0 { 
            println!("{}", std::str::from_utf8(&(*qemu_process).aux.misc.data).unwrap());
        }
    }
}




#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
