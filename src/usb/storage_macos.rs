use super::{Device, StorageNode, display_path};
use core_foundation::{
    base::{CFType, TCFType},
    string::CFString,
};
use io_kit_sys::*;
use std::{collections::HashMap, ffi::CStr, ptr};

// Each successful iterator, next, and parent call returns one owned send right.
struct Object(u32);
impl Drop for Object {
    fn drop(&mut self) {
        unsafe { IOObjectRelease(self.0) };
    }
}

fn usb_owner(mut entry: Object, owners: &HashMap<u64, usize>) -> Option<usize> {
    loop {
        let mut id = 0;
        if unsafe { IORegistryEntryGetRegistryEntryID(entry.0, &mut id) } == 0
            && let Some(&index) = owners.get(&id)
        {
            return Some(index);
        }
        if unsafe { IOObjectConformsTo(entry.0, c"IOUSBHostDevice".as_ptr()) } != 0 {
            return None;
        }
        let mut parent = 0;
        if unsafe { IORegistryEntryGetParentEntry(entry.0, c"IOService".as_ptr(), &mut parent) }
            != 0
        {
            return None;
        }
        entry = Object(parent);
    }
}

fn mounts() -> HashMap<String, Vec<String>> {
    let mut result = HashMap::<_, Vec<_>>::new();
    let count = unsafe { libc::getfsstat(ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    if count <= 0 {
        return result;
    }
    let mut entries = Vec::<libc::statfs>::with_capacity(count as usize + 1);
    let Ok(size) = (entries.capacity() * size_of::<libc::statfs>()).try_into() else {
        return result;
    };
    let filled = unsafe { libc::getfsstat(entries.as_mut_ptr(), size, libc::MNT_NOWAIT) };
    if filled < 0 {
        return result;
    }
    // getfsstat initialized at most capacity records in our own buffer.
    unsafe { entries.set_len((filled as usize).min(entries.capacity())) };
    for fs in entries {
        let source = unsafe { CStr::from_ptr(fs.f_mntfromname.as_ptr()) }.to_string_lossy();
        let mount = unsafe { CStr::from_ptr(fs.f_mntonname.as_ptr()) }.to_string_lossy();
        result
            .entry(source.into_owned())
            .or_default()
            .push(display_path(&mount));
    }
    result
}

#[cfg(test)]
#[test]
fn mount_snapshot_contains_root() {
    assert!(mounts().values().flatten().any(|path| path == "/"));
}

pub(super) fn populate(devices: &mut [Device], owners: &HashMap<u64, usize>) {
    let matching = unsafe { IOServiceMatching(c"IOMedia".as_ptr()) };
    let mut iter = 0;
    if matching.is_null() || unsafe { IOServiceGetMatchingServices(0, matching, &mut iter) } != 0 {
        return;
    }
    let iter = Object(iter);
    let mut mounts = mounts();
    let key = CFString::new("BSD Name");
    loop {
        let media = unsafe { IOIteratorNext(iter.0) };
        if media == 0 {
            break;
        }
        let media = Object(media);
        let property = unsafe {
            IORegistryEntryCreateCFProperty(media.0, key.as_concrete_TypeRef(), ptr::null(), 0)
        };
        if property.is_null() {
            continue;
        }
        let property = unsafe { CFType::wrap_under_create_rule(property) };
        if let Some(name) = property.downcast_into::<CFString>()
            && let Some(index) = usb_owner(media, owners)
        {
            let path = format!("/dev/{name}");
            devices[index].storage.push(StorageNode {
                mounts: mounts.remove(&path).unwrap_or_default(),
                path,
            });
        }
    }
    for device in devices {
        device
            .storage
            .sort_by_cached_key(|node| super::sort_key(&node.path));
        for node in &mut device.storage {
            node.mounts.sort();
            node.mounts.dedup();
        }
    }
}
