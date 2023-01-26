use nix::sys::wait::WaitStatus;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExitReason {
    Normal(i32),
    Timeout,
    Signaled(i32),
    Crash(Vec<u8>),
    Asan,
    Stopped(i32),
    FuzzerError,
    InvalidWriteToPayload(Vec<u8>),
}

impl ExitReason {
    pub fn from_wait_status(status: WaitStatus) -> ExitReason {
        match status {
            WaitStatus::Exited(_, return_value) => ExitReason::Normal(return_value),
            WaitStatus::Signaled(_, signal, _) => ExitReason::Signaled(signal as i32),
            WaitStatus::Stopped(_, signal) => ExitReason::Stopped(signal as i32),
            _ => panic!("Unknown WaitStatus: {:?}", status),
        }
    }

    pub fn is_normal(&self) -> bool{
        use ExitReason::*;
        matches!(self, Normal(_))
    }

    pub fn name(&self) -> &str{
        use ExitReason::*;
        match self {
            Normal(_) => "normal",
            Timeout => "timeout",
            Signaled(_) => "signal",
            Crash(_) => "crash",
            Asan => "asan",
            Stopped(_) => "stop",
            InvalidWriteToPayload(_) => "invalid_write_to_payload_buffer",
            FuzzerError => unreachable!(),
        }
    }
}
