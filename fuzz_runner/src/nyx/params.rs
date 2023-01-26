use crate::config::SnapshotPath;
use crate::config::IptFilter;

pub struct KernelVmParams {
    pub qemu_binary: String,
    pub kernel: String,
    pub sharedir: String,
    pub ramfs: String,
    pub ram_size: usize,
    pub bitmap_size: usize,
    pub qemu_args: String,
    pub debug: bool,

    pub dump_python_code_for_inputs: bool,
    pub write_protected_input_buffer: bool,
    pub cow_primary_size: Option<u64>,
    pub ipt_filters: [IptFilter; 4],
    pub input_buffer_size: usize,
}

pub struct SnapshotVmParams{
    pub qemu_binary: String,
    pub hda: String,
    pub sharedir: String,
    pub presnapshot: String,
    pub snapshot_path: SnapshotPath,
    pub ram_size: usize,
    pub bitmap_size: usize,
    pub debug: bool,

    pub dump_python_code_for_inputs: bool,
    pub write_protected_input_buffer: bool,
    pub cow_primary_size: Option<u64>,
    pub ipt_filters: [IptFilter; 4],
    pub input_buffer_size: usize,
}

pub struct QemuParams {
    pub cmd: Vec<String>,
    pub qemu_aux_buffer_filename: String,
    pub control_filename: String,
    pub workdir: String,
    pub qemu_id: usize,
    pub bitmap_size: usize,
    pub payload_size: usize,

    pub dump_python_code_for_inputs: bool,
    pub write_protected_input_buffer: bool,
    pub cow_primary_size: Option<u64>,
}

impl QemuParams {
    pub fn new_from_snapshot(workdir: &str, qemu_id: usize, cpu: usize, params: &SnapshotVmParams, create_snapshot_file: bool) -> QemuParams{
    
        
        let qemu_aux_buffer_filename = format!("{workdir}/aux_buffer_{qemu_id}");
        let control_filename = format!("{workdir}/interface_{qemu_id}");

        let mut cmd = vec![];
        cmd.push(params.qemu_binary.to_string());

        cmd.push("-drive".to_string());
        cmd.push(format!("file={},format=raw,index=0,media=disk", params.hda));

        if !params.debug {
            cmd.push("-display".to_string());
            cmd.push("none".to_string());
        } else {
            cmd.push("-vnc".to_string());
            cmd.push(format!(":{}",qemu_id+cpu));
        }

        cmd.push("-serial".to_string());
        if params.debug {
            cmd.push("mon:stdio".to_string());
        } else {
            cmd.push("stdio".to_string());
        }

        cmd.push("-enable-kvm".to_string());

        cmd.push("-net".to_string());
        cmd.push("none".to_string());

        cmd.push("-k".to_string());
        cmd.push("de".to_string());

        cmd.push("-m".to_string());
        cmd.push(params.ram_size.to_string());

        cmd.push("-chardev".to_string());
        cmd.push(format!(
            "socket,server,path={control_filename},id=nyx_interface"
        ));

        cmd.push("-device".to_string());
        let mut nyx_ops = "nyx,chardev=nyx_interface".to_string();
        nyx_ops += &format!(",bitmap_size={}", params.bitmap_size);
        nyx_ops += &format!(",input_buffer_size={}", params.input_buffer_size);
        nyx_ops += &format!(",worker_id={qemu_id}");
        nyx_ops += &format!(",workdir={workdir}");
        nyx_ops += &format!(",sharedir={}", params.sharedir);


        let mut i = 0;
        for filter in params.ipt_filters{
            if filter.a != 0 && filter.b != 0 {
                nyx_ops += &format!(",ip{}_a={},ip{}_b={}", i, filter.a, i, filter.b);
                i += 1;
            }
        }

        if params.cow_primary_size.is_some(){
            nyx_ops += &format!(",cow_primary_size={}", params.cow_primary_size.unwrap());
        }

        cmd.push(nyx_ops);

        cmd.push("-machine".to_string());
        cmd.push("kAFL64-v1".to_string());

        cmd.push("-cpu".to_string());
        cmd.push("kAFL64-Hypervisor-v1".to_string());

        match &params.snapshot_path {
            SnapshotPath::Create(path) => {
                if create_snapshot_file {
                    cmd.push("-fast_vm_reload".to_string());
                    cmd.push(format!("path={},load=off,pre_path={}", path,params.presnapshot));
                }
                else{
                    cmd.push("-fast_vm_reload".to_string());
                    cmd.push(format!("path={},load=off,pre_path={},skip_serialization=on", path,params.presnapshot));
                }
            },
            SnapshotPath::Reuse(path) => {
                cmd.push("-fast_vm_reload".to_string());
                cmd.push(format!("path={path},load=on"));
            }
            SnapshotPath::DefaultPath => panic!(),
        }

        QemuParams {
            cmd,
            qemu_aux_buffer_filename,
            control_filename,
            workdir: workdir.to_string(),
            qemu_id,
            bitmap_size: params.bitmap_size,
            payload_size: params.input_buffer_size,
            dump_python_code_for_inputs: params.dump_python_code_for_inputs,
            write_protected_input_buffer: params.write_protected_input_buffer,
            cow_primary_size: params.cow_primary_size,
        }
    }

