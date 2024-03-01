use core::sync::atomic::{AtomicU32, Ordering};

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
    vec::Vec,
};
use bitmap::{traits::BitMapOps, StaticBitmap};
use ida::IdAllocator;
use system_error::SystemError;

use crate::{
    driver::{base::device::IdTable, tty::tty_device::TtyDevice},
    filesystem::vfs::FileType,
    libs::{rwlock::RwLock, spinlock::SpinLock},
};

use super::vfs::{FileSystem, FsInfo, IndexNode};

const DEV_PTYFS_MAX_NAMELEN: usize = 16;

const PTY_NR_LIMIT: usize = 4096;

#[derive(Debug)]
pub struct DevPtsFs {
    /// 根节点
    root_inode: Arc<LockedDevPtsFSInode>,
    pts_ida: IdAllocator,
    pts_count: AtomicU32,
}

impl FileSystem for DevPtsFs {
    fn root_inode(&self) -> Arc<dyn IndexNode> {
        self.root_inode.clone()
    }

    fn info(&self) -> super::vfs::FsInfo {
        return FsInfo {
            blk_dev_id: 0,
            max_name_len: DEV_PTYFS_MAX_NAMELEN,
        };
    }

    fn as_any_ref(&self) -> &dyn core::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct LockedDevPtsFSInode {
    inner: SpinLock<PtsDevInode>,
}

#[derive(Debug)]
pub struct PtsDevInode {
    fs: Weak<DevPtsFs>,
    children: Option<BTreeMap<String, Arc<TtyDevice>>>,
}

impl PtsDevInode {
    pub fn children_unchecked(&self) -> &BTreeMap<String, Arc<TtyDevice>> {
        self.children.as_ref().unwrap()
    }

    pub fn children_unchecked_mut(&mut self) -> &mut BTreeMap<String, Arc<TtyDevice>> {
        self.children.as_mut().unwrap()
    }
}

impl IndexNode for LockedDevPtsFSInode {
    fn read_at(
        &self,
        offset: usize,
        len: usize,
        buf: &mut [u8],
        _data: &mut super::vfs::FilePrivateData,
    ) -> Result<usize, system_error::SystemError> {
        todo!()
    }

    fn write_at(
        &self,
        offset: usize,
        len: usize,
        buf: &[u8],
        _data: &mut super::vfs::FilePrivateData,
    ) -> Result<usize, system_error::SystemError> {
        todo!()
    }

    fn fs(&self) -> alloc::sync::Arc<dyn super::vfs::FileSystem> {
        self.inner.lock().fs.upgrade().unwrap()
    }

    fn as_any_ref(&self) -> &dyn core::any::Any {
        todo!()
    }

    fn list(&self) -> Result<alloc::vec::Vec<alloc::string::String>, system_error::SystemError> {
        let info = self.metadata()?;
        if info.file_type != FileType::Dir {
            return Err(SystemError::ENOTDIR);
        }

        let mut keys: Vec<String> = Vec::new();
        keys.push(String::from("."));
        keys.push(String::from(".."));
        keys.append(
            &mut self
                .inner
                .lock()
                .children_unchecked()
                .keys()
                .cloned()
                .collect(),
        );

        return Ok(keys);
    }

    fn create_with_data(
        &self,
        name: &str,
        file_type: FileType,
        _mode: super::vfs::syscall::ModeType,
        _data: usize,
    ) -> Result<Arc<dyn IndexNode>, SystemError> {
        if file_type != FileType::CharDevice {
            return Err(SystemError::ENOSYS);
        }

        let mut guard = self.inner.lock();

        if guard.children_unchecked_mut().contains_key(name) {
            return Err(SystemError::EEXIST);
        }

        let fs = guard.fs.upgrade().unwrap();

        let result = TtyDevice::new(name.to_string(), IdTable::new(name.to_string(), None));

        guard
            .children_unchecked_mut()
            .insert(name.to_string(), result.clone());

        fs.pts_count.fetch_add(1, Ordering::SeqCst);

        Ok(result)
    }
}
