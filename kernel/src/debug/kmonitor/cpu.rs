use alloc::vec::Vec;
use miniserde::{json, Serialize};

use crate::{
    libs::{once::Once, spinlock::SpinLock},
    process::Pid,
    smp::core::smp_get_processor_id,
};

use super::{KMonitor, KMonitorTransportChannel};
use alloc::string::String;

#[no_mangle]
static mut KCPU_MONITOR_TRANSPORT_CHANNEL: KMonitorTransportChannel =
    KMonitorTransportChannel::default();

#[derive(Serialize)]
struct CpuStatPacket {
    core_id: u32,
    pid: usize,
    delta: usize,
}

pub struct KCpuStatMonitor {
    packets: SpinLock<Vec<CpuStatPacket>>,
}

impl KCpuStatMonitor {
    pub fn instance() -> &'static Self {
        static mut PROCESS_DEBUG_MONITOR: Option<KCpuStatMonitor> = None;
        static ONCE: Once = Once::new();

        ONCE.call_once(|| unsafe {
            PROCESS_DEBUG_MONITOR = Some(KCpuStatMonitor {
                packets: SpinLock::new(Vec::new()),
            });
        });

        return unsafe { PROCESS_DEBUG_MONITOR.as_ref().unwrap() };
    }

    pub fn on_tick(pid: Pid, delta: usize) {
        let core_id = smp_get_processor_id().into();
        let pid = pid.data();
        let mut packets = Self::instance().packets.lock_irqsave();

        let mut updated = false;
        for packet in packets.iter_mut() {
            if packet.core_id == core_id && packet.pid == pid {
                packet.delta += delta;
                updated = true;
                break;
            }
        }

        if !updated {
            packets.push(CpuStatPacket {
                core_id,
                pid,
                delta,
            });
        }
    }
}

impl KMonitor for KCpuStatMonitor {
    #[inline(never)]
    fn data(&self) -> Option<String> {
        let mut packets = self.packets.lock_irqsave();
        if !packets.is_empty() {
            let json = json::to_string(packets.as_ref() as &Vec<CpuStatPacket>);
            packets.clear();
            return Some(json);
        }
        None
    }

    fn transpoter(&self) -> &'static mut super::KMonitorTransportChannel {
        unsafe { &mut KCPU_MONITOR_TRANSPORT_CHANNEL }
    }
}
