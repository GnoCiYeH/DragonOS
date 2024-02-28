use alloc::sync::Arc;
use system_error::SystemError;

use super::{
    tty_core::{TtyCore, TtyCoreData, TtyFlag},
    tty_driver::{TtyDriver, TtyDriverSubType},
    tty_port::{DefaultTtyPort, TtyPort},
};

#[cfg(feature = "legacy_ptys")]
pub mod legacy_pty;
#[cfg(feature = "unix98ptys")]
pub mod unix98pty;

pub struct PtyCommon;

impl PtyCommon {
    pub fn pty_common_install(
        driver: Arc<TtyDriver>,
        tty: Arc<TtyCore>,
        legacy: bool,
    ) -> Result<(), SystemError> {
        let core = tty.core();
        let other_tty = TtyCore::new(driver.other_pty_driver().unwrap(), core.index());
        let port0: Arc<dyn TtyPort> = Arc::new(DefaultTtyPort::new());
        let port1: Arc<dyn TtyPort> = Arc::new(DefaultTtyPort::new());

        let o_core = other_tty.core();

        if legacy {
            core.init_termios();
            o_core.init_termios();

            driver
                .other_pty_driver()
                .unwrap()
                .ttys()
                .insert(core.index(), other_tty.clone());
            driver.ttys().insert(core.index(), tty.clone());
        } else {
            *core.termios_write() = driver.init_termios();
            *o_core.termios_write() = driver.other_pty_driver().unwrap().init_termios();
        }

        core.set_link(Arc::downgrade(&other_tty));
        o_core.set_link(Arc::downgrade(&tty));

        port0.setup_internal_tty(Arc::downgrade(&tty));
        port1.setup_internal_tty(Arc::downgrade(&other_tty));
        other_tty.set_port(port0);
        tty.set_port(port1);

        core.add_count();
        o_core.add_count();

        Ok(())
    }

    pub fn pty_common_open(core: &TtyCoreData) -> Result<(), SystemError> {
        if core.link().upgrade().is_none() {
            return Err(SystemError::ENODEV);
        }

        let link = core.link().upgrade().unwrap();
        let link_core = link.core();

        if core.flags().contains(TtyFlag::OTHER_CLOSED) {
            core.flags_write().insert(TtyFlag::IO_ERROR);
            return Err(SystemError::EIO);
        }

        if link_core.flags().contains(TtyFlag::PTY_LOCK) {
            core.flags_write().insert(TtyFlag::IO_ERROR);
            return Err(SystemError::EIO);
        }

        if core.driver().tty_driver_sub_type() == TtyDriverSubType::PtySlave
            && link_core.count() != 1
        {
            // 只能有一个master，如果当前为slave，则link的count必须为1
            core.flags_write().insert(TtyFlag::IO_ERROR);
            return Err(SystemError::EIO);
        }

        core.flags_write().remove(TtyFlag::IO_ERROR);
        link_core.flags_write().remove(TtyFlag::OTHER_CLOSED);
        core.flags_write().insert(TtyFlag::THROTTLED);

        Ok(())
    }
}
