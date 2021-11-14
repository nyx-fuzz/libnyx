#[derive(Debug, Copy, Clone)]
#[repr(C, packed(1))]
pub struct InterpreterData{
    pub executed_opcode_num: u32
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct IjonData {
    pub max_data: [u64;256],
}

#[derive(Copy, Clone)]
#[repr(C, packed(1))]
pub struct SharedFeedbackData{
    pub interpreter: InterpreterData,
    pad: [u8; 0x1000/2-std::mem::size_of::<InterpreterData>()],
    pub ijon: IjonData,
}

pub struct FeedbackBuffer {
    pub shared: &'static mut SharedFeedbackData,
}

impl FeedbackBuffer{
    pub fn new(shared: &'static mut SharedFeedbackData) -> Self{
        Self{shared}
    }
}