use super::{Device, StorageNode};
use std::{fs, path::Path};

/// The interface directory sits directly under its USB device. Match the
/// nearest such pair so a disk behind a hub belongs only to the leaf device.
pub(super) fn usb_owner(path: &Path) -> Option<&str> {
    path.ancestors().find_map(|p| {
        let (device, _) = p.file_name()?.to_str()?.split_once(':')?;
        (p.parent()?.file_name()? == device).then_some(device)
    })
}

fn sorted_entries(path: &Path) -> Vec<fs::DirEntry> {
    let mut entries: Vec<_> = fs::read_dir(path).into_iter().flatten().flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    entries
}

pub(super) fn populate(devices: &mut [Device]) {
    let mounts = std::cell::OnceCell::new();
    for entry in sorted_entries(Path::new("/sys/block")) {
        let Ok(target) = fs::read_link(entry.path()) else {
            continue;
        };
        let Some(owner) = usb_owner(&target) else {
            continue;
        };
        let Some(device) = devices.iter_mut().find(|d| d.name == owner) else {
            continue;
        };
        let table = mounts.get_or_init(|| {
            let bytes = fs::read("/proc/self/mountinfo").unwrap_or_default();
            String::from_utf8_lossy(&bytes).into_owned()
        });
        add_disk(&entry.path(), table, &mut device.storage);
    }
}

fn add_disk(disk: &Path, table: &str, nodes: &mut Vec<StorageNode>) {
    add_node(disk, table, nodes);
    for partition in super::partitions(disk) {
        add_node(&disk.join(partition), table, nodes);
    }
}

fn add_node(sysfs: &Path, table: &str, nodes: &mut Vec<StorageNode>) {
    let Some(name) = sysfs.file_name() else {
        return;
    };
    let path = format!("/dev/{}", name.to_string_lossy());
    if nodes.iter().any(|n| n.path == path) {
        return;
    }
    let number = fs::read_to_string(sysfs.join("dev")).unwrap_or_default();
    let mapper = if path.starts_with("/dev/dm-") {
        fs::read_to_string(sysfs.join("dm/name")).ok().map(|name| {
            // mountinfo escapes whitespace and backslashes in the source field.
            let name = name
                .strip_suffix('\n')
                .unwrap_or(&name)
                .replace('\\', r"\134")
                .replace(' ', r"\040")
                .replace('\t', r"\011")
                .replace('\n', r"\012");
            format!("/dev/mapper/{name}")
        })
    } else {
        None
    };
    nodes.push(StorageNode {
        mounts: mount_points(table, number.trim(), &path, mapper.as_deref()),
        path,
    });
    for holder in sorted_entries(&sysfs.join("holders")) {
        add_node(&holder.path(), table, nodes);
    }
}

fn mount_points(table: &str, number: &str, path: &str, mapper: Option<&str>) -> Vec<String> {
    let mut points = Vec::new();
    for line in table.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let mut fields = left.split_ascii_whitespace();
        // Some filesystems (including btrfs) use a synthetic major:minor.
        let source = right.split_ascii_whitespace().nth(1);
        if fields.nth(2) != Some(number) && !source.is_some_and(|s| s == path || Some(s) == mapper)
        {
            continue;
        }
        if let Some(point) = fields.nth(1) {
            // Decode spaces and backslashes once. Keep control bytes escaped
            // so hostile volume names cannot write terminal control sequences.
            let point = point.replace(r"\040", " ").replace(r"\134", "\\");
            points.push(super::display_path(&point));
        }
    }
    points.sort();
    points.dedup();
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_matches_leaf_port_only() {
        assert_eq!(
            usb_owner(Path::new(
                "../../usb2/2-1/2-1.3/2-1.3:1.0/host2/block/sda/sda1"
            )),
            Some("2-1.3")
        );
        assert_eq!(
            usb_owner(Path::new(
                "../../devices/pci0000:00/0000:02:00.0/nvme/nvme0n1"
            )),
            None
        );
    }

    #[test]
    fn storage_follows_stacked_holders_to_the_mounted_volume() {
        let root = std::env::temp_dir().join(format!("usbtree-holders-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let disk = root.join("sda");
        let part = disk.join("sda1");
        let crypt = part.join("holders/dm-0");
        let volume = crypt.join("holders/dm-1");
        fs::create_dir_all(&volume).unwrap();
        fs::write(part.join("partition"), "1\n").unwrap();
        for number in [10, 2] {
            let other = disk.join(format!("sda{number}"));
            fs::create_dir(&other).unwrap();
            fs::write(other.join("partition"), number.to_string()).unwrap();
        }
        fs::write(volume.join("dev"), "253:1\n").unwrap();
        fs::create_dir(volume.join("dm")).unwrap();
        fs::write(volume.join("dm/name"), "vg-data vault\n").unwrap();
        let table = r"1 0 0:44 / /media/Vault rw - btrfs /dev/mapper/vg-data\040vault rw";
        let mut nodes = Vec::new();
        add_disk(&disk, table, &mut nodes);
        add_node(&crypt, table, &mut nodes);
        fs::remove_dir_all(root).unwrap();
        assert_eq!(
            nodes.iter().map(|n| n.path.as_str()).collect::<Vec<_>>(),
            [
                "/dev/sda",
                "/dev/sda1",
                "/dev/dm-0",
                "/dev/dm-1",
                "/dev/sda2",
                "/dev/sda10"
            ]
        );
        assert!(nodes[0].mounts.is_empty());
        assert_eq!(nodes[3].mounts, ["/media/Vault"]);
    }

    #[test]
    fn storage_reports_all_direct_mounts() {
        let table = r"1 0 8:1 / /media/My\040Disk rw - ext4 /dev/disk/by-label/test rw
2 0 8:1 /sub /mnt/copy rw - ext4 /dev/sda1 rw
3 0 0:44 / /mnt/literal\134040 rw - btrfs /dev/sdb rw";
        assert_eq!(
            mount_points(table, "8:1", "/dev/sda1", None),
            ["/media/My Disk", "/mnt/copy"]
        );
        assert_eq!(
            mount_points(table, "8:16", "/dev/sdb", None),
            [r"/mnt/literal\040"]
        );
        assert!(mount_points(table, "8:2", "/dev/sda2", None).is_empty());
    }
}
