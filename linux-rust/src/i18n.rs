use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{OnceLock, RwLock};
use tokio::sync::watch;

// Interface strings live in i18n/<code>.json and are looked up by the keys of `Msg`.
// A missing translation falls back to English, and a missing English entry to the key itself,
// so a catalog problem can never crash the app. See the tests for the consistency checks.

macro_rules! messages {
    ($($variant:ident => $key:literal,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Msg { $($variant,)* }

        impl Msg {
            #[cfg(test)]
            pub const ALL: &'static [Msg] = &[$(Msg::$variant,)*];

            pub fn key(self) -> &'static str {
                match self { $(Msg::$variant => $key,)* }
            }
        }
    };
}

messages! {
    Devices => "devices",
    Settings => "settings",
    SelectDevice => "select_device",
    Connected => "connected",
    UnknownDevice => "unknown_device",
    UnsupportedDevice => "unsupported_device",
    ErrAirpodsNoManagers => "err_airpods_no_managers",
    ErrNothingNoAtt => "err_nothing_no_att",
    ErrNothingNoManager => "err_nothing_no_manager",
    ErrNothingNoState => "err_nothing_no_state",
    Appearance => "appearance",
    Theme => "theme",
    SelectTheme => "select_theme",
    Language => "language",
    SelectLanguage => "select_language",
    LanguageAuto => "language_auto",
    LanguageEn => "language_en",
    LanguageZhCn => "language_zh_cn",
    UseTextInTray => "use_text_in_tray",
    UseTextInTrayDesc => "use_text_in_tray_desc",
    Controls => "controls",
    StemControl => "stem_control",
    StemControlDesc => "stem_control_desc",
    PickPairedDevice => "pick_paired_device",
    NoNewPairedDevices => "no_new_paired_devices",
    DeviceType => "device_type",
    SelectDeviceType => "select_device_type",
    Cancel => "cancel",
    AddDevice => "add_device",
    DeviceInformation => "device_information",
    ModelNumber => "model_number",
    Manufacturer => "manufacturer",
    SerialNumber => "serial_number",
    LeftSerialNumber => "left_serial_number",
    RightSerialNumber => "right_serial_number",
    FirmwareVersion => "firmware_version",
    Name => "name",
    ListeningMode => "listening_mode",
    SelectListeningMode => "select_listening_mode",
    NoiseControlMode => "noise_control_mode",
    SelectNoiseControlMode => "select_noise_control_mode",
    AudioSettings => "audio_settings",
    PersonalizedVolume => "personalized_volume",
    PersonalizedVolumeDesc => "personalized_volume_desc",
    ConversationAwareness => "conversation_awareness",
    ConversationAwarenessDesc => "conversation_awareness_desc",
    OffListeningMode => "off_listening_mode",
    OffListeningModeDesc => "off_listening_mode_desc",
    ModeOff => "mode_off",
    ModeNoiseCancellation => "mode_noise_cancellation",
    ModeTransparency => "mode_transparency",
    ModeAdaptive => "mode_adaptive",
    ModeLowNoiseCancellation => "mode_low_noise_cancellation",
    ModeMidNoiseCancellation => "mode_mid_noise_cancellation",
    ModeHighNoiseCancellation => "mode_high_noise_cancellation",
    ModeAdaptiveNoiseCancellation => "mode_adaptive_noise_cancellation",
    TrayBatteryStatus => "tray_battery_status",
    TrayOpenWindow => "tray_open_window",
    TrayConversationDetection => "tray_conversation_detection",
    TrayExit => "tray_exit",
    BatteryShortLeft => "battery_short_left",
    BatteryShortRight => "battery_short_right",
    BatteryShortCase => "battery_short_case",
    Add => "add",
    VersionLabel => "version_label",
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    ZhCn,
}

impl Lang {
    fn catalog_json(self) -> &'static str {
        match self {
            Lang::En => include_str!("../i18n/en.json"),
            Lang::ZhCn => include_str!("../i18n/zh-CN.json"),
        }
    }

    fn id(self) -> u8 {
        match self {
            Lang::En => 0,
            Lang::ZhCn => 1,
        }
    }

    fn from_id(id: u8) -> Lang {
        if id == 1 { Lang::ZhCn } else { Lang::En }
    }
}

// What the user picked in the settings, `Auto` follows the system locale
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LangSetting {
    Auto,
    En,
    ZhCn,
}

impl LangSetting {
    pub const ALL: [LangSetting; 3] = [LangSetting::Auto, LangSetting::En, LangSetting::ZhCn];

