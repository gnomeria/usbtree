use super::*;

#[test]
    fn bus_key_strips_device_function() {
        let d = PciDevice {
            addr: "0000:00:1f.2".into(),
            vid: 0,
            pid: 0,
            class: 0,
            subclass: 0,
            prog_if: 0,
            revision: 0,
            subsystem: None,
            driver: None,
            link: None,
            numa_node: None,
            iommu_group: None,
            power_state: None,
        };
        assert_eq!(d.bus(), "0000:00");
    }
