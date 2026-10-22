// Guards the frozen audio behaviour: LibrePods must not select, lock, optimise or switch the
// Bluetooth audio codec or profile on its own, and must not touch what WirePlumber saved.
// The system (BlueZ, PipeWire, WirePlumber) negotiates and switches codecs, not LibrePods.

const SOURCES: &[(&str, &str)] = &[
    ("main.rs", include_str!("main.rs")),
    ("media_controller.rs", include_str!("media_controller.rs")),
    ("volume_duck.rs", include_str!("volume_duck.rs")),
    ("i18n.rs", include_str!("i18n.rs")),
    ("utils.rs", include_str!("utils.rs")),
    ("devices/airpods.rs", include_str!("devices/airpods.rs")),
    ("devices/nothing.rs", include_str!("devices/nothing.rs")),
    ("devices/enums.rs", include_str!("devices/enums.rs")),
    ("bluetooth/aacp.rs", include_str!("bluetooth/aacp.rs")),
    ("bluetooth/att.rs", include_str!("bluetooth/att.rs")),
    ("bluetooth/le.rs", include_str!("bluetooth/le.rs")),
    ("bluetooth/discovery.rs", include_str!("bluetooth/discovery.rs")),
    ("bluetooth/managers.rs", include_str!("bluetooth/managers.rs")),
    ("ui/window.rs", include_str!("ui/window.rs")),
    ("ui/airpods.rs", include_str!("ui/airpods.rs")),
    ("ui/nothing.rs", include_str!("ui/nothing.rs")),
    ("ui/tray.rs", include_str!("ui/tray.rs")),
];

// production code only: the trailing `mod tests` and comment lines are left out
fn production_code(source: &str) -> String {
    let end = source.find("#[cfg(test)]\nmod tests").unwrap_or(source.len());
    source[..end]
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn no_hardcoded_codec_or_profile_preference() {
    let forbidden = ["sbc_xq", "a2dp-sink", "\"aac\"", "systemctl", "wireplumber", "pactl", "wpctl", "set_default_sink", "set_default_source"];
    for (name, source) in SOURCES {
        let code = production_code(source).to_lowercase();
        for word in forbidden {
            // the historical code lives behind the feature, it is allowed to name profiles
            if *name == "media_controller.rs" && (word == "a2dp-sink" || word == "sbc_xq") {
                continue;
            }
            assert!(!code.contains(word), "{} contains {:?}", name, word);
        }
    }
}

#[test]
fn profile_writes_only_exist_behind_the_feature() {
    let media = include_str!("media_controller.rs");
    let gate = "#[cfg(feature = \"audio-profile-control\")]";
    for item in [
        "fn set_card_profile_sync",
        "pub async fn activate_a2dp_profile(&self) {\n        debug!(\"Entering",
        "pub async fn deactivate_a2dp_profile(&self) {\n        debug!(\"Entering",
    ] {
        let at = media.find(item).unwrap_or_else(|| panic!("missing {}", item));
        let before = &media[..at];
        let previous_line = before.trim_end().lines().last().unwrap_or("");
        assert_eq!(previous_line.trim(), gate, "{} is not behind the feature", item);
    }
    for (name, source) in SOURCES {
        if *name != "media_controller.rs" {
            assert!(!production_code(source).contains("set_card_profile"), "{} writes a card profile", name);
        }
    }
    // the only profile write in media_controller.rs is the gated helper
    let code = production_code(media);
    assert_eq!(code.matches("set_card_profile_by_index").count(), 1);
}
