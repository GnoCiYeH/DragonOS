use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use cpu::KCpuStatMonitor;
use log::debug;
use mem::KMemStatMonitor;
use process::KProcessDebugMonitor;

use alloc::string::String;

use crate::arch::mm::barrier::mfence;
use crate::process::kthread::{KernelThreadClosure, KernelThreadMechanism};
use crate::process::ProcessControlBlock;
use crate::time::sleep::nanosleep;
use crate::time::PosixTimeSpec;
use crate::{
    arch::CurrentIrqArch,
    exception::InterruptArch,
    process::ProcessManager,
    sched::{schedule, SchedMode},
};

pub mod cpu;
pub mod mem;
pub mod process;

#[macro_export]
macro_rules! kmonitor_call {
    ($hook:expr) => {
        paste::paste! {
            {
                use crate::debug::kmonitor::wakeup_kmonitor;
                use crate::debug::kmonitor::process::*;
                wakeup_kmonitor();
                $hook;
            }
        }
    };
}

const KTRANSPORTER_DATA_SIZE: usize = 4096;

#[repr(C)]
pub(super) struct KMonitorTransportChannel {
    data: [u8; KTRANSPORTER_DATA_SIZE],
    size: i64,
    recv_seq: i64,
    send_seq: i64,
    packet_over: i64,
}

impl KMonitorTransportChannel {
    pub const fn default() -> Self {
        Self {
            data: [0; KTRANSPORTER_DATA_SIZE],
            size: 0,
            recv_seq: 0,
            send_seq: 0,
            packet_over: 0,
        }
    }
}

bitflags! {
    struct KTransporterFlags: i32 {
        // 表示当前的buffer是可读不可写的
        const WRITE_OVER = 1 << 0;
        // 表示一个数据包发送完成
        const PACKET_OVER = 1 << 1;
    }
}

pub(super) trait KMonitor {
    fn data(&self) -> Option<String>;

    fn transpoter(&self) -> &'static mut KMonitorTransportChannel;
}

static mut KMONITOR_THREAD: Option<Arc<ProcessControlBlock>> = None;

pub fn wakeup_kmonitor() {
    if let Some(ref kmonitor) = unsafe { &KMONITOR_THREAD } {
        ProcessManager::wakeup(kmonitor).unwrap();
    }
}

pub fn kmonitor_thread_init() {
    let closure =
        KernelThreadClosure::StaticEmptyClosure((&(kmonitor_thread_func as fn() -> i32), ()));
    let pcb = KernelThreadMechanism::create_and_run(closure, "kmonitor_thread".to_string())
        .ok_or("")
        .expect("create kmonitor_thread thread failed");
    unsafe {
        KMONITOR_THREAD = Some(pcb);
    }
}

#[inline(never)]
pub fn kmonitor_thread_func() -> i32 {
    let monitors: &mut [&'static dyn KMonitor] = &mut [
        KProcessDebugMonitor::instance(),
        KCpuStatMonitor::instance(),
        &KMemStatMonitor {},
    ];
    let mut monitor_need_send_packets: Vec<Vec<String>> = vec![Vec::new(); monitors.len()];
    let mut need_mark_sleep = true;
    loop {
        for (index, monitor) in monitors.into_iter().enumerate() {
            let packets = &mut monitor_need_send_packets[index];
            if packets.len() < 1000 {
                if let Some(data) = monitor.data() {
                    packets.push(data);
                }
            }

            let transporter = monitor.transpoter();
            if packets.is_empty() {
                continue;
            }

            // 有一次发消息就不标记sleep
            need_mark_sleep = false;
            let packet = &packets[0];
            let packet_bytes = packet.as_bytes();
            let mut packet_len = packet_bytes.len();
            let mut sended_size = 0;
            while packet_len > 0 {
                if transporter.recv_seq < transporter.send_seq {
                    // 休眠0.5秒
                    let _ = nanosleep(PosixTimeSpec::new(0, 500_000_000));
                    continue;
                }

                let size = if packet_len <= KTRANSPORTER_DATA_SIZE {
                    packet_len
                } else {
                    KTRANSPORTER_DATA_SIZE
                };

                transporter.data[..size]
                    .copy_from_slice(&packet_bytes[sended_size..sended_size + size]);
                transporter.size = size as i64;

                sended_size += size;
                packet_len -= size;

                mfence();
                if packet_len == 0 {
                    transporter.packet_over = 1;
                } else {
                    transporter.packet_over = 0;
                }

                transporter.send_seq += 1;
                mfence();
            }
            packets.remove(0);
        }

        if need_mark_sleep {
            // 睡眠10s
            let _ = nanosleep(PosixTimeSpec::new(0, 10_000_000_000));
        }
        need_mark_sleep = true;
    }
}
