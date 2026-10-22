// use ksni::TrayMethods; // provides the spawn method

use crate::i18n::{Msg, tr};

use ab_glyph::{Font, ScaleFont};
use ksni::{Icon, ToolTip};
use tokio::sync::mpsc::UnboundedSender;

use crate::bluetooth::aacp::{BatteryStatus, ControlCommandIdentifiers};
use crate::ui::messages::BluetoothUIMessage;
use crate::utils::get_app_settings_path;

#[derive(Debug)]
pub struct MyTray {
    pub conversation_detect_enabled: Option<bool>,
    pub battery_headphone: Option<u8>,
    pub battery_headphone_status: Option<BatteryStatus>,
    pub battery_l: Option<u8>,
    pub battery_l_status: Option<BatteryStatus>,
    pub battery_r: Option<u8>,
    pub battery_r_status: Option<BatteryStatus>,
    pub battery_c: Option<u8>,
    pub battery_c_status: Option<BatteryStatus>,
    pub connected: bool,
    pub listening_mode: Option<u8>,
    pub allow_off_option: Option<u8>,
    pub command_tx: Option<UnboundedSender<(ControlCommandIdentifiers, Vec<u8>)>>,
    pub ui_tx: Option<UnboundedSender<BluetoothUIMessage>>,
}

