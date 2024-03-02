use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use system_error::SystemError;
use unified_init::macros::unified_init;

use crate::{
    driver::base::device::{
        device_number::{DeviceNumber, Major},
        device_register, Device, IdTable,
    },
    filesystem::{devfs::devfs_register, vfs::IndexNode},
    init::initcall::INITCALL_DEVICE,
    mm::VirtAddr,
    syscall::user_access::{UserBufferReader, UserBufferWriter},
};

use self::unix98pty::{Unix98PtyDriverInner, NR_UNIX98_PTY_MAX};

use super::{
    termios::{ControlMode, InputMode, LocalMode, OutputMode, TTY_STD_TERMIOS},
    tty_core::{TtyCore, TtyCoreData, TtyFlag, TtyPacketStatus},
    tty_device::{TtyDevice, TtyType},
    tty_driver::{TtyDriver, TtyDriverManager, TtyDriverSubType, TtyDriverType},
    tty_port::{DefaultTtyPort, TtyPort},
};

#[cfg(feature = "legacy_ptys")]
pub mod legacy_pty;
#[cfg(feature = "unix98ptys")]
pub mod unix98pty;

lazy_static! {
    pub static ref PTM_DRIVER: Arc<TtyDriver> = {
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
        TtyDriverManager::tty_register_driver(ptm_driver).unwrap()
    };
    pub static ref PTS_DRIVER: Arc<TtyDriver> = {
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
        TtyDriverManager::tty_register_driver(pts_driver).unwrap()
    };
}

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

        core.set_link(Some(other_tty.clone()));
        o_core.set_link(Some(tty.clone()));

        port0.setup_internal_tty(Arc::downgrade(&tty));
        port1.setup_internal_tty(Arc::downgrade(&other_tty));
        other_tty.set_port(port0);
        tty.set_port(port1);

        core.add_count();
        o_core.add_count();

        Ok(())
    }

    pub fn pty_common_open(core: &TtyCoreData) -> Result<(), SystemError> {
        if core.link().is_none() {
            return Err(SystemError::ENODEV);
        }

        let link = core.link().unwrap();
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

    pub fn pty_set_lock(tty: &TtyCoreData, arg: VirtAddr) -> Result<(), SystemError> {
        let user_reader =
            UserBufferReader::new(arg.as_ptr::<i32>(), core::mem::size_of::<i32>(), true)?;

        if *user_reader.read_one_from_user::<i32>(0)? != 0 {
            tty.flags_write().insert(TtyFlag::PTY_LOCK);
        } else {
            tty.flags_write().remove(TtyFlag::PTY_LOCK);
        }

        Ok(())
    }

    pub fn pty_get_lock(tty: &TtyCoreData, arg: VirtAddr) -> Result<(), SystemError> {
        let mut user_writer =
            UserBufferWriter::new(arg.as_ptr::<i32>(), core::mem::size_of::<i32>(), true)?;

        user_writer.copy_one_to_user(&tty.flags().contains(TtyFlag::PTY_LOCK), 0)?;
        Ok(())
    }

    pub fn pty_set_packet_mode(tty: &TtyCoreData, arg: VirtAddr) -> Result<(), SystemError> {
        let user_reader =
            UserBufferReader::new(arg.as_ptr::<i32>(), core::mem::size_of::<i32>(), true)?;

        let mut ctrl = tty.contorl_info_irqsave();
        if *user_reader.read_one_from_user::<i32>(0)? != 0 {
            if !ctrl.packet {
                tty.link().unwrap().core().contorl_info_irqsave().pktstatus =
                    TtyPacketStatus::empty();
                ctrl.packet = true;
            }
        } else {
            ctrl.packet = false;
        }

        Ok(())
    }

    pub fn pty_get_packet_mode(tty: &TtyCoreData, arg: VirtAddr) -> Result<(), SystemError> {
        let mut user_writer =
            UserBufferWriter::new(arg.as_ptr::<i32>(), core::mem::size_of::<i32>(), true)?;

        user_writer.copy_one_to_user(&tty.contorl_info_irqsave().packet, 0)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn legacy_pty_init() -> Result<(), SystemError> {
        Ok(())
    }

    pub fn unix98pty_init() -> Result<(), SystemError> {
        PTM_DRIVER.set_other_pty_driver(PTS_DRIVER.clone());
        PTS_DRIVER.set_other_pty_driver(PTM_DRIVER.clone());

        let idt = IdTable::new(
            String::from("ptmx"),
            Some(DeviceNumber::new(Major::TTYAUX_MAJOR, 2)),
        );
        let ptmx_dev = TtyDevice::new("ptmx".to_string(), idt.clone(), TtyType::Pty);

        ptmx_dev.inner_write().metadata_mut().raw_dev = idt.device_number();
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
