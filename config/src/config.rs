use std::time::Duration;
use serde_derive::Serialize; 
use serde_derive::Deserialize; 
use std::fs::File;
use std::path::{Path};
use crate::loader::*;

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

#[derive(Clone)]
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

#[derive(Clone, Serialize, Deserialize)]
pub enum SnapshotPath {
    Reuse(String),
    Create(String),
    DefaultPath,
}

#[derive(Clone)]
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

#[derive(Clone)]
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

#[derive(Copy, Clone, Serialize, Deserialize)]
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

#[derive(Clone)]
pub struct FuzzerConfig {
    pub spec_path: String,
    pub workdir_path: String,
    pub bitmap_size: usize,
    pub mem_limit: usize,
    pub time_limit: Duration,
    pub target_binary: Option<String>,
    pub threads: usize,
    pub thread_id: usize,
    pub cpu_pin_start_at: usize,
    pub seed_path: Option<String>,
    pub dict: Vec<Vec<u8>>,
    pub snapshot_placement: SnapshotPlacement,
    pub dump_python_code_for_inputs: Option<bool>,
    pub exit_after_first_crash: bool
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
            mem_limit: config.mem_limit.or(default.mem_limit).expect("no mem_limit specified"),
            time_limit: config.time_limit.or(default.time_limit).expect("no time_limit specified"),
            target_binary: config.target_binary.or(default.target_binary),
            threads: config.threads.or(default.threads).expect("no threads specified"),
            thread_id: config.thread_id.or(default.thread_id).expect("no thread_id specified"),
            cpu_pin_start_at: config.cpu_pin_start_at.or(default.cpu_pin_start_at).expect("no cpu_pin_start_at specified"),
            seed_path: seed_path_value,
            dict: config.dict.or(default.dict).expect("no dict specified"),
            snapshot_placement: config.snapshot_placement.or(default.snapshot_placement).expect("no snapshot_placement specified"),
            dump_python_code_for_inputs: config.dump_python_code_for_inputs.or(default.dump_python_code_for_inputs),
            exit_after_first_crash: config.exit_after_first_crash.unwrap_or(default.exit_after_first_crash.unwrap_or(false)),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub runner: FuzzRunnerConfig,
    pub fuzz: FuzzerConfig,
}

impl Config{
    pub fn new_from_loader(sharedir: &str, default_config_folder: &str, default: ConfigLoader, config: ConfigLoader) -> Self{
        Self{
            runner: FuzzRunnerConfig::new_from_loader(&default_config_folder, default.runner, config.runner),
            fuzz:  FuzzerConfig::new_from_loader(&sharedir, default.fuzz, config.fuzz),
        }
    }

    pub fn new_from_sharedir(sharedir: &str) -> Self {
        let path_to_config = format!("{}/config.ron", sharedir);
        let cfg_file = File::open(&path_to_config).expect("could not open config file");
        let mut cfg: ConfigLoader = ron::de::from_reader(cfg_file).unwrap();

        let default_path = into_absolute_path(sharedir, cfg.include_default_config_path.unwrap());
        let default_config_folder = Path::new(&default_path).parent().unwrap().to_str().unwrap();
        cfg.include_default_config_path = Some(default_path.clone());

        let default_file = File::open(cfg.include_default_config_path.as_ref().expect("no default config given")).expect("could not open config file");
        let default: ConfigLoader = ron::de::from_reader(default_file).unwrap();

        Self::new_from_loader(&sharedir, &default_config_folder, default, cfg)
    }
}