impl ksni::Tray for MyTray {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }
    fn title(&self) -> String {
        "AirPods".into()
    }
    fn icon_pixmap(&self) -> Vec<Icon> {
        let text = {
            let mut levels: Vec<u8> = Vec::new();
            if let Some(h) = self.battery_headphone {
                if self.battery_headphone_status != Some(BatteryStatus::Disconnected) {
                    levels.push(h);
                }
            } else {
                if let Some(l) = self.battery_l
                    && self.battery_l_status != Some(BatteryStatus::Disconnected)
                {
                    levels.push(l);
                }
                if let Some(r) = self.battery_r
                    && self.battery_r_status != Some(BatteryStatus::Disconnected)
                {
                    levels.push(r);
                }
                // if let Some(c) = self.battery_c {
                //     if self.battery_c_status != Some(BatteryStatus::Disconnected) {
                //         levels.push(c);
                //     }
                // }
            }
            let min_battery = levels.iter().min().copied();
            if let Some(b) = min_battery {
                format!("{}", b)
            } else {
                "?".to_string()
            }
        };
        let any_bud_charging = matches!(self.battery_l_status, Some(BatteryStatus::Charging))
            || matches!(self.battery_r_status, Some(BatteryStatus::Charging));
        let app_settings_path = get_app_settings_path();
        let settings = std::fs::read_to_string(&app_settings_path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
        let text_mode = settings
            .clone()
            .and_then(|v| v.get("tray_text_mode").cloned())
            .and_then(|ttm| serde_json::from_value(ttm).ok())
            .unwrap_or(false);
        let icon = generate_icon(&text, text_mode, any_bud_charging);
        vec![icon]
    }
    fn tool_tip(&self) -> ToolTip {
        let format_component =
            |label: &str, level: Option<u8>, status: Option<BatteryStatus>| -> String {
                match status {
                    Some(BatteryStatus::Disconnected) => format!("{}: -", label),
                    _ => {
                        let pct = level.map(|b| format!("{}%", b)).unwrap_or("?".to_string());
                        let suffix = if status == Some(BatteryStatus::Charging) {
                            "⚡"
                        } else {
                            ""
                        };
                        format!("{}: {}{}", label, pct, suffix)
                    }
                }
            };

        let l = format_component(&tr(Msg::BatteryShortLeft), self.battery_l, self.battery_l_status);
        let r = format_component(&tr(Msg::BatteryShortRight), self.battery_r, self.battery_r_status);
        let c = format_component(&tr(Msg::BatteryShortCase), self.battery_c, self.battery_c_status);

        ToolTip {
            icon_name: "".to_string(),
            icon_pixmap: vec![],
            title: tr(Msg::TrayBatteryStatus),
            description: format!("{} {} {}", l, r, c),
        }
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let allow_off = self.allow_off_option == Some(0x01);
        let options = if allow_off {
            vec![
                (tr(Msg::ModeOff), 0x01),
                (tr(Msg::ModeNoiseCancellation), 0x02),
                (tr(Msg::ModeTransparency), 0x03),
                (tr(Msg::ModeAdaptive), 0x04),
            ]
        } else {
            vec![
                (tr(Msg::ModeNoiseCancellation), 0x02),
                (tr(Msg::ModeTransparency), 0x03),
                (tr(Msg::ModeAdaptive), 0x04),
            ]
        };
        let selected = self
            .listening_mode
            .and_then(|mode| options.iter().position(|&(_, val)| val == mode))
            .unwrap_or(0);
        let options_clone = options.clone();
        vec![
            StandardItem {
                label: tr(Msg::TrayOpenWindow),
                icon_name: "window-new".into(),
                activate: Box::new(|this: &mut Self| {
                    if let Some(tx) = &this.ui_tx {
                        let _ = tx.send(BluetoothUIMessage::OpenWindow);
                    }
                }),
                ..Default::default()
            }
            .into(),
            RadioGroup {
                selected,
                select: Box::new(move |this: &mut Self, current| {
                    if let Some(tx) = &this.command_tx {
                        let value = options_clone
                            .get(current)
                            .map(|&(_, val)| val)
                            .unwrap_or(0x02);
                        let _ = tx.send((ControlCommandIdentifiers::ListeningMode, vec![value]));
                    }
                }),
                options: options
                    .into_iter()
                    .map(|(label, _)| RadioItem {
                        label: label.into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: tr(Msg::TrayConversationDetection),
                checked: self.conversation_detect_enabled.unwrap_or(false),
                enabled: self.conversation_detect_enabled.is_some(),
                activate: Box::new(|this: &mut Self| {
                    if let Some(tx) = &this.command_tx
                        && let Some(is_enabled) = this.conversation_detect_enabled
                    {
                        let new_state = !is_enabled;
                        let value = if !new_state { 0x02 } else { 0x01 };
                        let _ = tx.send((
                            ControlCommandIdentifiers::ConversationDetectConfig,
                            vec![value],
                        ));
                        this.conversation_detect_enabled = Some(new_state);
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: tr(Msg::TrayExit),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| {
                    crate::volume_duck::restore_blocking();
                    std::process::exit(0)
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

// Text size that fits `max_width`, never larger than `max_scale`. A three digit level such as 100
// does not fit at the full size and was cut off at the edges of the icon.
fn fit_scale(font: &ab_glyph::FontRef, text: &str, max_width: f32, max_scale: f32) -> f32 {
    use ab_glyph::{Font, ScaleFont};
    let at_max = font.as_scaled(ab_glyph::PxScale::from(max_scale));
    let width: f32 = text.chars().map(|c| at_max.h_advance(font.glyph_id(c))).sum();
    if width <= max_width || width <= 0.0 {
        max_scale
    } else {
        max_scale * max_width / width
    }
}

fn draw_centered_text(
    img: &mut image::RgbaImage,
    font: &ab_glyph::FontRef,
    text: &str,
    color: image::Rgba<u8>,
    max_width: f32,
    max_scale: f32,
) {
    use ab_glyph::{Font, PxScale, ScaleFont};
    use imageproc::drawing::draw_text_mut;

    let size = fit_scale(font, text, max_width, max_scale);
    let scale = PxScale::from(size);
    let scaled = font.as_scaled(scale);
    let width: f32 = text.chars().map(|c| scaled.h_advance(font.glyph_id(c))).sum();
    let x = ((img.width() as f32 - width) / 2.0).max(0.0) as i32;
    // digits are about 0.73 em tall, put their middle on the middle of the icon
    let baseline = img.height() as f32 / 2.0 + 0.365 * size;
    let y = (baseline - scaled.ascent()).max(0.0) as i32;
    draw_text_mut(img, color, x, y, scale, font, text);
}

fn render_icon(text: &str, text_mode: bool, charging: bool) -> Option<image::RgbaImage> {
    use ab_glyph::FontRef;
    use image::{ImageBuffer, Rgba};

    let width = 64;
    let height = 64;

    let mut img = ImageBuffer::from_fn(width, height, |_, _| Rgba([0u8, 0u8, 0u8, 0u8]));

    let font_data = include_bytes!("../../assets/font/DejaVuSans.ttf");
    let font = FontRef::try_from_slice(font_data).ok()?;
    let color = if charging {
        Rgba([0u8, 255u8, 0u8, 255u8])
    } else {
        Rgba([255u8, 255u8, 255u8, 255u8])
    };
    if !text_mode {
        let percentage = text.parse::<f32>().unwrap_or(0.0) / 100.0;

        let center_x = width as f32 / 2.0;
        let center_y = height as f32 / 2.0;
        let inner_radius = 22.0;
        let outer_radius = 28.0;

        // ring background
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > inner_radius && dist <= outer_radius {
                    img.put_pixel(x, y, Rgba([128u8, 128u8, 128u8, 255u8]));
                }
            }
        }

        // ring
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > inner_radius && dist <= outer_radius {
                    let angle = dy.atan2(dx);
                    let angle_from_top =
                        (angle + std::f32::consts::PI / 2.0).rem_euclid(2.0 * std::f32::consts::PI);
                    if angle_from_top <= percentage * 2.0 * std::f32::consts::PI {
                        img.put_pixel(x, y, Rgba([0u8, 255u8, 0u8, 255u8]));
                    }
                }
            }
        }
        // the level inside the ring, green while charging, inner diameter is 44
        draw_centered_text(&mut img, &font, text, color, 40.0, 34.0);
    } else {
        // battery text, kept inside the icon
        draw_centered_text(&mut img, &font, text, color, 60.0, 48.0);
    }
    Some(img)
}

fn generate_icon(text: &str, text_mode: bool, charging: bool) -> Icon {
    let width = 64;
    let height = 64;
    let Some(img) = render_icon(text, text_mode, charging) else {
        return Icon {
            width,
            height,
            data: vec![0u8; (width * height * 4) as usize],
        };
    };

    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for pixel in img.pixels() {
        data.push(pixel[3]);
        data.push(pixel[0]);
        data.push(pixel[1]);
        data.push(pixel[2]);
    }

    Icon {
        width: width,
        height: height,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ab_glyph::FontRef;

    fn font() -> FontRef<'static> {
        FontRef::try_from_slice(include_bytes!("../../assets/font/DejaVuSans.ttf")).unwrap()
    }

    fn count(img: &image::RgbaImage, inside: bool, pred: impl Fn(&image::Rgba<u8>) -> bool) -> usize {
        img.enumerate_pixels()
            .filter(|(x, y, p)| {
                let (dx, dy) = (*x as f32 - 32.0, *y as f32 - 32.0);
                let in_hole = (dx * dx + dy * dy).sqrt() <= 22.0;
                in_hole == inside && pred(p)
            })
            .count()
    }

    fn white(p: &image::Rgba<u8>) -> bool {
        p[3] > 200 && p[0] > 200 && p[1] > 200 && p[2] > 200
    }

    fn green(p: &image::Rgba<u8>) -> bool {
        p[3] > 200 && p[0] < 60 && p[1] > 200 && p[2] < 60
    }

    #[test]
    fn icon_is_a_64_by_64_argb_pixmap() {
        let icon = generate_icon("94", false, false);
        assert_eq!((icon.width, icon.height), (64, 64));
        assert_eq!(icon.data.len(), 64 * 64 * 4);
    }

    #[test]
    fn ring_mode_shows_the_level_inside_the_ring() {
        let img = render_icon("94", false, false).unwrap();
        assert!(count(&img, true, white) > 30, "no level drawn inside the ring");
        assert!(count(&img, false, green) > 500, "ring lost its progress arc");
    }

    #[test]
    fn charging_level_is_green() {
        let img = render_icon("45", false, true).unwrap();
        assert_eq!(count(&img, true, white), 0);
        assert!(count(&img, true, green) > 30);
    }

    #[test]
    fn three_digit_level_stays_inside_the_icon() {
        for text_mode in [false, true] {
            let img = render_icon("100", text_mode, false).unwrap();
            for (x, _y, p) in img.enumerate_pixels() {
                if p[3] > 0 {
                    assert!((2..62).contains(&x), "pixel at column {} in text_mode={}", x, text_mode);
                }
            }
            assert!(img.pixels().any(|p| p[3] > 0));
        }
    }

    #[test]
    fn fit_scale_only_shrinks_when_needed() {
        let font = font();
        assert_eq!(fit_scale(&font, "9", 40.0, 34.0), 34.0);
        let shrunk = fit_scale(&font, "100", 40.0, 48.0);
        assert!(shrunk < 48.0 && shrunk > 10.0);
    }

    // writes the icons as PNG files for a visual check: TRAY_ICON_DUMP=<dir> cargo test dump_tray_icons -- --ignored
    #[test]
    #[ignore]
    fn dump_tray_icons() {
        let Ok(dir) = std::env::var("TRAY_ICON_DUMP") else { return };
        for (name, text, text_mode, charging) in [
            ("ring-94", "94", false, false),
            ("ring-100", "100", false, false),
            ("ring-45-charging", "45", false, true),
            ("ring-7", "7", false, false),
            ("text-100", "100", true, false),
            ("text-94", "94", true, false),
        ] {
            render_icon(text, text_mode, charging).unwrap().save(format!("{}/{}.png", dir, name)).unwrap();
        }
    }
}
