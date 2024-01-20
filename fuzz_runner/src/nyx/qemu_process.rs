use std::os::unix::prelude::FromRawFd;
use std::path::PathBuf;
use nix::sys::mman::*;
use std::fs;
use std::io;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::os::unix::fs::symlink;
use std::os::unix::io::IntoRawFd;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::{thread, time};
use std::process;
use fs4::FileExt;

use nix::unistd::gettid;

use std::str;

extern crate colored; // not needed in Rust 2018

use colored::*;


use crate::nyx::aux_buffer::AuxBuffer;
use crate::nyx::aux_buffer::{NYX_SUCCESS, NYX_CRASH, NYX_HPRINTF, NYX_TIMEOUT, NYX_ABORT, NYX_INPUT_WRITE};

use crate::nyx::ijon_data::{SharedFeedbackData, FeedbackBuffer};
use crate::nyx::mem_barrier::mem_barrier;
use crate::nyx::params::QemuParams;

pub struct QemuProcess {

    process: Child,

    /* ptr to the aux buffer */
    aux: AuxBuffer,

    pub feedback_data: FeedbackBuffer,
    pub ijon_buffer: &'static mut [u8],
    pub ctrl: UnixStream,
    pub bitmap: &'static mut [u8],
    pub bitmap_size: usize,
    pub input_buffer_size: usize,
    pub payload: &'static mut [u8],
    pub params: QemuParams,
    shm_work_dir: PathBuf,
    #[allow(unused)]
    shm_file_lock: File,

    hprintf_file: Option<File>,
}

fn execute_qemu(ctrl: &mut UnixStream) -> io::Result<()>{
    ctrl.write_all(&[120_u8])?;
    Ok(())
}

fn wait_qemu(ctrl: &mut UnixStream) -> io::Result<()>{
    let mut buf = [0];
    ctrl.read_exact(&mut buf)?;
    Ok(())
}

fn run_qemu(ctrl: &mut UnixStream) -> io::Result<()>{
    execute_qemu(ctrl)?;
    wait_qemu(ctrl)?;
    Ok(())
}

fn make_shared_data(file: &File, size: usize) -> &'static mut [u8] {
    let prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
    let flags = MapFlags::MAP_SHARED;
    let null_addr = std::num::NonZeroUsize::new(0);
    let wrapped_size = std::num::NonZeroUsize::new(size).unwrap();
    unsafe {
        let ptr = mmap(null_addr, wrapped_size, prot, flags, file.as_raw_fd(), 0).unwrap();

        let data = std::slice::from_raw_parts_mut(ptr as *mut u8, size);
        return data;
    }
}

fn make_shared_ijon_data(file: File, size: usize) -> FeedbackBuffer {
    let prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
    let flags = MapFlags::MAP_SHARED;
    let null_addr = std::num::NonZeroUsize::new(0);
    let wrapped_size = std::num::NonZeroUsize::new(size).unwrap();
    unsafe {
        let ptr = mmap(null_addr, wrapped_size, prot, flags, file.into_raw_fd(), 0).unwrap();
        FeedbackBuffer::new((ptr as *mut SharedFeedbackData).as_mut().unwrap())
    }
}

impl QemuProcess {

    pub fn new(params: QemuParams) -> Result<QemuProcess, String> {
        Self::prepare_redqueen_workdir(&params.workdir, params.qemu_id);

        if params.qemu_id == 0{
            println!("[!] libnyx: spawning qemu with:\n {}", params.cmd.join(" "));
        }

        let (shm_work_dir, file_lock) = Self::create_shm_work_dir();
        let mut shm_work_dir_path = PathBuf::from(&shm_work_dir);

        shm_work_dir_path.push("bitmap");

        let bitmap_shm_f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&shm_work_dir_path)
            .expect("couldn't open bitmap file");


        if Path::new(&format!("{}/bitmap_{}", params.workdir, params.qemu_id)).exists(){
            fs::remove_file(format!("{}/bitmap_{}", params.workdir, params.qemu_id)).unwrap();
        }