    pub fn new_from_kernel(workdir: &str, qemu_id: usize, params: &KernelVmParams, create_snapshot_file: bool) -> QemuParams {

        let qemu_aux_buffer_filename = format!("{workdir}/aux_buffer_{qemu_id}");
        let control_filename = format!("{workdir}/interface_{qemu_id}");

        let mut cmd = vec![
            params.qemu_binary.to_string(),
            "-kernel".to_string(),
            params.kernel.to_string(),

            "-initrd".to_string(),
            params.ramfs.to_string(),

            "-append".to_string(),
            params.qemu_args.to_string(),

            "-enable-kvm".to_string(),

            "-net".to_string(),
            "none".to_string(),

            "-k".to_string(),
            "de".to_string(),

            "-m".to_string(),
            params.ram_size.to_string(),
        ];

        if !params.debug {
            cmd.push("-display".to_string());
            cmd.push("none".to_string());
        }

        cmd.push("-serial".to_string());
        if params.debug {
            cmd.push("mon:stdio".to_string());
        } else {
            cmd.push("none".to_string());
        }



        cmd.push("-chardev".to_string());
        cmd.push(format!(
            "socket,server,path={control_filename},id=nyx_interface"
        ));

        cmd.push("-device".to_string());
        let mut nyx_ops = "nyx,chardev=nyx_interface".to_string();
        nyx_ops += &format!(",bitmap_size={}", params.bitmap_size);
        nyx_ops += &format!(",input_buffer_size={}", params.input_buffer_size);
        nyx_ops += &format!(",worker_id={qemu_id}");
        nyx_ops += &format!(",workdir={workdir}");
        nyx_ops += &format!(",sharedir={}", params.sharedir);

        let mut i = 0;
        for filter in params.ipt_filters{
            if filter.a != 0 && filter.b != 0 {
                nyx_ops += &format!(",ip{}_a={:x},ip{}_b={:x}", i, filter.a, i, filter.b);
                i += 1;
            }
        }

        if params.cow_primary_size.is_some(){
            nyx_ops += &format!(",cow_primary_size={}", params.cow_primary_size.unwrap());
        }

        cmd.push(nyx_ops);

        cmd.push("-machine".to_string());
        cmd.push("kAFL64-v1".to_string());

        cmd.push("-cpu".to_string());
        cmd.push("kAFL64-Hypervisor-v1,+vmx".to_string());

        if create_snapshot_file {
            cmd.push("-fast_vm_reload".to_string());
            if qemu_id == 0{
                cmd.push(format!("path={workdir}/snapshot/,load=off"));
            } else {
                cmd.push(format!("path={workdir}/snapshot/,load=on"));
            }
        }

        QemuParams {
            cmd,
            qemu_aux_buffer_filename,
            control_filename,
            workdir: workdir.to_string(),
            qemu_id,
            bitmap_size: params.bitmap_size,
            payload_size: params.input_buffer_size,
            dump_python_code_for_inputs: params.dump_python_code_for_inputs,
            write_protected_input_buffer: params.write_protected_input_buffer,
            cow_primary_size: params.cow_primary_size,
        }
    }
}
