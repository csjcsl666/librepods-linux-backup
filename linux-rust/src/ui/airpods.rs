use crate::i18n::{Msg, tr, tr_with};
use crate::bluetooth::aacp::{AACPManager, BatteryComponent, BatteryInfo, BatteryStatus, ControlCommandIdentifiers};
use iced::Alignment::End;
use iced::border::Radius;
use iced::overlay::menu;
use iced::widget::button::Style;
use iced::widget::rule::FillMode;
use iced::widget::{
    Space, button, column, combo_box, container, row, rule, text, text_input, toggler,
};
use iced::{Background, Border, Center, Color, Length, Padding, Theme};
use log::error;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use tokio::runtime::Runtime;
// use crate::bluetooth::att::ATTManager;
use crate::devices::enums::{AirPodsState, DeviceData, DeviceInformation, DeviceState};
use crate::ui::window::Message;

pub fn airpods_view<'a>(
    mac: &'a str,
    devices_list: &HashMap<String, DeviceData>,
    state: &'a AirPodsState,
    aacp_manager: Arc<AACPManager>,
    // att_manager: Arc<ATTManager>
) -> iced::widget::Container<'a, Message> {
    let mac = mac.to_string();
    // order: name, noise control, press and hold config, call controls (not sure if why it might be needed, adding it just in case), audio (personalized volume, conversational awareness, adaptive audio slider), connection settings, microphone, head gestures (not adding this), off listening mode, device information

    let aacp_manager_for_rename = aacp_manager.clone();
    let rename_input = container(
        row![
            Space::new().width(10),
            text(tr(Msg::Name)).size(16).style(|theme: &Theme| {
                let mut style = text::Style::default();
                style.color = Some(theme.palette().text);
                style
            }),
            Space::new().width(Length::Fill),
            text_input("", &state.device_name)
                .padding(Padding {
                    top: 5.0,
                    bottom: 5.0,
                    left: 10.0,
                    right: 10.0,
                })
                .style(|theme: &Theme, _status| {
                    text_input::Style {
                        background: Background::Color(Color::TRANSPARENT),
                        border: Default::default(),
                        icon: Default::default(),
                        placeholder: theme.palette().text.scale_alpha(0.7),
                        value: theme.palette().text,
                        selection: Default::default(),
                    }
                })
                .align_x(End)
                .on_input({
                    let mac = mac.clone();
                    let state = state.clone();
                    move |new_name| {
                        let aacp_manager = aacp_manager_for_rename.clone();
                        run_async_in_thread({
                            let new_name = new_name.clone();
                            async move {
                                aacp_manager
                                    .send_rename_packet(&new_name)
                                    .await
                                    .expect("Failed to send rename packet");
                            }
                        });
                        let mut state = state.clone();
                        state.device_name = new_name.clone();
                        Message::StateChanged(mac.to_string(), DeviceState::AirPods(state))
                    }
                })
        ]
        .align_y(Center),
    )
    .padding(Padding {
        top: 5.0,
        bottom: 5.0,
        left: 10.0,
        right: 10.0,
    })
    .style(|theme: &Theme| {
        let mut style = container::Style::default();
        style.background = Some(Background::Color(theme.palette().primary.scale_alpha(0.1)));
        let mut border = Border::default();
        border.color = theme.palette().primary.scale_alpha(0.5);
        style.border = border.rounded(16);
        style
    });

    let listening_mode = container(
        row![
            text(tr(Msg::ListeningMode)).size(16).style(|theme: &Theme| {
                let mut style = text::Style::default();
                style.color = Some(theme.palette().text);
                style
            }),
            Space::new().width(Length::Fill),
            {
                let state_clone = state.clone();
                let mac = mac.clone();
                // this combo_box doesn't go really well with the design, but I am not writing my own dropdown menu for this
                combo_box(
                    &state.noise_control_state,
                    &tr(Msg::SelectListeningMode),
                    Some(&state.noise_control_mode.clone()),
                    {
                        let aacp_manager = aacp_manager.clone();
                        move |selected_mode| {
                            let aacp_manager = aacp_manager.clone();
                            let selected_mode_c = selected_mode.clone();
                            run_async_in_thread(async move {
                                aacp_manager
                                    .send_control_command(
                                        ControlCommandIdentifiers::ListeningMode,
                                        &[selected_mode_c.to_byte()],
                                    )
                                    .await
                                    .expect("Failed to send Noise Control Mode command");
                            });
                            let mut state = state_clone.clone();
                            state.noise_control_mode = selected_mode.clone();
                            Message::StateChanged(mac.to_string(), DeviceState::AirPods(state))
                        }
                    },
                )
                .width(Length::from(200))
                .input_style(|theme: &Theme, _status| text_input::Style {
                    background: Background::Color(theme.palette().primary.scale_alpha(0.2)),
                    border: Border {
                        width: 1.0,
                        color: theme.palette().text.scale_alpha(0.3),
                        radius: Radius::from(4.0),
                    },
                    icon: Default::default(),
                    placeholder: theme.palette().text,
                    value: theme.palette().text,
                    selection: Default::default(),
                })
                .padding(Padding {
                    top: 5.0,
                    bottom: 5.0,
                    left: 10.0,
                    right: 10.0,
                })
                .menu_style(|theme: &Theme| menu::Style {
                    background: Background::Color(theme.palette().background),
                    border: Border {
                        width: 1.0,
                        color: theme.palette().text,
                        radius: Radius::from(4.0),
                    },
                    text_color: theme.palette().text,
                    selected_text_color: theme.palette().text,
                    selected_background: Background::Color(
                        theme.palette().primary.scale_alpha(0.3),
                    ),
                    shadow: Default::default()
                })
            }
        ]
        .align_y(Center),
    )
    .padding(Padding {
        top: 5.0,
        bottom: 5.0,
        left: 18.0,
        right: 18.0,
    })
    .style(|theme: &Theme| {
        let mut style = container::Style::default();
        style.background = Some(Background::Color(theme.palette().primary.scale_alpha(0.1)));
        let mut border = Border::default();
        border.color = theme.palette().primary.scale_alpha(0.5);
        style.border = border.rounded(16);
        style
    });

    let mac_audio = mac.clone();
    let mac_information = mac.clone();

    let audio_settings_col = column![
        container(
            text(tr(Msg::AudioSettings)).size(18).style(
                |theme: &Theme| {
                    let mut style = text::Style::default();
                    style.color = Some(theme.palette().primary);
                    style
                }
            )
        )
        .padding(Padding{
            top: 5.0,
            bottom: 5.0,
            left: 18.0,
            right: 18.0,
        }),

        container(
            column![
                {
                    let aacp_manager_pv = aacp_manager.clone();
                    row![
                        column![
                            text(tr(Msg::PersonalizedVolume)).size(16),
                            text(tr(Msg::PersonalizedVolumeDesc)).size(12).style(
                                |theme: &Theme| {
                                    let mut style = text::Style::default();
                                    style.color = Some(theme.palette().text.scale_alpha(0.7));
                                    style
                                }
                            ).width(Length::Fill),
                        ].width(Length::Fill),
                        toggler(state.personalized_volume_enabled)
                            .on_toggle(
                            {
                                let mac = mac_audio.clone();
                                let state = state.clone();
                                move |is_enabled| {
                                    let aacp_manager = aacp_manager_pv.clone();
                                    let mac = mac.clone();
                                    run_async_in_thread(
                                        async move {
                                            aacp_manager.send_control_command(
                                                ControlCommandIdentifiers::AdaptiveVolumeConfig,
                                                if is_enabled { &[0x01] } else { &[0x02] }
                                            ).await.expect("Failed to send Personalized Volume command");
                                        }
                                    );
                                    let mut state = state.clone();
                                    state.personalized_volume_enabled = is_enabled;
                                    Message::StateChanged(mac, DeviceState::AirPods(state))
                                }
                            }
                        )
                        .spacing(0)
                        .size(20)
                    ]
                    .align_y(Center)
                    .spacing(8)
                },
                rule::horizontal(1).style(
                    |theme: &Theme| {
                        rule::Style {
                            color: theme.palette().text.scale_alpha(0.2),
                            radius: Radius::from(12),
                            fill_mode: FillMode::Full,
                            snap: false
                        }
                    }
                ),
                {
                    let aacp_manager_conv_detect = aacp_manager.clone();
                    row![
                        column![
                            text(tr(Msg::ConversationAwareness)).size(16),
                            text(tr(Msg::ConversationAwarenessDesc)).size(12).style(
                                |theme: &Theme| {
                                    let mut style = text::Style::default();
                                    style.color = Some(theme.palette().text.scale_alpha(0.7));
                                    style
                                }
                            ).width(Length::Fill),
                        ].width(Length::Fill),
                        toggler(state.conversation_awareness_enabled)
                            .on_toggle(move |is_enabled| {
                                let aacp_manager = aacp_manager_conv_detect.clone();
                                run_async_in_thread(
                                    async move {
                                        aacp_manager.send_control_command(
                                            ControlCommandIdentifiers::ConversationDetectConfig,
                                            if is_enabled { &[0x01] } else { &[0x02] }
                                        ).await.expect("Failed to send Conversation Awareness command");
                                    }
                                );
                                let mut state = state.clone();
                                state.conversation_awareness_enabled = is_enabled;
                                Message::StateChanged(mac_audio.to_string(), DeviceState::AirPods(state))
                            })
                        .spacing(0)
                        .size(20)
                    ]
                    .align_y(Center)
                    .spacing(8)
                }
            ]
                .spacing(4)
                .padding(8)
        )
        .padding(Padding{
            top: 5.0,
            bottom: 5.0,
            left: 10.0,
            right: 10.0,
        })
        .style(
            |theme: &Theme| {
                let mut style = container::Style::default();
                style.background = Some(Background::Color(theme.palette().primary.scale_alpha(0.1)));
                let mut border = Border::default();
                border.color = theme.palette().primary.scale_alpha(0.5);
                style.border = border.rounded(16);
                style
            }
        )
    ];

    let off_listening_mode_toggle = {
        let aacp_manager_olm = aacp_manager.clone();
        let mac = mac.clone();
        container(row![
            column![
                text(tr(Msg::OffListeningMode)).size(16),
                text(tr(Msg::OffListeningModeDesc)).size(12).style(
                    |theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text.scale_alpha(0.7));
                        style
                    }
                ).width(Length::Fill)
            ].width(Length::Fill),
            toggler(state.allow_off_mode)
                .on_toggle(move |is_enabled| {
                    let aacp_manager = aacp_manager_olm.clone();
                    run_async_in_thread(
                        async move {
                            aacp_manager.send_control_command(
                                ControlCommandIdentifiers::AllowOffOption,
                                if is_enabled { &[0x01] } else { &[0x02] }
                            ).await.expect("Failed to send Off Listening Mode command");
                        }
                    );
                    let mut state = state.clone();
                    state.allow_off_mode = is_enabled;
                    Message::StateChanged(mac.to_string(), DeviceState::AirPods(state))
                })
            .spacing(0)
            .size(20)
        ]
            .align_y(Center)
            .spacing(8)
        )
            .padding(Padding{
                top: 5.0,
                bottom: 5.0,
                left: 18.0,
                right: 18.0,
            })
            .style(
                |theme: &Theme| {
                    let mut style = container::Style::default();
                    style.background = Some(Background::Color(theme.palette().primary.scale_alpha(0.1)));
                    let mut border = Border::default();
                    border.color = theme.palette().primary.scale_alpha(0.5);
                    style.border = border.rounded(16);
                    style
                }
            )
    };

    let mut information_col = column![];
    if let Some(device) = devices_list.get(mac_information.as_str()) {
        if let Some(DeviceInformation::AirPods(ref airpods_info)) = device.information {
            let info_rows = column![
                row![
                    text(tr(Msg::ModelNumber)).size(16).style(|theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text);
                        style
                    }),
                    Space::new().width(Length::Fill),
                    text(airpods_info.model_number.clone()).size(16)
                ],
                row![
                    text(tr(Msg::Manufacturer)).size(16).style(|theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text);
                        style
                    }),
                    Space::new().width(Length::Fill),
                    text(airpods_info.manufacturer.clone()).size(16)
                ],
                row![
                    text(tr(Msg::SerialNumber)).size(16).style(|theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text);
                        style
                    }),
                    Space::new().width(Length::Fill),
                    button(text(airpods_info.serial_number.clone()).size(16))
                        .style(|theme: &Theme, _status| {
                            let mut style = Style::default();
                            style.text_color = theme.palette().text;
                            style.background = Some(Background::Color(Color::TRANSPARENT));
                            style
                        })
                        .padding(0)
                        .on_press(Message::CopyToClipboard(airpods_info.serial_number.clone()))
                ],
                row![
                    text(tr(Msg::LeftSerialNumber)).size(16).style(|theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text);
                        style
                    }),
                    Space::new().width(Length::Fill),
                    button(text(airpods_info.left_serial_number.clone()).size(16))
                        .style(|theme: &Theme, _status| {
                            let mut style = Style::default();
                            style.text_color = theme.palette().text;
                            style.background = Some(Background::Color(Color::TRANSPARENT));
                            style
                        })
                        .padding(0)
                        .on_press(Message::CopyToClipboard(
                            airpods_info.left_serial_number.clone()
                        ))
                ],
                row![
                    text(tr(Msg::RightSerialNumber)).size(16).style(|theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text);
                        style
                    }),
                    Space::new().width(Length::Fill),
                    button(text(airpods_info.right_serial_number.clone()).size(16))
                        .style(|theme: &Theme, _status| {
                            let mut style = Style::default();
                            style.text_color = theme.palette().text;
                            style.background = Some(Background::Color(Color::TRANSPARENT));
                            style
                        })
                        .padding(0)
                        .on_press(Message::CopyToClipboard(
                            airpods_info.right_serial_number.clone()
                        ))
                ],
                row![
                    text(tr_with(Msg::VersionLabel, &[("n", "1")])).size(16).style(|theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text);
                        style
                    }),
                    Space::new().width(Length::Fill),
                    text(airpods_info.version1.clone()).size(16)
                ],
                row![
                    text(tr_with(Msg::VersionLabel, &[("n", "2")])).size(16).style(|theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text);
                        style
                    }),
                    Space::new().width(Length::Fill),
                    text(airpods_info.version2.clone()).size(16)
                ],
                row![
                    text(tr_with(Msg::VersionLabel, &[("n", "3")])).size(16).style(|theme: &Theme| {
                        let mut style = text::Style::default();
                        style.color = Some(theme.palette().text);
                        style
                    }),
                    Space::new().width(Length::Fill),
                    text(airpods_info.version3.clone()).size(16)
                ]
            ]
            .spacing(4)
            .padding(8);

            information_col = column![
                container(text(tr(Msg::DeviceInformation)).size(18).style(|theme: &Theme| {
                    let mut style = text::Style::default();
                    style.color = Some(theme.palette().primary);
                    style
                }))
                .padding(Padding {
                    top: 5.0,
                    bottom: 5.0,
                    left: 18.0,
                    right: 18.0,
                }),
                container(info_rows)
                    .padding(Padding {
                        top: 5.0,
                        bottom: 5.0,
                        left: 10.0,
                        right: 10.0,
                    })
                    .style(|theme: &Theme| {
                        let mut style = container::Style::default();
                        style.background =
                            Some(Background::Color(theme.palette().primary.scale_alpha(0.1)));
                        let mut border = Border::default();
                        border.color = theme.palette().primary.scale_alpha(0.5);
                        style.border = border.rounded(16);
                        style
                    })
            ];
        } else {
            error!(
                "Expected AirPodsInformation for device {}, got something else",
                mac.clone()
            );
        }
    }

    let battery_rows = battery_lines(&state.battery, &tr(Msg::Charging));
    let battery_col = if battery_rows.is_empty() {
        column![]
    } else {
        let mut rows = column![].spacing(4).padding(8);
        for (label, value) in battery_rows {
            rows = rows.push(row![
                text(tr(label)).size(16),
                Space::new().width(Length::Fill),
                text(value).size(16)
            ]);
        }
        column![
            container(text(tr(Msg::Battery)).size(18).style(|theme: &Theme| {
                let mut style = text::Style::default();
                style.color = Some(theme.palette().primary);
                style
            }))
            .padding(Padding {
                top: 5.0,
                bottom: 5.0,
                left: 18.0,
                right: 18.0,
            }),
            container(rows)
                .padding(Padding {
                    top: 5.0,
                    bottom: 5.0,
                    left: 10.0,
                    right: 10.0,
                })
                .style(|theme: &Theme| {
                    let mut style = container::Style::default();
                    style.background =
                        Some(Background::Color(theme.palette().primary.scale_alpha(0.1)));
                    let mut border = Border::default();
                    border.color = theme.palette().primary.scale_alpha(0.5);
                    style.border = border.rounded(16);
                    style
                })
        ]
    };

    container(column![
        rename_input,
        Space::new().height(Length::from(20)),
        battery_col,
        Space::new().height(Length::from(20)),
        listening_mode,
        Space::new().height(Length::from(20)),
        audio_settings_col,
        Space::new().height(Length::from(20)),
        off_listening_mode_toggle,
        Space::new().height(Length::from(20)),
        information_col
    ])
    .padding(20)
    .center_x(Length::Fill)
    .height(Length::Fill)
}

