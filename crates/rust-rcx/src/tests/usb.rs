use super::*;

// USB tests require hardware — just test that open fails gracefully
#[test]
fn test_open_no_device() {
    // Should return an error, not panic
    let result = RcxUsb::open();
    // Either succeeds (if a tower is connected) or fails with an error message
    if let Err(e) = result {
        assert!(e.contains("No LEGO USB IR tower found") || e.contains("USB"));
    }
}
