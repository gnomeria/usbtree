use super::{Device, StorageNode, display_path};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    fs::OpenOptions,
    os::windows::{ffi::OsStringExt, fs::OpenOptionsExt, io::AsRawHandle},
    ptr,
};
use windows_sys::Win32::{
    Devices::DeviceAndDriverInstallation::*,
    Foundation::{ERROR_MORE_DATA, GetLastError, INVALID_HANDLE_VALUE},
    Storage::FileSystem::*,
    System::{IO::DeviceIoControl, Ioctl::*},
};

fn text(buf: &[u16]) -> &[u16] {
    &buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())]
}

fn device_number(path: &std::ffi::OsStr) -> Option<u32> {
    let file = OpenOptions::new().access_mode(0).open(path).ok()?;
    let mut number = STORAGE_DEVICE_NUMBER::default();
    let mut written = 0;
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            ptr::null(),
            0,
            (&mut number as *mut STORAGE_DEVICE_NUMBER).cast(),
            size_of_val(&number) as u32,
            &mut written,
            ptr::null_mut(),
        )
    };
    (ok != 0 && written as usize == size_of_val(&number) && number.DeviceType == FILE_DEVICE_DISK)
        .then_some(number.DeviceNumber)
}

fn instance_id(node: u32) -> Option<[u16; MAX_DEVICE_ID_LEN as usize]> {
    let mut id = [0; MAX_DEVICE_ID_LEN as usize];
    (unsafe { CM_Get_Device_IDW(node, id.as_mut_ptr(), MAX_DEVICE_ID_LEN, 0) } == CR_SUCCESS)
        .then_some(id)
}

fn usb_owner(mut node: u32, owners: &HashMap<String, usize>) -> Option<usize> {
    loop {
        let id = String::from_utf16_lossy(text(&instance_id(node)?)).to_ascii_uppercase();
        if let Some(&index) = owners.get(&id) {
            return Some(index);
        }
        // Composite USB interfaces have another USB devnode above them.
        if id.starts_with(r"USB\VID_") && !id.split('\\').nth(1)?.contains("&MI_") {
            return None;
        }
        let mut parent = 0;
        if unsafe { CM_Get_Parent(&mut parent, node, 0) } != CR_SUCCESS {
            return None;
        }
        node = parent;
    }
}

fn disk_number(node: u32) -> Option<u32> {
    let id = instance_id(node)?;
    let mut len = 0;
    let guid = &GUID_DEVINTERFACE_DISK;
    if unsafe { CM_Get_Device_Interface_List_SizeW(&mut len, guid, id.as_ptr(), 0) } != CR_SUCCESS
        || len < 2
    {
        return None;
    }
    let mut buffer = vec![0; len as usize];
    if unsafe { CM_Get_Device_Interface_ListW(guid, id.as_ptr(), buffer.as_mut_ptr(), len, 0) }
        != CR_SUCCESS
    {
        return None;
    }
    device_number(&OsString::from_wide(text(&buffer)))
}

fn mount_paths(volume: &[u16]) -> Vec<String> {
    let mut len = 0;
    let mut paths = vec![0; 260];
    loop {
        if unsafe {
            GetVolumePathNamesForVolumeNameW(
                volume.as_ptr(),
                paths.as_mut_ptr(),
                paths.len() as u32,
                &mut len,
            )
        } != 0
        {
            break;
        }
        if unsafe { GetLastError() } != ERROR_MORE_DATA || len as usize <= paths.len() {
            return Vec::new();
        }
        paths.resize(len as usize, 0);
    }
    let mut mounts: Vec<_> = paths
        .split(|&c| c == 0)
        .take_while(|p| !p.is_empty())
        .map(|p| display_path(&String::from_utf16_lossy(p)))
        .collect();
    mounts.sort();
    mounts.dedup();
    mounts
}

#[cfg(test)]
#[test]
fn system_volume_lists_its_drive_letter() {
    let drive = format!("{}\\", std::env::var("SystemDrive").unwrap());
    let root: Vec<u16> = drive.encode_utf16().chain(Some(0)).collect();
    let mut volume = [0; 128];
    assert_ne!(
        unsafe {
            GetVolumeNameForVolumeMountPointW(
                root.as_ptr(),
                volume.as_mut_ptr(),
                volume.len() as u32,
            )
        },
        0
    );
    assert!(
        mount_paths(&volume)
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&drive))
    );
}

pub(super) fn populate(devices: &mut [Device], owners: &HashMap<String, usize>) {
    let set = unsafe {
        SetupDiGetClassDevsW(
            &GUID_DEVCLASS_DISKDRIVE,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT,
        )
    };
    if set == -1 {
        return;
    }
    let mut disks = BTreeMap::new();
    let mut info = SP_DEVINFO_DATA {
        cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    };
    for i in 0.. {
        if unsafe { SetupDiEnumDeviceInfo(set, i, &mut info) } == 0 {
            break;
        }
        if let Some(index) = usb_owner(info.DevInst, owners)
            && let Some(number) = disk_number(info.DevInst)
        {
            disks.insert(number, (index, Vec::new()));
        }
    }
    unsafe { SetupDiDestroyDeviceInfoList(set) };
    if disks.is_empty() {
        return;
    }

    populate_volumes(&mut disks);
    for (number, (index, mut volumes)) in disks {
        let storage = &mut devices[index].storage;
        storage.push(StorageNode {
            path: format!(r"\\.\PhysicalDrive{number}"),
            mounts: Vec::new(),
        });
        volumes.sort_by(|a, b| a.path.cmp(&b.path));
        storage.append(&mut volumes);
    }
}

fn populate_volumes(disks: &mut BTreeMap<u32, (usize, Vec<StorageNode>)>) {
    // Volume GUID paths have fixed-size GUIDs. Opening a volume for a device
    // number requires stripping its trailing slash; mount enumeration needs it.
    let mut buf = [0; 128];
    let iter = unsafe { FindFirstVolumeW(buf.as_mut_ptr(), buf.len() as u32) };
    if iter == INVALID_HANDLE_VALUE {
        return;
    }
    loop {
        let volume = text(&buf);
        if let Some(raw) = volume.strip_suffix(&[b'\\' as u16])
            && let Some(number) = device_number(&OsString::from_wide(raw))
            && let Some((_, volumes)) = disks.get_mut(&number)
        {
            volumes.push(StorageNode {
                path: String::from_utf16_lossy(volume),
                mounts: mount_paths(&buf),
            });
        }
        if unsafe { FindNextVolumeW(iter, buf.as_mut_ptr(), buf.len() as u32) } == 0 {
            break;
        }
    }
    unsafe { FindVolumeClose(iter) };
}
