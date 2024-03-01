use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use system_error::SystemError;
use unified_init::macros::unified_init;

use crate::{
    driver::base::{
        char::{CharDevOps, CharDevice},
        device::{
            device_number::{DeviceNumber, Major},
            device_register, IdTable,
        },
    },
    filesystem::devfs::devfs_register,
    init::initcall::INITCALL_DEVICE,
};

use self::unix98pty::{Unix98PtyDriverInner, NR_UNIX98_PTY_MAX};

use super::{
    termios::{ControlMode, InputMode, LocalMode, OutputMode, TTY_STD_TERMIOS},
    tty_core::{TtyCore, TtyCoreData, TtyFlag},
    tty_device::TtyDevice,
    tty_driver::{TtyDriver, TtyDriverManager, TtyDriverSubType, TtyDriverType, TTY_DRIVERS},
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

    #[allow(dead_code)]
    pub fn legacy_pty_init() -> Result<(), SystemError> {
        Ok(())
    }

    pub fn unix98pty_init() -> Result<(), SystemError> {
        let mut ptm_driver = TtyDriver::new(
            NR_UNIX98_PTY_MAX,
            "ptm",
            0,
            Major::UNIX98_PTY_MASTER_MAJOR,
            0,
            TtyDriverType::Pty,
            TTY_STD_TERMIOS.clone(),
            Arc::new(Unix98PtyDriverInner::new()),
        );

        ptm_driver.set_subtype(TtyDriverSubType::PtyMaster);
        let term = ptm_driver.init_termios_mut();
        term.input_mode = InputMode::empty();
        term.output_mode = OutputMode::empty();
        term.control_mode = ControlMode::B38400 | ControlMode::CS8 | ControlMode::CREAD;
        term.local_mode = LocalMode::empty();
        term.input_speed = 38400;
        term.output_speed = 38400;

        let mut pts_driver = TtyDriver::new(
            NR_UNIX98_PTY_MAX,
            "pts",
            0,
            Major::UNIX98_PTY_SLAVE_MAJOR,
            0,
            TtyDriverType::Pty,
            TTY_STD_TERMIOS.clone(),
            Arc::new(Unix98PtyDriverInner::new()),
        );

        pts_driver.set_subtype(TtyDriverSubType::PtySlave);
        let term = pts_driver.init_termios_mut();
        term.input_mode = InputMode::empty();
        term.output_mode = OutputMode::empty();
        term.control_mode = ControlMode::B38400 | ControlMode::CS8 | ControlMode::CREAD;
        term.local_mode = LocalMode::empty();
        term.input_speed = 38400;
        term.output_speed = 38400;

        let ptm = TtyDriverManager::tty_register_driver(ptm_driver)?;
        let pts = TtyDriverManager::tty_register_driver(pts_driver)?;

        ptm.set_other_pty_driver(pts.clone());
        pts.set_other_pty_driver(ptm.clone());

        let mut driver_set = TTY_DRIVERS.lock();
        driver_set.push(ptm);
        driver_set.push(pts);

        let ptmx_dev = TtyDevice::new(
            "ptmx".to_string(),
            IdTable::new(
                String::from("ptmx"),
                Some(DeviceNumber::new(Major::TTYAUX_MAJOR, 2)),
            ),
        );

        device_register(ptmx_dev.clone())?;
        devfs_register("ptmx", ptmx_dev)?;

        Ok(())
    }
}

#[unified_init(INITCALL_DEVICE)]
#[inline(never)]
pub fn pty_init() -> Result<(), SystemError> {
    #[cfg(feature = "unix98ptys")]
    return PtyCommon::unix98pty_init();
    #[cfg(feature = "legacy_ptys")]
    return PtyCommon::legacy_pty_init();
}
