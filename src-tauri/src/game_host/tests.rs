use super::*;
#[test]
fn restricted_commands_do_not_reach_application() {
    for label in ["game-abc", "migration-abc"] {
        assert!(Host::command_allowed(label, "game_plugin_call"));
        for method in [
            "get_settings",
            "get_notebook",
            "submit_chat",
            "create_provider",
            "plugin:autostart|enable",
            "plugin:event|listen",
        ] {
            assert!(!Host::command_allowed(label, method));
        }
    }
    assert!(Host::command_allowed("settings", "get_settings"));
}
#[test]
fn network_rejects_private_and_mixed_dns() {
    for addr in [
        "127.0.0.1:443",
        "10.0.0.1:443",
        "169.254.1.1:443",
        "[::1]:443",
        "[::ffff:127.0.0.1]:443",
        "[fc00::1]:443",
        "[fe80::1]:443",
    ] {
        assert!(network::checked_addresses(vec![addr.parse().unwrap()]).is_err());
    }
    assert!(network::checked_addresses(vec!["8.8.8.8:443".parse().unwrap()]).is_ok());
    assert!(network::checked_addresses(vec![
        "8.8.8.8:443".parse().unwrap(),
        "192.168.1.2:443".parse().unwrap()
    ])
    .is_err());
}
#[test]
fn network_does_not_forward_host_credentials() {
    for header in [
        "Cookie",
        "Host",
        "Proxy-Authorization",
        "Connection",
        "X-Forwarded-For",
    ] {
        let mut h = std::collections::BTreeMap::new();
        h.insert(header.into(), "secret".into());
        assert!(network::headers(&h).is_err());
    }
    let mut h = std::collections::BTreeMap::new();
    h.insert("Authorization".into(), "Bearer plugin-owned-key".into());
    assert!(network::headers(&h).is_ok());
}
