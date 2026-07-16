use super::*;

#[test]
    fn usbmon_parse() {
        // completed interrupt IN: counted
        let l = "ffff9c1a 3003687252 C Ii:1:002:1 0:8 8 = 1f000000";
        assert_eq!(parse_usbmon(l), Some(((1, 2), 8)));
        // submitted bulk OUT: counted
        let l = "ffff9c1a 3003687252 S Bo:2:005:2 -115 512 = aa";
        assert_eq!(parse_usbmon(l), Some(((2, 5), 512)));
        // submitted IN (no data moved yet): not counted
        let l = "ffff9c1a 3003687252 S Ii:1:002:1 -115:8 8 <";
        assert_eq!(parse_usbmon(l), None);
        // control setup: skipped
        let l = "ffff9c1a 3003687252 S Co:1:001:0 s 23 01 0010 0002 0000 0";
        assert_eq!(parse_usbmon(l), None);
        assert_eq!(parse_usbmon("garbage"), None);
    }

    #[test]
    fn lockdown_bracket() {
        assert!(!lockdown_on("[none] integrity confidentiality\n"));
        assert!(lockdown_on("none [integrity] confidentiality\n"));
        assert!(lockdown_on("none integrity [confidentiality]\n"));
    }
