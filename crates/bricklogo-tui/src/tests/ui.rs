use super::*;

fn app_with_status(
    devices: &[&str],
    active: Option<&str>,
    outputs: &[&str],
    inputs: &[&str],
) -> App {
    let mut app = App::new();
    app.connected_devices = devices.iter().map(|s| s.to_string()).collect();
    app.active_device = active.map(|s| s.to_string());
    app.selected_outputs = outputs.iter().map(|s| s.to_string()).collect();
    app.selected_inputs = inputs.iter().map(|s| s.to_string()).collect();
    app
}

#[test]
fn test_status_line_strings_with_active_and_cross_device_ports() {
    let app = app_with_status(
        &["bot1", "bot2", "bot3"],
        Some("bot1"),
        &["a", "bot2.b"],
        &["bot3.c"],
    );
    assert_eq!(
        status_line_strings(&app),
        vec![
            "[devices: bot1* bot2 bot3]".to_string(),
            "[talkto: a bot2.b]".to_string(),
            "[listento: bot3.c]".to_string(),
        ]
    );
}

#[test]
fn test_status_line_strings_when_no_devices() {
    let app = app_with_status(&[], None, &[], &[]);
    assert_eq!(
        status_line_strings(&app),
        vec![
            "[devices: none]".to_string(),
            "[talkto: -]".to_string(),
            "[listento: -]".to_string(),
        ]
    );
}