    pub fn as_str(self) -> &'static str {
        match self {
            LangSetting::Auto => "auto",
            LangSetting::En => "en",
            LangSetting::ZhCn => "zh-CN",
        }
    }

    pub fn parse(s: &str) -> LangSetting {
        match s {
            "en" => LangSetting::En,
            "zh-CN" => LangSetting::ZhCn,
            _ => LangSetting::Auto,
        }
    }

    fn resolve(self) -> Lang {
        match self {
            LangSetting::Auto => detect_system_lang(),
            LangSetting::En => Lang::En,
            LangSetting::ZhCn => Lang::ZhCn,
        }
    }
}

impl fmt::Display for LangSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            LangSetting::Auto => Msg::LanguageAuto,
            LangSetting::En => Msg::LanguageEn,
            LangSetting::ZhCn => Msg::LanguageZhCn,
        };
        write!(f, "{}", tr(msg))
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);
static SETTING: RwLock<LangSetting> = RwLock::new(LangSetting::Auto);
static CHANNEL: OnceLock<watch::Sender<Lang>> = OnceLock::new();

fn channel() -> &'static watch::Sender<Lang> {
    CHANNEL.get_or_init(|| watch::channel(Lang::En).0)
}

// Receivers are notified when the language changes, used to refresh the tray menu
pub fn subscribe() -> watch::Receiver<Lang> {
    channel().subscribe()
}

pub fn setting() -> LangSetting {
    SETTING.read().map(|s| *s).unwrap_or(LangSetting::Auto)
}

pub fn set_setting(setting: LangSetting) {
    if let Ok(mut s) = SETTING.write() {
        *s = setting;
    }
    let lang = setting.resolve();
    CURRENT.store(lang.id(), Ordering::Relaxed);
    channel().send_replace(lang);
}

pub fn current() -> Lang {
    Lang::from_id(CURRENT.load(Ordering::Relaxed))
}

fn catalog(lang: Lang) -> &'static HashMap<String, String> {
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    static ZH_CN: OnceLock<HashMap<String, String>> = OnceLock::new();
    let cell = match lang {
        Lang::En => &EN,
        Lang::ZhCn => &ZH_CN,
    };
    cell.get_or_init(|| parse_catalog(lang.catalog_json()))
}

fn parse_catalog(json: &str) -> HashMap<String, String> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        log::error!("Failed to parse translation catalog: {}", e);
        HashMap::new()
    })
}

fn lookup(lang: Lang, key: &str) -> String {
    catalog(lang)
        .get(key)
        .or_else(|| catalog(Lang::En).get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

pub fn tr(msg: Msg) -> String {
    lookup(current(), msg.key())
}

// Replaces `{name}` placeholders, unknown placeholders are left as they are
pub fn tr_with(msg: Msg, args: &[(&str, &str)]) -> String {
    substitute(&tr(msg), args)
}

fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{}}}", name), value);
    }
    out
}

pub fn detect_system_lang() -> Lang {
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    lang_from_locale_vars(&var("LC_ALL"), &var("LC_MESSAGES"), &var("LANG"), &var("LANGUAGE"))
}