fn run_async_in_thread<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(fut);
    });
}

// Rows for the battery section: left bud, right bud and case, in that order. Components the
// device does not report are left out, a disconnected component is shown as a dash.
fn battery_lines(info: &[BatteryInfo], charging_label: &str) -> Vec<(Msg, String)> {
    [
        (BatteryComponent::Left, Msg::BatteryLeft),
        (BatteryComponent::Right, Msg::BatteryRight),
        (BatteryComponent::Case, Msg::BatteryCase),
    ]
    .into_iter()
    .filter_map(|(component, label)| {
        let item = info.iter().find(|i| i.component == component)?;
        let value = match item.status {
            BatteryStatus::Disconnected => "\u{2014}".to_string(),
            BatteryStatus::Charging => format!("{}% \u{00b7} {}", item.level, charging_label),
            BatteryStatus::NotCharging => format!("{}%", item.level),
        };
        Some((label, value))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(component: BatteryComponent, level: u8, status: BatteryStatus) -> BatteryInfo {
        BatteryInfo { component, level, status }
    }

    #[test]
    fn lists_left_right_case_in_order_with_charging() {
        let data = vec![
            info(BatteryComponent::Case, 40, BatteryStatus::Charging),
            info(BatteryComponent::Right, 93, BatteryStatus::NotCharging),
            info(BatteryComponent::Left, 94, BatteryStatus::NotCharging),
        ];
        let lines = battery_lines(&data, "Charging");
        assert_eq!(
            lines,
            vec![
                (Msg::BatteryLeft, "94%".to_string()),
                (Msg::BatteryRight, "93%".to_string()),
                (Msg::BatteryCase, "40% \u{00b7} Charging".to_string()),
            ]
        );
    }

    #[test]
    fn disconnected_component_is_a_dash_and_missing_ones_are_omitted() {
        let data = vec![
            info(BatteryComponent::Left, 80, BatteryStatus::NotCharging),
            info(BatteryComponent::Case, 0, BatteryStatus::Disconnected),
        ];
        let lines = battery_lines(&data, "Charging");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], (Msg::BatteryCase, "\u{2014}".to_string()));
    }

    #[test]
    fn no_data_gives_no_rows() {
        assert!(battery_lines(&[], "Charging").is_empty());
    }
}
