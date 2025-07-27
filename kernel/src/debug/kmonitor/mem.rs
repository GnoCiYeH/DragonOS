use alloc::vec::Vec;
use core::sync::atomic::AtomicBool;

use alloc::string::String;

use crate::{
    arch::mm::LockedFrameAllocator,
    mm::allocator::page_frame::FrameAllocator,
    process::ProcessManager,
    time::timer::{clock, next_n_ms_timer_jiffies},
};

use super::{KMonitor, KMonitorTransportChannel};
use miniserde::{json, Serialize};

#[no_mangle]
static mut KMEM_MONITOR_TRANSPORT_CHANNEL: KMonitorTransportChannel =
    KMonitorTransportChannel::default();

#[derive(Serialize)]
struct MemStatPacket {
    total: usize,
    used: usize,
    processes: Vec<ProcessMemStatPacket>,
}

#[derive(Serialize)]
struct ProcessMemStatPacket {
    pid: usize,
    virt_usage: usize,
    phys_usage: usize,
}

pub(super) struct KMemStatMonitor {}

impl KMonitor for KMemStatMonitor {
    fn data(&self) -> Option<String> {
        static mut NEXT_UPDATE_TIME: u64 = 0;
        // 最多一秒统计一次
        unsafe {
            if clock() > NEXT_UPDATE_TIME {
                NEXT_UPDATE_TIME = next_n_ms_timer_jiffies(1000);
            } else {
                return None;
            }
        }

        let usage = unsafe { LockedFrameAllocator.usage() };
        let mut packet = MemStatPacket {
            total: usage.total().bytes(),
            used: usage.used().bytes(),
            processes: vec![],
        };
        for pid in ProcessManager::get_all_processes() {
            if let Some(pcb) = ProcessManager::find(pid) {
                if let Some(vm) = pcb.basic().user_vm() {
                    let va_guard = vm.read_irqsave();
                    packet.processes.push(ProcessMemStatPacket {
                        pid: pid.data(),
                        virt_usage: va_guard.virt_mem_usage(),
                        phys_usage: va_guard.phys_mem_usage(),
                    });
                }
            }
        }

        return Some(json::to_string(&packet));
    }

    fn transpoter(&self) -> &'static mut KMonitorTransportChannel {
        unsafe { &mut KMEM_MONITOR_TRANSPORT_CHANNEL }
    }
}