        symlink(
            &shm_work_dir_path,
            format!("{}/bitmap_{}", params.workdir, params.qemu_id),
        )
        .unwrap();

        shm_work_dir_path.pop();
        shm_work_dir_path.push("ijon");

        let ijon_buffer_shm_f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&shm_work_dir_path)
            .expect("couldn't open bitmap file");


        if Path::new(&format!("{}/ijon_{}", params.workdir, params.qemu_id)).exists(){
            fs::remove_file(format!("{}/ijon_{}", params.workdir, params.qemu_id)).unwrap();
        }

        symlink(
            &shm_work_dir_path,
            format!("{}/ijon_{}", params.workdir, params.qemu_id),
        )
        .unwrap();

        shm_work_dir_path.pop();
        shm_work_dir_path.push("input");

        let mut payload_shm_f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&shm_work_dir_path)
            .expect("couldn't open payload file");

        if Path::new(&format!("{}/payload_{}", params.workdir, params.qemu_id)).exists(){
            fs::remove_file(format!("{}/payload_{}", params.workdir, params.qemu_id)).unwrap();
        }

        symlink(
            &shm_work_dir_path,
            format!("{}/payload_{}", params.workdir, params.qemu_id),
        )
        .unwrap();

        payload_shm_f.write_all(b"not_init").unwrap();
        bitmap_shm_f.set_len(params.bitmap_size as u64).unwrap();
        ijon_buffer_shm_f.set_len(0x1000).unwrap();

        let mut bitmap_shared = make_shared_data(&bitmap_shm_f, params.bitmap_size);
        let mut payload_shared = make_shared_data(&payload_shm_f, params.payload_size);

        let ijon_shared = make_shared_data(&ijon_buffer_shm_f, 0x1000);
        let ijon_feedback_buffer = make_shared_ijon_data(ijon_buffer_shm_f, 0x1000);

        let mut child = if params.dump_python_code_for_inputs{
            Command::new(&params.cmd[0])
            .args(&params.cmd[1..])
            .env("DUMP_PAYLOAD_MODE", "TRUE")
            .spawn()
            .expect("failed to execute process")            
        }
        else{
            Command::new(&params.cmd[0])
            .args(&params.cmd[1..])
            .spawn()
            .expect("failed to execute process")
        };

        let mut control = loop {
            match UnixStream::connect(&params.control_filename) {
                Ok(stream) => break stream,
                _ => {
                    thread::sleep(time::Duration::from_millis(1))
                },
            }
        };

        if wait_qemu(&mut control).is_err() {
            return Err(format!("cannot launch QEMU-Nyx..."));
        }

        let aux_buffer = {
            let aux_shm_f = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&params.qemu_aux_buffer_filename)
                .expect("couldn't open aux buffer file");

            AuxBuffer::new(aux_shm_f, params.aux_buffer_size)
        };

        match aux_buffer.validate_header(){
            Err(x) => {
                child.kill().unwrap();
                child.wait().unwrap();
                return Err(x);
            },
            Ok(_) => {},
        }
        if params.write_protected_input_buffer{
            if params.qemu_id == 0 {
                println!("[!] libnyx: input buffer is write protected");
            }
            aux_buffer.config.protect_payload_buffer = 1;
            aux_buffer.config.changed = 1;
        }

        let mut hprintf_file = match params.hprintf_fd {
            Some(fd) =>  Some(unsafe { File::from_raw_fd(fd) }),
            None => None,
        }; 

        loop {

            match aux_buffer.result.exec_result_code {
                NYX_HPRINTF     => {
                    let len = aux_buffer.misc.len;
                    QemuProcess::output_hprintf(&mut hprintf_file, &String::from_utf8_lossy(&aux_buffer.misc.data[0..len as usize]).yellow());
                },
                NYX_ABORT => {
                    let len = aux_buffer.misc.len;
                    let msg = format!("agent abort() -> \n\t{}", String::from_utf8_lossy(&aux_buffer.misc.data[0..len as usize]).red());

                    /* get rid of this process */
                    child.kill().unwrap();
                    child.wait().unwrap();

                    return Err(msg);
                }
                NYX_SUCCESS => {},
                x => {
                    panic!(" -> unkown type ? {}", x);
                }
            }

            if aux_buffer.result.state == 3 {
                break;
            }
            if run_qemu(&mut control).is_err(){
                return Err(format!("failed to establish fuzzing loop..."));
            } 
        }

        let mut bitmap_size = params.bitmap_size as usize;
        //println!("[!] libnyx: {:x}", aux_buffer.cap.agent_coverage_bitmap_size);
        if aux_buffer.cap.agent_coverage_bitmap_size != 0 {
            //let file_len = bitmap_shm_f.metadata().unwrap().len();
            bitmap_size = aux_buffer.cap.agent_coverage_bitmap_size as usize;
            if aux_buffer.cap.agent_coverage_bitmap_size as usize > bitmap_shared.len(){
                //println!("[!] libnyx: agent requests a differnt coverage bitmap size: {:x} (current: {:x})", aux_buffer.cap.agent_coverage_bitmap_size as u32, file_len);
                bitmap_shared = make_shared_data(&bitmap_shm_f, aux_buffer.cap.agent_coverage_bitmap_size as usize);
            }
        }

        let mut input_buffer_size = params.payload_size as usize;
        if aux_buffer.cap.agent_input_buffer_size != 0 {
            input_buffer_size = aux_buffer.cap.agent_input_buffer_size as usize;
            if aux_buffer.cap.agent_input_buffer_size as usize > payload_shared.len(){
                payload_shared = make_shared_data(&payload_shm_f, aux_buffer.cap.agent_input_buffer_size as usize);
            }
        }

        match aux_buffer.cap.agent_trace_bitmap {
            0 => println!("[!] libnyx: coverage mode: Intel-PT (KVM-Nyx and libxdc)"),
            1 => println!("[!] libnyx: coverage mode: compile-time instrumentation"),
            _ => panic!("unkown aux_buffer.cap.agent_trace_bitmap value"),
        };

        println!("[!] libnyx: qemu #{} is ready:", params.qemu_id);

        aux_buffer.config.reload_mode = 1;
        aux_buffer.config.timeout_sec = params.time_limit.as_secs() as u8;
        aux_buffer.config.timeout_usec = params.time_limit.subsec_micros();
        aux_buffer.config.changed = 1;

        return Ok(QemuProcess {
            process: child,
            aux: aux_buffer,
            feedback_data: ijon_feedback_buffer,
            ijon_buffer: ijon_shared,
            ctrl: control,
            bitmap: bitmap_shared,
            bitmap_size: bitmap_size,
            input_buffer_size: input_buffer_size,
            payload: payload_shared,
            params,
            shm_work_dir,
            shm_file_lock: file_lock,
            hprintf_file,
        });
    }

    fn output_hprintf(hprintf_file: &mut Option<File>, msg: &str){
        match hprintf_file {
            Some(ref mut f) => {
                f.write_fmt(format_args!("{}", msg)).unwrap();
            },
            None => {
                print!("{}", msg);
            }
        }
    }

    pub fn aux_buffer(&self) -> &AuxBuffer{
        &self.aux
    }

    pub fn aux_buffer_mut(&mut self) -> &mut AuxBuffer{
        &mut self.aux
    }

    pub fn set_hprintf_fd(&mut self, fd: i32){
        self.hprintf_file = unsafe { Some(File::from_raw_fd(fd)) };
    }

    pub fn send_payload(&mut self) -> io::Result<()>{
        let mut old_address: u64 = 0;

        loop {
            mem_barrier();
            match run_qemu(&mut self.ctrl) {
                Err(x) => return Err(x),
                Ok(_) => {},
            }
            mem_barrier();

            if self.aux.result.page_not_found != 0 {
                let v = self.aux.result.page_not_found_addr;
                if old_address != self.aux.result.page_not_found_addr {
                    //println!("libnyx: page is missing -> {:x}\n", v);
                    old_address = self.aux.result.page_not_found_addr;
                    self.aux.config.page_addr = self.aux.result.page_not_found_addr;
                    self.aux.config.page_dump_mode = 1;
                    self.aux.config.changed = 1;

                    mem_barrier();
                    match run_qemu(&mut self.ctrl) {
                        Err(x) => return Err(x),
                        Ok(_) => {},
                    }
                    mem_barrier();

                    continue;
                }
                else{
                    println!("libnyx: cannot dump missing page -> {:x}", v);
                }
            }
            
            match self.aux.result.exec_result_code {
                NYX_HPRINTF     => {
                    let len = self.aux.misc.len;
                    QemuProcess::output_hprintf(&mut self.hprintf_file, &String::from_utf8_lossy(&self.aux.misc_data_slice()[0..len as usize]).yellow());
                    continue;
                },
                NYX_ABORT       => {
                    let len = self.aux.misc.len;
                    println!("[!] libnyx: agent abort() -> \"{}\"", String::from_utf8_lossy(&self.aux.misc_data_slice()[0..len as usize]).red());
                    break;
                },
                NYX_SUCCESS | NYX_CRASH | NYX_INPUT_WRITE | NYX_TIMEOUT      => {
                    break;
                },
                x => {
                    panic!("[!] libnyx: ERROR -> unkown Nyx exec result code: {}", x);
                }
            }
        }
        Ok(())
    }

    pub fn set_timeout(&mut self, timeout: std::time::Duration){
        self.aux.config.timeout_sec = timeout.as_secs() as u8;
        self.aux.config.timeout_usec = timeout.subsec_micros();
        self.aux.config.changed = 1;
    }

    pub fn wait(&mut self) {
        self.process.wait().unwrap();
    }

    fn remove_shm_work_dir(&mut self){

        /* move originals into workdir (in case we need the data to debug stuff) */
        let shm_path = self.shm_work_dir.to_str().unwrap();
        fs::remove_file(&format!("{}/bitmap_{}", &self.params.workdir, self.params.qemu_id)).unwrap();
        fs::copy(&format!("{}/bitmap", shm_path), &format!("{}/bitmap_{}", &self.params.workdir, self.params.qemu_id)).unwrap();

        fs::remove_file(&format!("{}/payload_{}", &self.params.workdir, self.params.qemu_id)).unwrap();
        fs::copy(&format!("{}/input", shm_path), &format!("{}/payload_{}", &self.params.workdir, self.params.qemu_id)).unwrap();

        fs::remove_file(&format!("{}/ijon_{}", &self.params.workdir, self.params.qemu_id)).unwrap();
        fs::copy(&format!("{}/ijon", shm_path), &format!("{}/ijon_{}", &self.params.workdir, self.params.qemu_id)).unwrap();

        /* remove this shm directory */
        fs::remove_dir_all(&self.shm_work_dir).unwrap();
    }

    pub fn shutdown(&mut self) {
        println!("[!] libnyx: sending SIGKILL to QEMU-Nyx process...");
        self.process.kill().unwrap();
        self.wait();
        self.remove_shm_work_dir();
    }

    pub fn wait_for_workdir(workdir: &str){
        println!("[!] libnyx: waiting for workdir to be created by parent process...");
        
        let files = vec![
            "page_cache.lock",
            "page_cache.addr",
            "page_cache.addr",
            "snapshot/fast_snapshot.qemu_state"
        ];
        for file in files.iter() {
            while !Path::new(&format!("{}/{}", workdir, file)).exists(){
                thread::sleep(time::Duration::from_secs(1));
            }
        }
    }

    pub fn prepare_workdir(workdir: &str, seed_path: Option<String>) {
        Self::clear_workdir(workdir);
        let folders = vec![
            "/corpus/normal",
            "/corpus/crash",
            "/corpus/kasan",
            "/corpus/timeout",
            "/imports",
            "/seeds",
            "/snapshot",
            "/forced_imports",
        ];

        for folder in folders.iter() {
            fs::create_dir_all(format!("{}/{}", workdir, folder))
                .expect("couldn't initialize workdir");
        }
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{}/filter", workdir))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{}/page_cache.lock", workdir))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{}/page_cache.dump", workdir))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{}/page_cache.addr", workdir))
            .unwrap();

        OpenOptions::new().create(true).write(true).open(format!("{}/program", workdir)).unwrap();

        //println!("IMPORT STUFF FOR {:?}", seed_path);
        if let Some(path) = seed_path {
            let pattern = format!("{}/*", path);
            //println!("IMPORT STUFF FOR {}", pattern);
            for (i,p) in glob::glob(&pattern).expect("couldn't glob seed pattern??").enumerate()
            {
                let src = p.unwrap_or_else(|e| panic!("invalid seed path found {:?}",e));
                //println!("import {} to {}/seeds/seed_{}",src.to_string_lossy(), workdir,i);
                let dst = format!("{}/seeds/seed_{}.bin",workdir, i);
                fs::copy(&src, &dst).unwrap_or_else(|e| panic!("couldn't copy seed {} to {} {:?}",src.to_string_lossy(),dst,e));
            }
        }
    }

    fn prepare_redqueen_workdir(workdir: &str, qemu_id: usize) {
        fs::create_dir_all(format!("{}/redqueen_workdir_{}", workdir, qemu_id))
            .expect("couldn't initialize workdir");   
    }

    fn remove_unused_shm_work_dirs(){
        /* find and remove orphaned Nyx shm workdirs in /dev/shm */
        for p in glob::glob(&format!("/dev/shm/nyx_*")).expect("couldn't glob??"){
            let mut path = p.unwrap();
            
            path.push("lock");
            if path.exists(){

                let file_lock = match OpenOptions::new()
                    .read(true)
                    .open(&path){
                        Err(x) => {
                            println!("Warning: {}", x);
                            Err(x)
                        },
                        x => {
                            x
                        },
                    };

                if file_lock.is_ok(){
                    path.pop();

                    match file_lock.unwrap().try_lock_exclusive(){
                        Ok(_) => {
                            if path.starts_with("/dev/shm/") {
                                match fs::remove_dir_all(path){
                                    Err(x) => {
                                        println!("Warning: {}", x);
                                    },
                                    _ => {},
                                }
                            }
                        },
                        Err(_) => {},
                    }
                }
            }
        }
    }

    fn clear_workdir(workdir: &str) {
        let _ = fs::remove_dir_all(workdir);
        Self::remove_unused_shm_work_dirs()
    }

    fn create_shm_work_dir() -> (PathBuf, File) {
        let shm_work_dir_path_str = format!("/dev/shm/nyx_{}_{}/", process::id(), gettid());
        let shm_work_dir_path = PathBuf::from(&shm_work_dir_path_str);

        fs::create_dir_all(&shm_work_dir_path).expect("couldn't initialize shm work directory");

        let file_lock_path_str = format!("/dev/shm/nyx_{}_{}/lock", process::id(), gettid());
        let file_lock_path = Path::new(&file_lock_path_str);

        let file_lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(file_lock_path)
            .expect("couldn't open shm work dir lock file");

        file_lock.lock_exclusive().unwrap();

        (shm_work_dir_path, file_lock)
    }
}

/* Helper function to remove a Nyx workdir safely. Returns an error if 
 * expected sub dirs are missing or the path does not exist */
pub fn remove_workdir_safe(workdir: &str) -> Result<(), String> {
    let folders = vec![
        "/corpus/normal",
        "/corpus/crash",
        "/corpus/kasan",
        "/corpus/timeout",
        "/imports",
        "/seeds",
        "/snapshot",
        "/forced_imports",
    ];

    if !Path::new(&format!("{}/", workdir)).exists() {
        return Err(format!("\"{}/\" does not exists", workdir));
    }

    /* check if all sub dirs exists */
    for folder in folders.iter() {
        if !Path::new(&format!("{}/{}", workdir, folder)).exists() {
            return Err(format!("\"{}/{}\" does not exists", workdir, folder));
        }
    }

    /* remove if all sub dirs exists */
    for folder in folders.iter() {
        let _ = fs::remove_dir_all(format!("{}/{}", workdir, folder));
    }

    let _ = fs::remove_dir_all(workdir);

    return Ok(());
}
