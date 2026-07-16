use super::*;

fn dev(name: &str, vid: u16, pid: u16, class: u8, ifaces: &[u8]) -> Device {
        Device {
            name: name.into(),
            vid,
            pid,
            manufacturer: None,
            product: None,
            serial: None,
            speed: "480".into(),
            usb_version: 0x0200,
            device_version: 0x0100,
            class,
            subclass: 0,
            protocol: 0,
            iface_classes: ifaces.to_vec(),
            config_attributes: None,
            devnum: 0,
            max_power_ma: None,
            interfaces: Vec::new(),
        }
    }

    #[test]
    fn max_power_parses_sysfs() {
        assert_eq!(parse_max_power("500mA\n"), Some(500));
        assert_eq!(parse_max_power("0mA"), Some(0));
        assert_eq!(parse_max_power(""), None);
    }

    #[test]
    fn parses_interfaces_and_endpoints() {
        // config(9) + interface(9, mass-storage BOT) + endpoint(7, bulk IN 512)
        #[rustfmt::skip]
        let config = [
            9, 2, 25, 0, 1, 1, 0, 0x80, 50,
            9, 4, 0, 0, 1, 0x08, 0x06, 0x50, 0,
            7, 5, 0x81, 0x02, 0x00, 0x02, 0,
        ];
        let (attrs, ifaces) = parse_config(&config);
        assert_eq!(attrs, Some(0x80)); // bmAttributes byte from the config descriptor
        assert_eq!(ifaces.len(), 1);
        let i = &ifaces[0];
        assert_eq!((i.class, i.subclass, i.protocol), (0x08, 0x06, 0x50));
        assert_eq!(i.endpoints.len(), 1);
        let e = &i.endpoints[0];
        assert_eq!(e.address, 0x81);
        assert!(e.input);
        assert_eq!(e.transfer, 2); // bulk
        assert_eq!(e.max_packet, 512);

        // Linux sysfs blob leads with an 18-byte device descriptor; skip it.
        let mut sysfs = vec![18, 1];
        sysfs.extend(std::iter::repeat_n(0, 16));
        sysfs.extend_from_slice(&config);
        assert_eq!(parse_config(&sysfs), (attrs, ifaces));
    }

    #[test]
    fn tree_nests_and_collapses() {
        let devices = vec![
            dev("usb1", 0, 0, 0x09, &[]),
            dev("1-1", 0x046d, 0xc52b, 0x00, &[0x03]),
            dev("1-1.4", 0x05e3, 0x0610, 0x09, &[]),
        ];
        let rows = flatten(&devices, &HashSet::new());
        assert_eq!(
            rows,
            vec![(0, 0), (1, 1), (2, 2)],
            "usb1 > 1-1 > 1-1.4 by depth"
        );
        assert_eq!(devices[1].parent_name().as_deref(), Some("usb1"));
        assert_eq!(devices[2].parent_name().as_deref(), Some("1-1"));
        assert_eq!(
            devices[1].effective_class(),
            0x03,
            "composite falls back to interface class"
        );

        // collapsing usb1 hides the whole subtree, no orphan resurfacing
        let collapsed = HashSet::from(["usb1".to_string()]);
        assert_eq!(flatten(&devices, &collapsed).len(), 1);
        assert_eq!(child_count(&devices, "usb1"), 1);
    }

    #[test]
    fn usb_ids_parse() {
        let sample = "# comment\n\
046d  Logitech, Inc.\n\
\tc52b  Unifying Receiver\n\
\t\t01  weird interface line\n\
07fd  Mark of the Unicorn\n\
\n\
C 03  Human Interface Device\n\
\t01  Boot Interface Subclass\n";
        let db = parse_usb_ids(sample);
        assert_eq!(db.vendors[&0x046d], "Logitech, Inc.");
        assert_eq!(db.products[&(0x046d, 0xc52b)], "Unifying Receiver");
        assert_eq!(db.vendors.len(), 2, "class section must not become a vendor");
        assert_eq!(db.products.len(), 1, "class subclass lines must be ignored");
    }

    #[test]
    fn overrides_parse() {
        let map = parse_overrides("# comment\n07fd:000b MOTU M2 Audio Interface\n\nbad line\n");
        assert_eq!(map.len(), 1);
        assert_eq!(map[&(0x07fd, 0x000b)], "MOTU M2 Audio Interface");
    }

    #[test]
    fn misc_class_resolves_via_interfaces() {
        // Misc/IAD at device level, like the MOTU M2
        let d = dev("1-9", 0x07fd, 0x000b, 0xef, &[0xff, 0x01, 0x01]);
        assert_eq!(d.effective_class(), 0x01);
        assert_eq!(d.class_name(), "Audio");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn block_symlink_slash_anchored() {
        let t = "../devices/pci0000:00/0000:00:14.0/usb2/2-1/2-1:1.0/host6/target6:0:0/6:0:0:0/block/sda";
        assert!(under_usb_device(t, "2-1"));
        assert!(!under_usb_device(t, "12-1")); // sibling id, not a substring match
        assert!(!under_usb_device(t, "2-2")); // different port
        // ejecting a hub grabs disks on its downstream ports (path passes through it)
        let child = "../devices/pci0000:00/usb2/2-1/2-1.4/2-1.4:1.0/host6/block/sdb";
        assert!(under_usb_device(child, "2-1"));
        assert!(under_usb_device(child, "2-1.4"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn busy_vs_not_mounted() {
        assert!(is_busy("Error unmounting /dev/sda1: target is busy"));
        assert!(is_busy("Device or resource busy"));
        assert!(is_busy("Object /dev/sda1 is in use"));
        // benign: an idle partition just isn't mounted — must not block power-off
        assert!(!is_busy("Error unmounting: not mounted"));
    }

    #[test]
    fn diff_detects_hotplug() {
        let before = vec![dev("usb1", 0, 0, 0x09, &[]), dev("1-1", 1, 2, 0x03, &[])];
        let mut after = before.clone();
        after.push(dev("1-2", 0x0781, 0x5567, 0x08, &[]));
        let (added, removed) = diff(&before, &after);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].name, "1-2");
        assert!(removed.is_empty());
        let (added, removed) = diff(&after, &before);
        assert_eq!(removed.len(), 1);
        assert!(added.is_empty());
    }

    #[test]
    fn bus_label_passes_numeric_synths_windows_paths() {
        let mut n = 1;
        assert_eq!(bus_label("001", &mut n), "1"); // Linux
        assert_eq!(bus_label("42", &mut n), "42"); // macOS
        // Windows opaque paths -> sequential synth, distinct paths distinct nums
        assert_eq!(bus_label("PCIROOT(0)#PCI(0201)#PCI(0000)#USBROOT(0)", &mut n), "1");
        assert_eq!(bus_label("PCIROOT(0)#PCI(0201)#PCI(0000)#USBROOT(1)", &mut n), "2");
        assert_eq!(n, 3);
        // synth names still link child -> hub
        assert_eq!(dev("1-16", 0, 0, 0x03, &[]).parent_name().as_deref(), Some("usb1"));
    }

    #[test]
    fn version_compare() {
        assert!(is_newer("0.0.2", "0.0.1"));
        assert!(is_newer("0.1.0", "0.0.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.0.1", "0.0.1"));
        assert!(!is_newer("0.0.1", "0.0.2"));
        assert!(is_newer("0.0.2", "0.0.1")); // shorter/longer parts default to 0
    }
