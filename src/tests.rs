use super::*;
use crate::app::{ContextMenu, visible_rows, filtered_rows, device_matches};
use crate::ui::base64;
use ratatui::layout::Rect;
use std::collections::HashSet;

#[test]
    fn menu_hit_test_excludes_borders() {
        let m = ContextMenu {
            rect: Rect::new(5, 5, 12, 5), // 3 content rows: y = 6,7,8
            items: vec![(String::new(), String::new(), String::new()); 3],
            hover: 0,
        };
        assert_eq!(m.item_at(6, 6), Some(0)); // first content row
        assert_eq!(m.item_at(6, 8), Some(2)); // last content row
        assert_eq!(m.item_at(6, 5), None); // top border
        assert_eq!(m.item_at(6, 9), None); // bottom border
        assert_eq!(m.item_at(5, 7), None); // left border
        assert_eq!(m.item_at(16, 7), None); // right border (x+width-1)
    }

    #[test]
    fn filter_keeps_ancestors_and_subtree() {
        // usb1(0) > 1-1(1), 1-2(1) > 1-2.1(2), 1-2.2(2)
        let rows = vec![(0, 0), (1, 1), (1, 2), (2, 3), (2, 4)];
        // match a leaf: keep it + its ancestor chain, nothing else
        let m = vec![false, false, false, true, false];
        assert_eq!(visible_rows(&rows, &m), vec![(0, 0), (1, 2), (2, 3)]);
        // match a hub: keep ancestor + whole subtree
        let m = vec![false, false, true, false, false];
        assert_eq!(visible_rows(&rows, &m), vec![(0, 0), (1, 2), (2, 3), (2, 4)]);
        // no match: empty
        assert!(visible_rows(&rows, &[false; 5]).is_empty());
    }

    #[test]
    fn device_matches_human_fields() {
        let d = usb::demo_scan(0)
            .into_iter()
            .find(|d| d.name == "1-3.1")
            .unwrap();
        assert!(device_matches(&d, "keychron")); // product / manufacturer
        assert!(device_matches(&d, "1-3.1")); // sysfs name
        assert!(device_matches(&d, "3434:0121")); // vid:pid
        assert!(!device_matches(&d, "logitech"));
    }

    #[test]
    fn filter_surfaces_collapsed_matches() {
        let devices = usb::demo_scan(0);
        let names =
            |rows: &[(usize, usize)]| rows.iter().map(|&(_, i)| devices[i].name.clone()).collect::<Vec<_>>();
        let mut collapsed = HashSet::new();
        collapsed.insert("1-3".to_string()); // hide the hub's children
        // idle: collapse hides the child
        assert!(!names(&filtered_rows(&devices, &collapsed, "")).contains(&"1-3.1".to_string()));
        // query surfaces it anyway, re-anchored under its collapsed parent
        let hit = names(&filtered_rows(&devices, &collapsed, "keychron"));
        assert!(hit.contains(&"1-3.1".to_string()));
        assert!(hit.contains(&"1-3".to_string()));
    }

    #[test]
    fn base64_matches_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"046d:c52b"), "MDQ2ZDpjNTJi");
    }
