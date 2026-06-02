import sys

with open("kernel/drivers/nvme.salt", "r") as f:
    content = f.read()

# Modify NvmeController struct to add IO Queues
old_struct = """pub struct NvmeController {
    pub bar: Ptr<u32>,
    pub asq: Ptr<SqEntry>,
    pub acq: Ptr<CqEntry>,
    pub asq_phys: u64,
    pub acq_phys: u64,
    pub asq_tail: u32,
    pub acq_head: u32,
    pub acq_phase: u16,
}"""

new_struct = """pub struct NvmeController {
    pub bar: Ptr<u32>,
    pub asq: Ptr<SqEntry>,
    pub acq: Ptr<CqEntry>,
    pub asq_phys: u64,
    pub acq_phys: u64,
    pub asq_tail: u32,
    pub acq_head: u32,
    pub acq_phase: u16,
    
    // I/O Queues
    pub iosq: Ptr<SqEntry>,
    pub iocq: Ptr<CqEntry>,
    pub iosq_phys: u64,
    pub iocq_phys: u64,
    pub iosq_tail: u32,
    pub iocq_head: u32,
    pub iocq_phase: u16,
}"""
content = content.replace(old_struct, new_struct)

# Add init_io_queues function
new_funcs = """
pub fn init_io_queues(ctrl_ptr: Ptr<NvmeController>, iosq_phys: u64, iocq_phys: u64) {
    let mut c = ctrl_ptr.offset(0).read();
    c.iosq = iosq_phys as Ptr<SqEntry>;
    c.iocq = iocq_phys as Ptr<CqEntry>;
    c.iosq_phys = iosq_phys;
    c.iocq_phys = iocq_phys;
    c.iosq_tail = 0;
    c.iocq_head = 0;
    c.iocq_phase = 1;
    ctrl_ptr.offset(0).write(c);

    // 1. Create I/O Completion Queue (IOCQ)
    // Opcode 0x05, NSID=0, cdw10=(size-1)<<16 | qid(1)
    // cdw11=1 (physically contiguous) | (1<<1) (interrupt enabled, though we poll)
    let mut cq_cmd = SqEntry {
        cdw0: 0x05,
        nsid: 0,
        rsvd2: 0,
        mptr: 0,
        dptr_prp1: iocq_phys,
        dptr_prp2: 0,
        cdw10: (31 << 16) | 1, // QID 1, size 32
        cdw11: 1, // Contiguous
        cdw12: 0, cdw13: 0, cdw14: 0, cdw15: 0
    };
    submit_admin_cmd(ctrl_ptr, (&cq_cmd) as Ptr<SqEntry>);
    poll_admin_cq(ctrl_ptr);
    
    // 2. Create I/O Submission Queue (IOSQ)
    // Opcode 0x01, NSID=0, cdw10=(size-1)<<16 | qid(1)
    // cdw11= (1<<16) CQID(1) | 1 (physically contiguous)
    let mut sq_cmd = SqEntry {
        cdw0: 0x01,
        nsid: 0,
        rsvd2: 0,
        mptr: 0,
        dptr_prp1: iosq_phys,
        dptr_prp2: 0,
        cdw10: (31 << 16) | 1, // QID 1, size 32
        cdw11: (1 << 16) | 1,  // CQID 1, Contiguous
        cdw12: 0, cdw13: 0, cdw14: 0, cdw15: 0
    };
    submit_admin_cmd(ctrl_ptr, (&sq_cmd) as Ptr<SqEntry>);
    poll_admin_cq(ctrl_ptr);
    
    serial.print("NVMe: I/O Queues created.\n");
}

pub fn submit_io_cmd(ctrl: Ptr<NvmeController>, cmd: Ptr<SqEntry>) {
    let asq = ctrl.offset(0).read().iosq;
    let tail = ctrl.offset(0).read().iosq_tail;
    
    let dest = asq.offset(tail as i64) as Ptr<u32>;
    let src = cmd as Ptr<u32>;
    let mut i = 0;
    while i < 16 {
        dest.offset(i as i64).write(src.offset(i as i64).read());
        i = i + 1;
    }

    let mut next_tail = tail + 1;
    if next_tail >= 32 { next_tail = 0; }
    
    let mut c = ctrl.offset(0).read();
    c.iosq_tail = next_tail;
    ctrl.offset(0).write(c);

    // Ring SQ1 Tail Doorbell (SQ1 TDBL is at 0x1000 + (2 * 1 * 4) = 0x1008)
    // Assuming doorbell stride is 4 bytes (1 << DSTRD=0, DSTRD*4)
    // CAP.DSTRD is 0 in QEMU usually. So SQ0_TDBL=0x1000, CQ0_HDBL=0x1004, SQ1_TDBL=0x1008
    let sq1_tdbl = 0x1008 / 4;
    let bar = ctrl.offset(0).read().bar;
    bar.offset(sq1_tdbl).write(next_tail as u32);
    unsafe { outb(0x80, 0); }
}

pub fn poll_io_cq(ctrl: Ptr<NvmeController>) -> u16 {
    let acq = ctrl.offset(0).read().iocq;
    let head = ctrl.offset(0).read().iocq_head;
    let expected_phase = ctrl.offset(0).read().iocq_phase;
    
    let entry = acq.offset(head as i64);
    
    while ((entry.offset(0).read().status & 1) != expected_phase) {
        unsafe { outb(0x80, 0); }
    }
    
    let status = entry.offset(0).read().status;
    
    let mut next_head = head + 1;
    let mut next_phase = expected_phase;
    if next_head >= 32 {
        next_head = 0;
        if next_phase == 1 { next_phase = 0; } else { next_phase = 1; }
    }
    
    let mut c = ctrl.offset(0).read();
    c.iocq_head = next_head;
    c.iocq_phase = next_phase;
    ctrl.offset(0).write(c);
    
    // Ring CQ1 Head Doorbell (CQ1 HDBL is at 0x1000 + (3 * 4) = 0x100C)
    let cq1_hdbl = 0x100C / 4;
    let bar = ctrl.offset(0).read().bar;
    bar.offset(cq1_hdbl).write(next_head as u32);
    
    return status;
}

pub fn read_block(ctrl: Ptr<NvmeController>, lba: u64, buf_phys: u64, blocks: u16) {
    // NVMe NVM Command Set: Opcode 0x02 is Read
    // cdw10, cdw11 = SLBA
    // cdw12 = Number of Logical Blocks (0-based)
    let mut read_cmd = SqEntry {
        cdw0: 0x02,
        nsid: 1, // NSID 1 for first namespace
        rsvd2: 0,
        mptr: 0,
        dptr_prp1: buf_phys,
        dptr_prp2: 0, // Assume contiguous single page for now
        cdw10: (lba & 0xFFFFFFFF) as u32,
        cdw11: (lba >> 32) as u32,
        cdw12: (blocks as u32) - 1,
        cdw13: 0, cdw14: 0, cdw15: 0
    };
    submit_io_cmd(ctrl, (&read_cmd) as Ptr<SqEntry>);
}

pub fn write_block(ctrl: Ptr<NvmeController>, lba: u64, buf_phys: u64, blocks: u16) {
    // Opcode 0x01 is Write
    let mut write_cmd = SqEntry {
        cdw0: 0x01,
        nsid: 1,
        rsvd2: 0,
        mptr: 0,
        dptr_prp1: buf_phys,
        dptr_prp2: 0,
        cdw10: (lba & 0xFFFFFFFF) as u32,
        cdw11: (lba >> 32) as u32,
        cdw12: (blocks as u32) - 1,
        cdw13: 0, cdw14: 0, cdw15: 0
    };
    submit_io_cmd(ctrl, (&write_cmd) as Ptr<SqEntry>);
}
"""

content += new_funcs

# Fix init to set the dummy values initially
content = content.replace("acq_phase: 1, // Phase tag is 1 initially", "acq_phase: 1,\niosq: 0 as Ptr<SqEntry>,\niocq: 0 as Ptr<CqEntry>,\niosq_phys: 0,\niocq_phys: 0,\niosq_tail: 0,\niocq_head: 0,\niocq_phase: 1,\n")

with open("kernel/drivers/nvme.salt", "w") as f:
    f.write(content)
