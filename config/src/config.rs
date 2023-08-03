use std::time::Duration;
use serde_derive::Serialize; 
use serde_derive::Deserialize; 
use std::fs::File;
use std::path::{Path};
use crate::loader::*;

use libc::fcntl;

const DEFAULT_AUX_BUFFER_SIZE: usize = 4096;

fn into_absolute_path(path_to_sharedir: &str, path_to_file: String) -> String {
    let path_to_default_config = Path::new(&path_to_file);

    if path_to_default_config.is_relative(){
        let path = &format!("{}/{}", path_to_sharedir, path_to_file);
        let absolute_path = Path::new(&path);
        return absolute_path.canonicalize().unwrap().to_str().unwrap().to_string();
    }
    else{
        return path_to_default_config.to_str().unwrap().to_string();
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct IptFilter {
    pub a: u64,
    pub b: u64,
}

#[derive(Clone, Debug)]
pub struct QemuKernelConfig {
    pub qemu_binary: String,
    pub kernel: String,
    pub ramfs: String,
    pub debug: bool,
}

impl QemuKernelConfig{
    pub fn new_from_loader(default_config_folder: &str, default: QemuKernelConfigLoader, config: QemuKernelConfigLoader) -> Self {
        let mut qemu_binary = config.qemu_binary.or(default.qemu_binary).expect("no qemu_binary specified");
        let mut kernel = config.kernel.or(default.kernel).expect("no kernel specified");
        let mut ramfs = config.ramfs.or(default.ramfs).expect("no ramfs specified");
        
        qemu_binary = into_absolute_path(default_config_folder, qemu_binary);
        kernel = into_absolute_path(default_config_folder, kernel);
        ramfs = into_absolute_path(default_config_folder, ramfs);

        Self{
            qemu_binary: qemu_binary,
            kernel: kernel,
            ramfs: ramfs,
            debug: config.debug.or(default.debug).expect("no debug specified"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SnapshotPath {
    Reuse(String),
    Create(String),
    DefaultPath,
}

#[derive(Clone, Debug)]
pub struct QemuSnapshotConfig {
    pub qemu_binary: String,
    pub hda: String,
    pub presnapshot: String,
    pub snapshot_path: SnapshotPath,
    pub debug: bool,
}

impl QemuSnapshotConfig{
    pub fn new_from_loader(default_config_folder: &str, default: QemuSnapshotConfigLoader, config: QemuSnapshotConfigLoader) -> Self {

        let mut qemu_binary = config.qemu_binary.or(default.qemu_binary).expect("no qemu_binary specified");
        let mut hda = config.hda.or(default.hda).expect("no hda specified");
        let mut presnapshot = config.presnapshot.or(default.presnapshot).expect("no presnapshot specified");
        qemu_binary = into_absolute_path(default_config_folder, qemu_binary);
        hda = into_absolute_path(default_config_folder, hda);
        presnapshot = into_absolute_path(default_config_folder, presnapshot);

        Self{
            qemu_binary: qemu_binary,
            hda: hda,
            presnapshot: presnapshot,
            snapshot_path: config.snapshot_path.or(default.snapshot_path).expect("no snapshot_path specified"),
            debug: config.debug.or(default.debug).expect("no debug specified"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum FuzzRunnerConfig {
    QemuKernel(QemuKernelConfig),
    QemuSnapshot(QemuSnapshotConfig),
}

impl FuzzRunnerConfig{
    pub fn new_from_loader(default_config_folder: &str, default: FuzzRunnerConfigLoader, config: FuzzRunnerConfigLoader) -> Self {
        match (default, config){
            (FuzzRunnerConfigLoader::QemuKernel(d),
            FuzzRunnerConfigLoader::QemuKernel(c)) => { Self::QemuKernel(QemuKernelConfig::new_from_loader(default_config_folder, d, c))},
            (FuzzRunnerConfigLoader::QemuSnapshot(d),
            FuzzRunnerConfigLoader::QemuSnapshot(c)) => { Self::QemuSnapshot(QemuSnapshotConfig::new_from_loader(default_config_folder, d, c))},
            _ => panic!("conflicting FuzzRunner configs"),
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotPlacement {
    None,
    Balanced,
    Aggressive,
}

impl std::str::FromStr for SnapshotPlacement {
    type Err = ron::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ron::de::from_str(s)
    }
}

#[derive(Clone, Debug)]
pub struct FuzzerConfig {
    pub spec_path: String,
    pub workdir_path: String,
    pub bitmap_size: usize,
    pub input_buffer_size: usize,
    pub mem_limit: usize,
    pub time_limit: Duration,
    pub seed_path: Option<String>,
    pub dict: Vec<Vec<u8>>,
    pub snapshot_placement: SnapshotPlacement,
    pub dump_python_code_for_inputs: Option<bool>,
    pub exit_after_first_crash: bool,
    pub write_protected_input_buffer: bool,
    pub cow_primary_size: Option<u64>,
    pub ipt_filters: [IptFilter;4],
}
impl FuzzerConfig{
    pub fn new_from_loader(sharedir: &str, default: FuzzerConfigLoader, config: FuzzerConfigLoader) -> Self {

        let seed_path = config.seed_path.or(default.seed_path).unwrap();
        let seed_path_value = if seed_path.is_empty() {
            None
        }
        else{
            Some(into_absolute_path(&sharedir, seed_path))
        };

        Self{
            spec_path: format!("{}/spec.msgp",sharedir),
            workdir_path: config.workdir_path.or(default.workdir_path).expect("no workdir_path specified"),
            bitmap_size: config.bitmap_size.or(default.bitmap_size).expect("no bitmap_size specified"),
            input_buffer_size: config.input_buffer_size,
            mem_limit: config.mem_limit.or(default.mem_limit).expect("no mem_limit specified"),
            time_limit: config.time_limit.or(default.time_limit).expect("no time_limit specified"),
            seed_path: seed_path_value,
            dict: config.dict.or(default.dict).expect("no dict specified"),
            snapshot_placement: config.snapshot_placement.or(default.snapshot_placement).expect("no snapshot_placement specified"),
            dump_python_code_for_inputs: config.dump_python_code_for_inputs.or(default.dump_python_code_for_inputs),
            exit_after_first_crash: config.exit_after_first_crash.unwrap_or(default.exit_after_first_crash.unwrap_or(false)),
            write_protected_input_buffer: config.write_protected_input_buffer,
            cow_primary_size: if config.cow_primary_size != 0 { Some( config.cow_primary_size as u64) } else { None },
            ipt_filters: [
                config.ip0,
                config.ip1,
                config.ip2,
                config.ip3,
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub enum QemuNyxRole {
    /* Standalone mode, snapshot is kept in memory and not serialized. */
    StandAlone,

    /* Serialize the VM snapshot after the root snapshot has been created.
     * The serialized snapshot will be stored in the workdir and the snapshot 
     * will later be used by the child processes. */
    Parent,

    /* Wait for the snapshot to be created by the parent process and 
     * deserialize it from the workdir. This way all child processes can
     * mmap() the snapshot files and access the snapshot directly via shared memory. 
     * Consequently, this will result in a much lower memory usage compared to spawning
     * multiple StandAlone-type instances. */
    Child,
}

#[derive(Clone, Debug)]
/* runtime specific configuration */
pub struct RuntimeConfig {
    /*  Configurable option to redirect hprintf to a file descriptor.
     *  If None, hprintf will be redirected to stdout via println!().
     */
    hprintf_fd: Option<i32>,
    
    /* Configurable option to specify the role of the process.
     *  If StandAlone, the process will not serialize the snapshot and keep everything in memory.
     *  If Parent, the process will create a snapshot and serialize it.
     *  If Child, the process will wait for the parent to create a snapshot and deserialize it. */
    process_role: QemuNyxRole,

    /* Configurable option to reuse a snapshot from a previous run (useful to avoid VM bootstrapping). */
    reuse_snapshot_path: Option<String>,

    /* enable advanced VM debug mode (such as spawning a VNC server per VM) */
    debug_mode: bool,

    /* worker_id of the current QEMU Nyx instance */
    worker_id: usize,

    /* aux_buffer size */
    aux_buffer_size: usize,
}

impl RuntimeConfig{
    pub fn new() -> Self {
        Self{
            hprintf_fd: None,
            process_role: QemuNyxRole::StandAlone,
            reuse_snapshot_path: None,
            debug_mode: false,
            worker_id: 0,
            aux_buffer_size: DEFAULT_AUX_BUFFER_SIZE,
        }
    }

    pub fn hprintf_fd(&self) -> Option<i32> {
        self.hprintf_fd
    }

    pub fn process_role(&self) -> &QemuNyxRole {
        &self.process_role
    }

    pub fn set_hpintf_fd(&mut self, fd: i32){
        /* sanitiy check to prevent invalid file descriptors via F_GETFD */
        unsafe { 
            /* TODO: return error instead of panicking */
            assert!(fcntl(fd, libc::F_GETFD) != -1); 
        };

        self.hprintf_fd = Some(fd);
    }

    pub fn set_process_role(&mut self, role: QemuNyxRole){
        self.process_role = role;
    }

    pub fn reuse_root_snapshot_path(&self) -> Option<String> {
        self.reuse_snapshot_path.clone()
    }

    pub fn set_reuse_snapshot_path(&mut self, path: String){
        let path = Path::new(&path).canonicalize().unwrap().to_str().unwrap().to_string();
        self.reuse_snapshot_path = Some(path);
    }

    pub fn debug_mode(&self) -> bool {
        self.debug_mode
    }

    pub fn set_debug_mode(&mut self, debug_mode: bool){
        self.debug_mode = debug_mode;
    }

    pub fn worker_id(&self) -> usize {
        self.worker_id
    }

    pub fn set_worker_id(&mut self, thread_id: usize){
        self.worker_id = thread_id;
    }

    pub fn set_aux_buffer_size(&mut self, aux_buffer_size: usize) -> bool{

        if aux_buffer_size < DEFAULT_AUX_BUFFER_SIZE || (aux_buffer_size & 0xfff) != 0 {
            return false;
        }

        self.aux_buffer_size = aux_buffer_size;
        return true
    }

    pub fn aux_buffer_size(&self) -> usize {
        self.aux_buffer_size
    }
    
}

#[derive(Clone, Debug)]
pub struct Config {
    pub runner: FuzzRunnerConfig,
    pub fuzz: FuzzerConfig,
    pub runtime: RuntimeConfig,
}

impl Config{
    pub fn new_from_loader(sharedir: &str, default_config_folder: &str, default: ConfigLoader, config: ConfigLoader) -> Self{
        Self{
            runner: FuzzRunnerConfig::new_from_loader(&default_config_folder, default.runner, config.runner),
            fuzz:  FuzzerConfig::new_from_loader(&sharedir, default.fuzz, config.fuzz),
            runtime: RuntimeConfig::new(),
        }
    }

    pub fn new_from_sharedir(sharedir: &str) -> Result<Self, String> {
        let path_to_config = format!("{}/config.ron", sharedir);

        let cfg_file = match File::open(&path_to_config){
            Ok(x) => {x},
            Err(_) => return Err(format!("file or folder not found ({})!", path_to_config)),
        }; 

        let mut cfg: ConfigLoader = match ron::de::from_reader(cfg_file){
            Ok(x) => {x},
            Err(x) => return Err(format!("invalid configuration ({})!", x)),
        };

        let include_default_config_path = match cfg.include_default_config_path{
            Some(x) => {x},
            None => return Err(format!("no path to default configuration given!")),
        };

        let default_path = into_absolute_path(sharedir, include_default_config_path);
        let default_config_folder = Path::new(&default_path).parent().unwrap().to_str().unwrap();
        cfg.include_default_config_path = Some(default_path.clone());

        let default_file = match File::open(default_path.clone()){
            Ok(x) => x,
            Err(_) => return Err(format!("default config not found ({})!", default_path)),
        };
         
        let default: ConfigLoader = match ron::de::from_reader(default_file){
            Ok(x) => {x},
            Err(x) => return Err(format!("invalid default configuration ({})!", x)),
        };

        Ok(Self::new_from_loader(&sharedir, &default_config_folder, default, cfg))
    }
}
