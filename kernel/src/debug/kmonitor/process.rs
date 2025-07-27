use crate::{
    libs::{once::Once, spinlock::SpinLock},
    process::{Pid, ProcessControlBlock},
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use hashbrown::HashMap;
use log::debug;
use miniserde::{json, Serialize};

use super::{KMonitor, KMonitorTransportChannel};

#[no_mangle]
static mut KPROCESS_MONITOR_TRANSPORT_CHANNEL: KMonitorTransportChannel =
    KMonitorTransportChannel::default();

#[derive(Default, Serialize, Clone)]
struct ProcessDebugData {
    pub pid: usize,
    pub name: String,
    pub run_time_stat: f64,
}

#[derive(Serialize)]
struct ProcessDebugDataPacket {
    pub tag: ProcessDebugTag,
    pub data: ProcessDebugData,
}

// 消息头，随机数保证数据正确性
#[repr(align(32))]
#[derive(Serialize, PartialEq, Eq)]
enum ProcessDebugTag {
    Execve,
    Update,
    Exit,
}

pub struct KProcessDebugMonitor {
    debug_data_map: SpinLock<HashMap<usize, ProcessDebugData>>,
    packets: SpinLock<Vec<ProcessDebugDataPacket>>,
}

impl KProcessDebugMonitor {
    pub fn instance() -> &'static Self {
        static mut PROCESS_DEBUG_MONITOR: Option<KProcessDebugMonitor> = None;
        static ONCE: Once = Once::new();

        ONCE.call_once(|| unsafe {
            PROCESS_DEBUG_MONITOR = Some(KProcessDebugMonitor {
                debug_data_map: SpinLock::new(HashMap::new()),
                packets: SpinLock::new(Vec::new()),
            });
        });

        return unsafe { PROCESS_DEBUG_MONITOR.as_ref().unwrap() };
    }

    #[inline(never)]
    pub fn on_execve(pcb: &Arc<ProcessControlBlock>) {
        let instance = Self::instance();
        let info = ProcessDebugData {
            pid: pcb.pid().into(),
            name: pcb.basic().name().into(),
            run_time_stat: 0.0,
        };
        instance
            .debug_data_map
            .lock_irqsave()
            .insert(pcb.pid().into(), info.clone());
        instance
            .packets
            .lock_irqsave()
            .push(ProcessDebugDataPacket {
                tag: ProcessDebugTag::Execve,
                data: info,
            });
    }

    fn update_process_debug_info(
        &self,
        pid: Pid,
        tag: ProcessDebugTag,
        mut update: impl FnMut(&mut ProcessDebugData),
    ) {
        if let Some(info) = self.debug_data_map.lock_irqsave().get_mut(&pid.into()) {
            update(info);
            let mut packets = self.packets.lock_irqsave();
            let mut updated = false;
            for packet in packets.iter_mut().rev() {
                if packet.tag == ProcessDebugTag::Update && packet.data.pid == pid.data() {
                    packet.data = info.clone();
                    updated = true;
                }
            }
            if !updated {
                packets.push(ProcessDebugDataPacket {
                    tag,
                    data: info.clone(),
                });
            }
        }
    }

    pub fn update_process_cpu_time(pid: Pid, count: usize) {
        Self::instance().update_process_debug_info(pid, ProcessDebugTag::Update, |info| {
            info.run_time_stat += count as f64 / 1_000_000_000.0
        });
    }

    pub fn on_process_exit(pid: Pid) {
        let instance = Self::instance();
        instance.update_process_debug_info(pid, ProcessDebugTag::Exit, |_| {});
        instance.debug_data_map.lock_irqsave().remove(&pid.data());
    }
}

impl KMonitor for KProcessDebugMonitor {
    #[inline(never)]
    fn data(&self) -> Option<String> {
        let mut packets = self.packets.lock_irqsave();
        if !packets.is_empty() {
            let json = json::to_string(packets.as_ref() as &Vec<ProcessDebugDataPacket>);
            packets.clear();
            return Some(json);
        }
        None
    }

    fn transpoter(&self) -> &'static mut KMonitorTransportChannel {
        unsafe { &mut KPROCESS_MONITOR_TRANSPORT_CHANNEL }
    }
}
