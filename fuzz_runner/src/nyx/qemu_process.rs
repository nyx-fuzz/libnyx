use std::path::PathBuf;
use nix::sys::mman::*;
use std::fs;
use std::io;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::num::NonZeroUsize;
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
use crate::nyx::aux_buffer::{
    NYX_SUCCESS, NYX_CRASH, NYX_HPRINTF, NYX_TIMEOUT, NYX_ABORT, NYX_INPUT_WRITE,
    NYX_SANITIZER, NYX_STARVED
};

use crate::nyx::ijon_data::{SharedFeedbackData, FeedbackBuffer};
use crate::nyx::mem_barrier::mem_barrier;
use crate::nyx::params::QemuParams;

pub struct QemuProcess {
    pub process: Child,
    pub aux: AuxBuffer,
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
    unsafe {
        let ptr = mmap(
            None,
            NonZeroUsize::new_unchecked(size),
            prot,
            flags,
            file.as_raw_fd(),
            0,
        )
        .unwrap();

        let data = std::slice::from_raw_parts_mut(ptr as *mut u8, size);
        data
    }
}

fn make_shared_ijon_data(file: File, size: usize) -> FeedbackBuffer {
    let prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
    let flags = MapFlags::MAP_SHARED;
    unsafe {
        let ptr = mmap(
            None,
            NonZeroUsize::new_unchecked(size),
            prot,
            flags,
            file.into_raw_fd(),
            0,
        )
        .unwrap();
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


        thread::sleep(time::Duration::from_secs(1));

        thread::sleep(time::Duration::from_millis(200*params.qemu_id as u64));


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


        thread::sleep(time::Duration::from_secs(1));

        thread::sleep(time::Duration::from_millis(200*params.qemu_id as u64));


        let mut control = loop {
            match UnixStream::connect(&params.control_filename) {
                Ok(stream) => break stream,
                _ => {
                    thread::sleep(time::Duration::from_millis(1))
                },
            }
        };

        if wait_qemu(&mut control).is_err() {
            return Err("cannot launch QEMU-Nyx...".to_string());
        }

        let aux_shm_f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&params.qemu_aux_buffer_filename)
            .expect("couldn't open aux buffer file");
        aux_shm_f.set_len(0x1000).unwrap();

        let aux_shm_f = OpenOptions::new()
            .write(true)
            .read(true)
            .open(&params.qemu_aux_buffer_filename)
            .expect("couldn't open aux buffer file");
        let mut aux_buffer = AuxBuffer::new(aux_shm_f);

        if let Err(x) = aux_buffer.validate_header() {
            child.kill().unwrap();
            child.wait().unwrap();
            return Err(x);
        }
        if params.write_protected_input_buffer{
            if params.qemu_id == 0 {
                println!("[!] libnyx: input buffer is write protected");
            }
            aux_buffer.config.protect_payload_buffer = 1;
            aux_buffer.config.changed = 1;
        }

        loop {

            match aux_buffer.result.exec_result_code {
                NYX_HPRINTF     => {
                    let len = aux_buffer.misc.len;
                    print!("{}", String::from_utf8_lossy(&aux_buffer.misc.data[0..len as usize]).yellow());
                },
                NYX_ABORT => {
                    let len = aux_buffer.misc.len;
                    let msg = format!("agent abort() -> \n\t{}", String::from_utf8_lossy(&aux_buffer.misc.data[0..len as usize]).red());

                    /* get rid of this process */
                    child.kill().unwrap();
                    child.wait().unwrap();

                    return Err(msg);
                }
                NYX_SUCCESS | NYX_STARVED => {},
                x => {
                    panic!(" -> unkown type ? {}", x);
                }
            }

            if aux_buffer.result.state == 3 {
                break;
            }
            if run_qemu(&mut control).is_err(){
                return Err("failed to establish fuzzing loop...".to_string());
            }
        }

        let mut bitmap_size = params.bitmap_size;
        //println!("[!] libnyx: {:x}", aux_buffer.cap.agent_coverage_bitmap_size);
        if aux_buffer.cap.agent_coverage_bitmap_size != 0 {
            //let file_len = bitmap_shm_f.metadata().unwrap().len();
            bitmap_size = aux_buffer.cap.agent_coverage_bitmap_size as usize;
            if aux_buffer.cap.agent_coverage_bitmap_size as usize > bitmap_shared.len(){
                //println!("[!] libnyx: agent requests a differnt coverage bitmap size: {:x} (current: {:x})", aux_buffer.cap.agent_coverage_bitmap_size as u32, file_len);
                bitmap_shared = make_shared_data(&bitmap_shm_f, aux_buffer.cap.agent_coverage_bitmap_size as usize);
            }
        }

        let mut input_buffer_size = params.payload_size;
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
        aux_buffer.config.timeout_sec = 0;
        aux_buffer.config.timeout_usec = 500_000;
        aux_buffer.config.changed = 1;

        Ok(QemuProcess {
            process: child,
            aux: aux_buffer,
            feedback_data: ijon_feedback_buffer,
            ijon_buffer: ijon_shared,
            ctrl: control,
            bitmap: bitmap_shared,
            bitmap_size,
            input_buffer_size,
            payload: payload_shared,
            params,
            shm_work_dir,
            shm_file_lock: file_lock,
        })
    }


    pub fn send_payload(&mut self) -> io::Result<()>{
        let mut old_address: u64 = 0;

        loop {
            mem_barrier();
            run_qemu(&mut self.ctrl)?;
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
                    run_qemu(&mut self.ctrl)?;
                    mem_barrier();

                    continue;
                }
                else{
                    println!("libnyx: cannot dump missing page -> {v:x}");
                }
            }


            match self.aux.result.exec_result_code {
                NYX_HPRINTF     => {
                    let len = self.aux.misc.len;
                    print!("{}", String::from_utf8_lossy(&self.aux.misc.data[0..len as usize]).yellow());
                    continue;
                },
                NYX_ABORT       => {
                    let len = self.aux.misc.len;
                    println!("[!] libnyx: agent abort() -> \"{}\"", String::from_utf8_lossy(&self.aux.misc.data[0..len as usize]).red());
                    break;
                },
                NYX_SUCCESS | NYX_CRASH | NYX_INPUT_WRITE | NYX_TIMEOUT | NYX_SANITIZER | NYX_STARVED => {
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
        fs::remove_file(format!(
            "{}/bitmap_{}",
            &self.params.workdir, self.params.qemu_id
        ))
        .unwrap();
        fs::copy(
            format!("{shm_path}/bitmap"),
            format!("{}/bitmap_{}", &self.params.workdir, self.params.qemu_id),
        )
        .unwrap();

        fs::remove_file(format!(
            "{}/payload_{}",
            &self.params.workdir, self.params.qemu_id
        ))
        .unwrap();
        fs::copy(
            format!("{shm_path}/input"),
            format!("{}/payload_{}", &self.params.workdir, self.params.qemu_id),
        )
        .unwrap();

        fs::remove_file(format!(
            "{}/ijon_{}",
            &self.params.workdir, self.params.qemu_id
        ))
        .unwrap();
        fs::copy(
            format!("{shm_path}/ijon"),
            format!("{}/ijon_{}", &self.params.workdir, self.params.qemu_id),
        )
        .unwrap();

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
            while !Path::new(&format!("{workdir}/{file}")).exists(){
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
            fs::create_dir_all(format!("{workdir}/{folder}"))
                .expect("couldn't initialize workdir");
        }
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{workdir}/filter"))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{workdir}/page_cache.lock"))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{workdir}/page_cache.dump"))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{workdir}/page_cache.addr"))
            .unwrap();

            OpenOptions::new().create(true).write(true).open(format!("{workdir}/program")).unwrap();

            //println!("IMPORT STUFF FOR {:?}", seed_path);
            if let Some(path) = seed_path {
                let pattern = format!("{path}/*");
                //println!("IMPORT STUFF FOR {}", pattern);
                for (i,p) in glob::glob(&pattern).expect("couldn't glob seed pattern??").enumerate()
                {
                    let src = p.unwrap_or_else(|e| panic!("invalid seed path found {:?}",e));
                    //println!("import {} to {}/seeds/seed_{}",src.to_string_lossy(), workdir,i);
                    let dst = format!("{workdir}/seeds/seed_{i}.bin");
                    fs::copy(&src, &dst).unwrap_or_else(|e| panic!("couldn't copy seed {} to {} {:?}",src.to_string_lossy(),dst,e));
                }
            }
        }

    fn prepare_redqueen_workdir(workdir: &str, qemu_id: usize) {
        fs::create_dir_all(format!("{workdir}/redqueen_workdir_{qemu_id}"))
            .expect("couldn't initialize workdir");
    }

    fn remove_unused_shm_work_dirs(){
        /* find and remove orphaned Nyx shm workdirs in /dev/shm */
        for p in glob::glob("/dev/shm/nyx_*").expect("couldn't glob??"){
            let mut path = p.unwrap();

            path.push("lock");
            if path.exists(){

                let file_lock = match OpenOptions::new()
                    .read(true)
                    .open(&path){
                        Err(x) => {
                            println!("Warning: {x}");
                            Err(x)
                    },
                    x => {
                        x
                        },
                    };

                if let Ok(file) = file_lock {
                    path.pop();

                    if file.try_lock_exclusive().is_ok() && path.starts_with("/dev/shm/") {
                        if let Err(x) = fs::remove_dir_all(path) {
                            println!("Warning: {x}");
                        }
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