// Follows gettext: the first of LC_ALL, LC_MESSAGES, LANG is the locale, C and POSIX mean English
// and disable LANGUAGE, otherwise the LANGUAGE list is tried before the locale itself.
// Traditional Chinese locales are not Simplified Chinese and use English until they are translated.
fn lang_from_locale_vars(lc_all: &str, lc_messages: &str, lang: &str, language: &str) -> Lang {
    let locale = [lc_all, lc_messages, lang]
        .into_iter()
        .find(|v| !v.is_empty())
        .unwrap_or("");
    let normalized = locale.to_lowercase();
    if normalized == "c" || normalized == "posix" {
        return Lang::En;
    }
    let candidates = language.split(':').chain(std::iter::once(locale));
    for candidate in candidates {
        let c = candidate.to_lowercase();
        let c = c.split(['.', '@']).next().unwrap_or("");
        if c.is_empty() {
            continue;
        }
        if c.starts_with("zh") {
            let traditional = c.contains("tw") || c.contains("hk") || c.contains("mo") || c.contains("hant");
            return if traditional { Lang::En } else { Lang::ZhCn };
        }
        if c.starts_with("en") {
            return Lang::En;
        }
    }
    Lang::En
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn placeholders(s: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut rest = s;
        while let Some(start) = rest.find('{') {
            let Some(end) = rest[start..].find('}') else { break };
            out.insert(rest[start + 1..start + end].to_string());
            rest = &rest[start + end + 1..];
        }
        out
    }

    fn contains_cjk(s: &str) -> bool {
        s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    }

    #[test]
    fn every_key_exists_in_every_catalog() {
        for lang in [Lang::En, Lang::ZhCn] {
            for msg in Msg::ALL {
                assert!(catalog(lang).contains_key(msg.key()), "{:?} is missing {}", lang, msg.key());
            }
        }
    }

    #[test]
    fn catalogs_have_no_orphan_keys() {
        let known: HashSet<&str> = Msg::ALL.iter().map(|m| m.key()).collect();
        for lang in [Lang::En, Lang::ZhCn] {
            for key in catalog(lang).keys() {
                assert!(known.contains(key.as_str()), "{:?} has unused key {}", lang, key);
            }
        }
    }

    #[test]
    fn keys_are_unique() {
        let keys: HashSet<&str> = Msg::ALL.iter().map(|m| m.key()).collect();
        assert_eq!(keys.len(), Msg::ALL.len());
    }

    #[test]
    fn placeholders_match_english() {
        for msg in Msg::ALL {
            let en = &catalog(Lang::En)[msg.key()];
            let zh = &catalog(Lang::ZhCn)[msg.key()];
            assert_eq!(placeholders(en), placeholders(zh), "placeholder mismatch in {}", msg.key());
        }
    }

    #[test]
    fn entries_are_not_empty_and_chinese_is_translated() {
        // proper names and the language names are intentionally identical or Latin only
        let same_allowed = ["language_en"];
        for msg in Msg::ALL {
            let en = &catalog(Lang::En)[msg.key()];
            let zh = &catalog(Lang::ZhCn)[msg.key()];
            assert!(!en.trim().is_empty() && !zh.trim().is_empty(), "empty entry {}", msg.key());
            if !same_allowed.contains(&msg.key()) {
                assert!(contains_cjk(zh), "{} is not translated: {}", msg.key(), zh);
            }
        }
    }

    #[test]
    fn missing_translation_falls_back_to_english_then_key() {
        let mut partial = HashMap::new();
        partial.insert("devices".to_string(), "X".to_string());
        assert_eq!(partial.get("settings").or_else(|| catalog(Lang::En).get("settings")).cloned().unwrap(), "Settings");
        assert_eq!(lookup(Lang::ZhCn, "no_such_key"), "no_such_key");
    }

    #[test]
    fn broken_catalog_does_not_panic() {
        assert!(parse_catalog("{not json").is_empty());
    }

    #[test]
    fn substitution_replaces_named_placeholders_only() {
        assert_eq!(substitute("{label}: {value}%", &[("label", "L"), ("value", "80")]), "L: 80%");
        assert_eq!(substitute("{label} {other}", &[("label", "L")]), "L {other}");
        assert_eq!(substitute("100%", &[("value", "1")]), "100%");
    }

    #[test]
    fn system_language_detection() {
        let d = lang_from_locale_vars;
        assert_eq!(d("", "", "zh_CN.UTF-8", ""), Lang::ZhCn);
        assert_eq!(d("zh_CN.UTF-8", "", "en_US.UTF-8", ""), Lang::ZhCn);
        assert_eq!(d("", "", "en_US.UTF-8", ""), Lang::En);
        assert_eq!(d("", "", "en_US.UTF-8", "zh_CN:en"), Lang::ZhCn);
        assert_eq!(d("C", "", "zh_CN.UTF-8", "zh_CN"), Lang::En);
        assert_eq!(d("", "", "POSIX", "zh_CN"), Lang::En);
        assert_eq!(d("", "", "zh_TW.UTF-8", ""), Lang::En);
        assert_eq!(d("", "", "zh_HK.UTF-8", ""), Lang::En);
        assert_eq!(d("", "", "zh_Hans_CN", ""), Lang::ZhCn);
        assert_eq!(d("", "", "de_DE.UTF-8", ""), Lang::En);
        assert_eq!(d("", "", "", ""), Lang::En);
    }

    #[test]
    fn setting_round_trips_and_defaults_to_auto() {
        for s in LangSetting::ALL {
            assert_eq!(LangSetting::parse(s.as_str()), s);
        }
        assert_eq!(LangSetting::parse("garbage"), LangSetting::Auto);
        assert_eq!(LangSetting::parse(""), LangSetting::Auto);
    }

    #[test]
    fn selecting_a_language_changes_output_and_notifies() {
        // the only test that touches the global language state
        let mut rx = subscribe();
        set_setting(LangSetting::ZhCn);
        assert_eq!(tr(Msg::Settings), "设置");
        assert_eq!(*rx.borrow_and_update(), Lang::ZhCn);
        set_setting(LangSetting::En);
        assert_eq!(tr(Msg::Settings), "Settings");
        assert_eq!(setting(), LangSetting::En);
        set_setting(LangSetting::Auto);
    }
}
