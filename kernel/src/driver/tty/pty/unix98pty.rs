use alloc::sync::Arc;
use system_error::SystemError;

use crate::{
    driver::tty::{
        termios::Termios,
        tty_core::{TtyCore, TtyCoreData, TtyPacketStatus},
        tty_driver::{TtyDriver, TtyDriverSubType, TtyOperation},
        tty_port::{DefaultTtyPort, TtyPort},
    },
    net::event_poll::EPollEventType,
};

use super::PtyCommon;

#[derive(Debug)]
pub struct Unix98PtyDriverInner;

impl TtyOperation for Unix98PtyDriverInner {
    fn install(&self, driver: Arc<TtyDriver>, tty: Arc<TtyCore>) -> Result<(), SystemError> {
        PtyCommon::pty_common_install(driver, tty, false)
    }

    fn open(&self, tty: &TtyCoreData) -> Result<(), SystemError> {
        PtyCommon::pty_common_open(tty)
    }

    fn write(&self, tty: &TtyCoreData, buf: &[u8], nr: usize) -> Result<usize, SystemError> {
        let to = tty.checked_link()?;

        if nr == 0 || tty.flow_irqsave().stopped {
            return Ok(0);
        }

        to.core().port().unwrap().receive_buf(buf, &[], nr)
    }

    fn write_room(&self, tty: &TtyCoreData) -> usize {
        // TODO 暂时
        if tty.flow_irqsave().stopped {
            return 0;
        }

        8192
    }

    fn flush_buffer(&self, tty: &TtyCoreData) -> Result<(), SystemError> {
        let to = tty.checked_link()?;

        let mut ctrl = to.core().contorl_info_irqsave();
        ctrl.pktstatus.insert(TtyPacketStatus::TIOCPKT_FLUSHWRITE);

        to.core().read_wq().wakeup_all();

        Ok(())
    }

    fn ioctl(&self, tty: Arc<TtyCore>, cmd: u32, arg: usize) -> Result<(), SystemError> {
        let core = tty.core();
        if core.driver().tty_driver_sub_type() != TtyDriverSubType::PtyMaster {
            return Err(SystemError::ENOSYS);
        }
        todo!()
    }

    fn set_termios(&self, tty: Arc<TtyCore>, _old_termios: Termios) -> Result<(), SystemError> {
        let core = tty.core();
        if core.driver().tty_driver_sub_type() != TtyDriverSubType::PtySlave {
            return Err(SystemError::ENOSYS);
        }
        todo!()
    }

    fn start(&self, core: &TtyCoreData) -> Result<(), SystemError> {
        if core.driver().tty_driver_sub_type() != TtyDriverSubType::PtySlave {
            return Err(SystemError::ENOSYS);
        }

        let link = core.checked_link()?;

        let mut ctrl = core.contorl_info_irqsave();
        ctrl.pktstatus.remove(TtyPacketStatus::TIOCPKT_STOP);
        ctrl.pktstatus.insert(TtyPacketStatus::TIOCPKT_START);

        link.core()
            .read_wq()
            .wakeup(EPollEventType::EPOLLIN.bits() as u64);

        Ok(())
    }

    fn stop(&self, core: &TtyCoreData) -> Result<(), SystemError> {
        if core.driver().tty_driver_sub_type() != TtyDriverSubType::PtySlave {
            return Err(SystemError::ENOSYS);
        }

        let link = core.checked_link()?;

        let mut ctrl = core.contorl_info_irqsave();
        ctrl.pktstatus.remove(TtyPacketStatus::TIOCPKT_START);
        ctrl.pktstatus.insert(TtyPacketStatus::TIOCPKT_STOP);

        link.core()
            .read_wq()
            .wakeup(EPollEventType::EPOLLIN.bits() as u64);

        Ok(())
    }

    fn flush_chars(&self, _tty: &TtyCoreData) {
        // 不做处理
    }
}
