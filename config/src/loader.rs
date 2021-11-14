use crate::config::*;
use serde_derive::Serialize; 
use serde_derive::Deserialize; 
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize)]
pub struct QemuKernelConfigLoader {
    pub qemu_binary: Option<String>,
    pub kernel: Option<String>,
    pub ramfs: Option<String>,
    pub debug: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct QemuSnapshotConfigLoader {
    pub qemu_binary: Option<String>,
    pub hda: Option<String>,
    pub presnapshot: Option<String>,
    pub snapshot_path: Option<SnapshotPath>,
    pub debug: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ForkServerConfigLoader {
    pub args: Option<Vec<String>>,
    pub hide_output: Option<bool>,
    pub input_size: Option<usize>,
    pub env: Option<Vec<String>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum FuzzRunnerConfigLoader {
    QemuKernel(QemuKernelConfigLoader),
    QemuSnapshot(QemuSnapshotConfigLoader),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FuzzerConfigLoader {
    pub workdir_path: Option<String>,
    pub bitmap_size: Option<usize>,
    pub mem_limit: Option<usize>,
    pub time_limit: Option<Duration>,
    pub target_binary: Option<String>,
    pub threads: Option<usize>,
    pub thread_id: Option<usize>,
    pub cpu_pin_start_at: Option<usize>,
    pub seed_path: Option<String>,
    pub dict: Option<Vec<Vec<u8>>>,
    pub snapshot_placement: Option<SnapshotPlacement>,
    pub dump_python_code_for_inputs: Option<bool>,
    pub exit_after_first_crash: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConfigLoader {
    pub include_default_config_path: Option<String>,
    pub runner: FuzzRunnerConfigLoader,
    pub fuzz: FuzzerConfigLoader,
}
