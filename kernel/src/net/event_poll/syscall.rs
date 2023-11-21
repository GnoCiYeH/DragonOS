use core::ops::Add;

use crate::{
    arch::ipc::signal::SigSet,
    filesystem::vfs::file::FileMode,
    ipc::signal::set_current_sig_blocked,
    mm::{verify_area, VirtAddr},
    syscall::{user_access::UserBufferReader, Syscall, SystemError},
    time::TimeSpec,
};

use super::{EPollCtlOption, EPollEvent, EventPoll};

impl Syscall {
    pub fn epoll_create(max_size: i32) -> Result<usize, SystemError> {
        if max_size < 0 {
            return Err(SystemError::EINVAL);
        }

        return EventPoll::do_create_epoll(FileMode::empty());
    }

    pub fn epoll_create1(flag: usize) -> Result<usize, SystemError> {
        let flags = FileMode::from_bits_truncate(flag as u32);

        let ret = EventPoll::do_create_epoll(flags);
        ret
    }

    pub fn epoll_wait(
        epfd: i32,
        events: VirtAddr,
        max_events: i32,
        timeout: i32,
    ) -> Result<usize, SystemError> {
        if max_events <= 0 || max_events as u32 > EventPoll::EP_MAX_EVENTS {
            return Err(SystemError::EINVAL);
        }

        let mut timespec = None;
        if timeout == 0 {
            timespec = Some(TimeSpec::new(0, 0));
        }

        if timeout > 0 {
            let sec: i64 = timeout as i64 / 1000;
            let nsec: i64 = 1000000 * (timeout as i64 % 1000);

            timespec = Some(TimeSpec::new(sec, nsec))
        }

        // 因为C中的epoll_event大小为12字节,而在rust中为16字节
        verify_area(events, 12 * max_events as usize)?;

        return EventPoll::do_epoll_wait(epfd, events, max_events, timespec);
    }

    pub fn epoll_ctl(epfd: i32, op: usize, fd: i32, event: VirtAddr) -> Result<usize, SystemError> {
        let op = EPollCtlOption::from_op_num(op)?;
        let mut epds = EPollEvent::default();
        if op != EPollCtlOption::EpollCtlDel {
            // 不为EpollCtlDel时不允许传入空指针
            if event.is_null() {
                return Err(SystemError::EFAULT);
            }

            // 还是一样的问题，C标准的epoll_event大小为12字节，而内核实现的epoll_event内存对齐后为16字节
            // 这样分别拷贝其实和整体拷贝差别不大，内核使用内存对其版本甚至可能提升性能
            let ev_reader =
                UserBufferReader::new(event.as_ptr::<u32>(), core::mem::size_of::<u32>(), true)?;
            let events = ev_reader.read_one_from_user::<u32>(0)?;
            let event = event.add(core::mem::size_of::<u32>());
            let data_reader =
                UserBufferReader::new(event.as_ptr::<u64>(), core::mem::size_of::<u64>(), true)?;
            let data = data_reader.read_one_from_user::<u64>(0)?;

            epds = EPollEvent {
                events: *events,
                data: *data,
            };
        }

        return EventPoll::do_epoll_ctl(epfd, op, fd, &mut epds, false);
    }

    pub fn epoll_pwait(
        epfd: i32,
        epoll_event: VirtAddr,
        max_events: i32,
        timespec: i32,
        sigmask: VirtAddr,
    ) -> Result<usize, SystemError> {
        if sigmask.is_null() {
            return Self::epoll_wait(epfd, epoll_event, max_events, timespec);
        }
        let reader = UserBufferReader::new(
            sigmask.as_ptr::<SigSet>(),
            core::mem::size_of::<SigSet>(),
            true,
        )?;

        let mut sigmask = reader.read_one_from_user::<SigSet>(0)?.clone();

        // 设置屏蔽的信号
        set_current_sig_blocked(&mut sigmask);

        let wait_ret = Self::epoll_wait(epfd, epoll_event, max_events, timespec);

        if wait_ret.is_err() && *wait_ret.as_ref().unwrap_err() != SystemError::EINTR {
            // TODO: 恢复信号?
        }
        wait_ret
    }
}
