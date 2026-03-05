use cosmic::iced::Event;
use cosmic::iced::event::Status;
use cosmic::iced::keyboard::{Event as KeyEvent, Key, key::Named};
use cosmic::iced::widget::text::Shaping;
use cosmic::iced::widget::tooltip::Position as TooltipPosition;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{button, column, container, menu, row, scrollable, text};
use cosmic::{Application, Core, Element, Task};
use evalexpr::eval_number;

pub fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default()
        .size(cosmic::iced::Size::new(320.0, 540.0))
        .size_limits(
            cosmic::iced::Limits::NONE
                .min_width(280.0)
                .min_height(460.0),
        );
    cosmic::app::run::<CalcApp>(settings, ())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalcMode {
    Standard,
    Scientific,
    Programmer,
    Rpn,
    Statistics,
}

impl CalcMode {
    #[allow(dead_code)]
    fn label(self) -> &'static str {
        match self {
            CalcMode::Standard => "Std",
            CalcMode::Scientific => "Sci",
            CalcMode::Programmer => "Prog",
            CalcMode::Rpn => "RPN",
            CalcMode::Statistics => "Stat",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    Hex,
    Dec,
    Oct,
    Bin,
}

impl Base {
    fn label(self) -> &'static str {
        match self {
            Base::Hex => "HEX",
            Base::Dec => "DEC",
            Base::Oct => "OCT",
            Base::Bin => "BIN",
        }
    }
    fn radix(self) -> u32 {
        match self {
            Base::Hex => 16,
            Base::Dec => 10,
            Base::Oct => 8,
            Base::Bin => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MenuAction {
    SetMode(CalcMode),
}

impl MenuAction {
    fn message(self) -> Message {
        match self {
            MenuAction::SetMode(m) => Message::SetMode(m),
        }
    }
}

struct CalcApp {
    core: Core,
    mode: CalcMode,
    display: String,
    prev_value: f64,
    current_op: Option<char>,
    new_input: bool,
    history: Vec<String>,
    show_history: bool,
    copied_index: Option<usize>,
    show_panel: bool,
    prog_base: Base,
    rpn_stack: Vec<f64>,
    stat_values: Vec<f64>,
}

#[derive(Debug, Clone)]
enum Message {
    Input(&'static str),
    KeyPressed(KeyEvent),
    SetMode(CalcMode),
    SetBase(Base),
    CopyResult,
    CopyHistoryItem(usize, String),
    ClearToast,
    ToggleHistory,
    ClearHistory,
    TogglePanel,
    ApplyConversion(&'static str, f64),
    InsertConstant(f64),
    StatAdd,
    StatClear,
}

impl Application for CalcApp {
    type Executor = cosmic::executor::Default;
    type Message = Message;
    type Flags = ();
    const APP_ID: &'static str = "com.cosmic.calculator";
    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: ()) -> (Self, Task<cosmic::Action<Message>>) {
        (
            Self {
                core,
                mode: CalcMode::Standard,
                display: "0".to_string(),
                prev_value: 0.0,
                current_op: None,
                new_input: true,
                history: Vec::new(),
                show_history: false,
                copied_index: None,
                show_panel: false,
                prog_base: Base::Dec,
                rpn_stack: Vec::new(),
                stat_values: Vec::new(),
            },
            Task::none(),
        )
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Message> {
        cosmic::iced::event::listen_with(|event, status, _| {
            if status == Status::Captured {
                return None;
            }
            if let Event::Keyboard(key_event @ KeyEvent::KeyPressed { .. }) = event {
                Some(Message::KeyPressed(key_event))
            } else {
                None
            }
        })
    }

    fn update(&mut self, message: Message) -> Task<cosmic::Action<Message>> {
        match message {
            Message::SetMode(m) => {
                self.mode = m;
                self.show_panel = false;
                self.prog_base = Base::Dec;
                self.rpn_stack.clear();
                self.stat_values.clear();
                self.reset_all();
            }
            Message::SetBase(b) => {
                let n = i64::from_str_radix(&self.display, self.prog_base.radix()).unwrap_or(0);
                self.prog_base = b;
                self.display = Self::format_in_base(n, b);
                self.new_input = true;
            }
            Message::Input(input) => match self.mode {
                CalcMode::Standard => self.handle_standard(input),
                CalcMode::Scientific => self.handle_scientific(input),
                CalcMode::Programmer => self.handle_programmer(input),
                CalcMode::Rpn => self.handle_rpn(input),
                CalcMode::Statistics => self.handle_statistics_input(input),
            },
            Message::StatAdd => {
                if let Ok(val) = self.display.parse::<f64>() {
                    self.stat_values.push(val);
                    self.new_input = true;
                    self.display = "0".to_string();
                }
            }
            Message::StatClear => {
                self.stat_values.clear();
                self.reset_all();
            }
            Message::InsertConstant(val) => {
                let formatted = Self::format_result(val);
                if self.mode == CalcMode::Scientific && !self.new_input && self.display != "0" {
                    // Append to expression so user can type e.g. 2*pi
                    self.display.push_str(&formatted);
                } else {
                    self.display = formatted;
                    self.new_input = true;
                }
                self.show_panel = false;
            }
            Message::TogglePanel => {
                self.show_panel = !self.show_panel;
            }
            Message::ApplyConversion(label, factor) => {
                if let Ok(val) = self.display.parse::<f64>() {
                    let res = val * factor;
                    let result_str = Self::format_result(res);
                    self.push_history(&format!("{} {}", val, label), &result_str);
                    self.display = result_str;
                    self.new_input = true;
                    self.current_op = None;
                    self.show_panel = false;
                }
            }
            Message::KeyPressed(event) => {
                // ── Ctrl+1..5: switch mode ────────────────────────────────
                if let KeyEvent::KeyPressed {
                    key: Key::Character(ref c),
                    modifiers,
                    ..
                } = event
                {
                    if modifiers.control() {
                        let new_mode = match c.as_str() {
                            "1" => Some(CalcMode::Standard),
                            "2" => Some(CalcMode::Scientific),
                            "3" => Some(CalcMode::Programmer),
                            "4" => Some(CalcMode::Rpn),
                            "5" => Some(CalcMode::Statistics),
                            _ => None,
                        };
                        if let Some(m) = new_mode {
                            self.mode = m;
                            self.show_panel = false;
                            self.prog_base = Base::Dec;
                            self.rpn_stack.clear();
                            self.stat_values.clear();
                            self.reset_all();
                            return Task::none();
                        }
                    }
                }

                let mapped: Option<&'static str> = match event {
                    KeyEvent::KeyPressed {
                        key: Key::Character(ref c),
                        modifiers,
                        ..
                    } => {
                        // ── a-f for hex digit entry in programmer mode ────
                        if self.mode == CalcMode::Programmer && self.prog_base == Base::Hex {
                            match c.as_str() {
                                "a" => {
                                    self.handle_programmer("A");
                                    return Task::none();
                                }
                                "b" => {
                                    self.handle_programmer("B");
                                    return Task::none();
                                }
                                "c" => {
                                    self.handle_programmer("C");
                                    return Task::none();
                                }
                                "d" => {
                                    self.handle_programmer("D");
                                    return Task::none();
                                }
                                "e" => {
                                    self.handle_programmer("E");
                                    return Task::none();
                                }
                                "f" => {
                                    self.handle_programmer("F");
                                    return Task::none();
                                }
                                _ => {}
                            }
                        }
                        match c.as_str() {
                            "0" => "0",
                            "1" => "1",
                            "2" => "2",
                            "3" => "3",
                            "4" => "4",
                            "5" => "5",
                            "6" => "6",
                            "7" => "7",
                            "9" => "9",
                            "." => ".",
                            "+" => "+",
                            "-" => "-",
                            "*" => "x",
                            "/" => "div",
                            "8" => {
                                if modifiers.shift() {
                                    "x"
                                } else {
                                    "8"
                                }
                            }
                            "=" => {
                                if modifiers.shift() {
                                    "+"
                                } else {
                                    "="
                                }
                            }
                            _ => return Task::none(),
                        }
                        .into()
                    }
                    KeyEvent::KeyPressed {
                        key: Key::Named(named),
                        ..
                    } => match named {
                        Named::Enter => Some("="),
                        Named::Backspace => Some("DEL"),
                        Named::Escape => Some("C"),
                        Named::Delete => Some("CE"),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(input) = mapped {
                    match self.mode {
                        CalcMode::Standard => self.handle_standard(input),
                        CalcMode::Scientific => self.handle_scientific(input),
                        CalcMode::Programmer => self.handle_programmer(input),
                        CalcMode::Rpn => self.handle_rpn(input),
                        CalcMode::Statistics => self.handle_statistics_input(input),
                    }
                }
            }
            Message::ToggleHistory => {
                self.show_history = !self.show_history;
                self.copied_index = None;
                self.show_panel = false;
            }
            Message::ClearHistory => {
                self.history.clear();
                self.copied_index = None;
            }
            Message::CopyResult => {
                let val = self.display.clone();
                return Task::batch([
                    cosmic::iced::clipboard::write::<cosmic::Action<Message>>(val.clone()),
                    cosmic::iced::clipboard::write_primary::<cosmic::Action<Message>>(val),
                ]);
            }
            Message::CopyHistoryItem(idx, entry) => {
                self.copied_index = Some(idx);
                return Task::batch([
                    cosmic::iced::clipboard::write::<cosmic::Action<Message>>(entry.clone()),
                    cosmic::iced::clipboard::write_primary::<cosmic::Action<Message>>(entry),
                    Task::perform(
                        async {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        },
                        |_| Message::ClearToast.into(),
                    ),
                ]);
            }
            Message::ClearToast => {
                self.copied_index = None;
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let (
            rad,
            rad_m,
            pill_acc_bg,
            pill_acc_fg,
            pill_std_bg,
            pill_std_fg,
            std_bg,
            std_fg,
            sug_bg,
            sug_fg,
            des_bg,
            des_fg,
            hist_fg,
            hist_dim,
            disp_bg,
            disp_fg,
            border_clr,
            acc_dim_bg,
        ) = {
            let theme = cosmic::theme::active();
            let ct = theme.cosmic();
            let rad: cosmic::iced::border::Radius = ct.corner_radii.radius_s.into();
            let rad_m: cosmic::iced::border::Radius = ct.corner_radii.radius_m.into();
            let pill_acc_bg: cosmic::iced::Color = ct.accent_color().into();
            let pill_acc_fg: cosmic::iced::Color = ct.on_accent_color().into();
            let pill_std_bg: cosmic::iced::Color = ct.bg_component_color().into();
            let pill_std_fg: cosmic::iced::Color = ct.on_bg_component_color().into();
            let std_bg: cosmic::iced::Color = ct.bg_component_color().into();
            let std_fg: cosmic::iced::Color = ct.on_bg_component_color().into();
            let sug_bg: cosmic::iced::Color = ct.accent_color().into();
            let sug_fg: cosmic::iced::Color = ct.on_accent_color().into();
            let acc: cosmic::iced::Color = ct.accent_color().into();
            let base_c: cosmic::iced::Color = ct.bg_component_color().into();
            let des_bg = cosmic::iced::Color {
                r: base_c.r * 0.70 + acc.r * 0.30,
                g: base_c.g * 0.70 + acc.g * 0.30,
                b: base_c.b * 0.70 + acc.b * 0.30,
                a: 1.0,
            };
            let des_fg: cosmic::iced::Color = ct.on_bg_component_color().into();
            let mut hist_fg: cosmic::iced::Color = ct.on_bg_component_color().into();
            hist_fg.a = 0.8;
            let mut hist_dim: cosmic::iced::Color = ct.on_bg_component_color().into();
            hist_dim.a = 0.3;
            let disp_bg: cosmic::iced::Color = ct.bg_component_color().into();
            let disp_fg: cosmic::iced::Color = ct.on_bg_component_color().into();
            let mut border_clr: cosmic::iced::Color = ct.on_bg_component_color().into();
            border_clr.a = 0.2;
            let acc_dim_bg = cosmic::iced::Color {
                r: base_c.r * 0.85 + acc.r * 0.15,
                g: base_c.g * 0.85 + acc.g * 0.15,
                b: base_c.b * 0.85 + acc.b * 0.15,
                a: 0.5,
            };
            (
                rad,
                rad_m,
                pill_acc_bg,
                pill_acc_fg,
                pill_std_bg,
                pill_std_fg,
                std_bg,
                std_fg,
                sug_bg,
                sug_fg,
                des_bg,
                des_fg,
                hist_fg,
                hist_dim,
                disp_bg,
                disp_fg,
                border_clr,
                acc_dim_bg,
            )
        };

        let calc_btn = |label: &'static str,
                        bg: cosmic::iced::Color,
                        fg: cosmic::iced::Color,
                        radius: cosmic::iced::border::Radius,
                        enabled: bool|
         -> Element<'static, Message> {
            let inner = container(
                text(label)
                    .size(15)
                    .shaping(Shaping::Advanced)
                    .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(
                move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(bg)),
                    text_color: Some(fg),
                    border: cosmic::iced::Border {
                        radius,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            let btn = button::custom(inner)
                .padding(0)
                .width(Length::Fill)
                .height(Length::Fill);
            if enabled {
                btn.on_press(Message::Input(label)).into()
            } else {
                btn.into()
            }
        };

        let d = |l: &'static str| calc_btn(l, std_bg, std_fg, rad, true);
        let o = |l: &'static str| calc_btn(l, sug_bg, sug_fg, rad, true);
        let a = |l: &'static str| calc_btn(l, des_bg, des_fg, rad, true);
        let dim = |l: &'static str| calc_btn(l, acc_dim_bg, std_fg, rad, false);
        let eq = || calc_btn("=", sug_bg, sug_fg, rad, true);

        let hist_active = self.show_history;
        let (hist_btn_bg, hist_btn_fg) = if hist_active {
            (pill_acc_bg, pill_acc_fg)
        } else {
            (pill_std_bg, pill_std_fg)
        };
        let history_btn: Element<'_, Message> = cosmic::iced::widget::tooltip(
            container(
                button::custom(
                    container(
                        text(if hist_active { "< Back" } else { "History" })
                            .size(13)
                            .shaping(Shaping::Advanced)
                            .align_x(Alignment::Center),
                    )
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .padding([6, 0])
                    .width(Length::Fixed(100.0))
                    .style(move |_: &cosmic::Theme| {
                        cosmic::iced::widget::container::Style {
                            background: Some(cosmic::iced::Background::Color(hist_btn_bg)),
                            text_color: Some(hist_btn_fg),
                            border: cosmic::iced::Border {
                                radius: rad_m,
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    }),
                )
                .padding(0)
                .on_press(Message::ToggleHistory),
            )
            .style(|theme: &cosmic::Theme| {
                let c = theme.cosmic();
                cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(
                        c.bg_component_color().into(),
                    )),
                    border: cosmic::iced::Border {
                        radius: c.corner_radii.radius_m.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .padding(4),
            container(
                text("View calculation history")
                    .size(12)
                    .shaping(Shaping::Advanced),
            )
            .padding(6)
            .style(
                move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(disp_bg)),
                    border: cosmic::iced::Border {
                        radius: rad_m,
                        color: border_clr,
                        width: 1.0,
                    },
                    text_color: Some(disp_fg),
                    ..Default::default()
                },
            ),
            TooltipPosition::Bottom,
        )
        .into();

        let content: Element<'_, Message> = if self.show_history {
            self.view_history(
                pill_acc_bg,
                pill_acc_fg,
                pill_std_bg,
                pill_std_fg,
                hist_fg,
                hist_dim,
                rad_m,
            )
        } else {
            match self.mode {
                CalcMode::Standard | CalcMode::Scientific => {
                    self.view_standard_sci(disp_bg, disp_fg, rad_m, &d, &o, &a, &eq)
                }
                CalcMode::Programmer => self.view_programmer(
                    std_fg,
                    sug_bg,
                    sug_fg,
                    des_bg,
                    des_fg,
                    disp_bg,
                    disp_fg,
                    rad,
                    rad_m,
                    pill_acc_bg,
                    pill_acc_fg,
                    pill_std_bg,
                    pill_std_fg,
                    &d,
                    &o,
                    &a,
                    &dim,
                ),
                CalcMode::Rpn => self.view_rpn(
                    sug_bg, sug_fg, des_bg, des_fg, disp_bg, disp_fg, rad_m, &d, &o, &a,
                ),
                CalcMode::Statistics => self.view_statistics(
                    sug_bg,
                    sug_fg,
                    des_bg,
                    des_fg,
                    disp_bg,
                    disp_fg,
                    rad_m,
                    pill_std_bg,
                    pill_std_fg,
                    &d,
                    &o,
                    &a,
                ),
            }
        };

        let panel_active = self.show_panel;
        let (panel_btn_bg, panel_btn_fg) = if panel_active {
            (pill_acc_bg, pill_acc_fg)
        } else {
            (pill_std_bg, pill_std_fg)
        };

        // ── Mode-specific panel label ─────────────────────────────────────
        let panel_label = match self.mode {
            CalcMode::Standard => {
                if panel_active {
                    "Convert ^"
                } else {
                    "Convert v"
                }
            }
            CalcMode::Scientific => {
                if panel_active {
                    "Const ^"
                } else {
                    "Const v"
                }
            }
            CalcMode::Programmer => {
                if panel_active {
                    "Bits ^"
                } else {
                    "Bits v"
                }
            }
            CalcMode::Rpn => {
                if panel_active {
                    "Stack ^"
                } else {
                    "Stack v"
                }
            }
            CalcMode::Statistics => {
                if panel_active {
                    "More ^"
                } else {
                    "More v"
                }
            }
        };

        // ── Shared popup item helpers ─────────────────────────────────────
        let popup_item_bg = pill_std_bg;
        let popup_item_fg = pill_std_fg;
        let popup_row_style = move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
            background: Some(cosmic::iced::Background::Color(popup_item_bg)),
            text_color: Some(popup_item_fg),
            border: cosmic::iced::Border {
                radius: rad_m,
                ..Default::default()
            },
            ..Default::default()
        };
        let popup_panel_style = move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
            background: Some(cosmic::iced::Background::Color(disp_bg)),
            border: cosmic::iced::Border {
                radius: rad_m,
                color: border_clr,
                width: 1.0,
            },
            ..Default::default()
        };

        // ── Build the overlay content for current mode ────────────────────
        let overlay_panel = match self.mode {
            CalcMode::Standard => {
                let conv_btn = |label: &'static str, factor: f64| -> Element<'_, Message> {
                    button::custom(
                        container(
                            text(label)
                                .size(13)
                                .shaping(Shaping::Advanced)
                                .align_x(Alignment::Center),
                        )
                        .width(Length::Fill)
                        .padding([6, 0])
                        .style(popup_row_style),
                    )
                    .padding(0)
                    .width(Length::Fill)
                    .on_press(Message::ApplyConversion(label, factor))
                    .into()
                };
                container(
                    column()
                        .spacing(6)
                        .push(
                            row()
                                .spacing(6)
                                .width(Length::Fill)
                                .push(conv_btn("pt->L", 0.568261))
                                .push(conv_btn("L->pt", 1.759754)),
                        )
                        .push(
                            row()
                                .spacing(6)
                                .width(Length::Fill)
                                .push(conv_btn("gal->L", 4.54609))
                                .push(conv_btn("L->gal", 0.219969)),
                        )
                        .push(
                            row()
                                .spacing(6)
                                .width(Length::Fill)
                                .push(conv_btn("mi->km", 1.60934))
                                .push(conv_btn("km->mi", 0.621371)),
                        )
                        .push(
                            row()
                                .spacing(6)
                                .width(Length::Fill)
                                .push(conv_btn("lb->kg", 0.453592))
                                .push(conv_btn("kg->lb", 2.204622)),
                        ),
                )
                .padding(8)
                .style(popup_panel_style)
            }
            CalcMode::Scientific => {
                let const_btn = |label: &'static str, val: f64| -> Element<'_, Message> {
                    button::custom(
                        container(
                            text(label)
                                .size(13)
                                .shaping(Shaping::Advanced)
                                .align_x(Alignment::Center),
                        )
                        .width(Length::Fill)
                        .padding([6, 0])
                        .style(popup_row_style),
                    )
                    .padding(0)
                    .width(Length::Fill)
                    .on_press(Message::InsertConstant(val))
                    .into()
                };
                container(
                    column()
                        .spacing(6)
                        .push(
                            row()
                                .spacing(6)
                                .width(Length::Fill)
                                .push(const_btn("pi=3.14159", std::f64::consts::PI))
                                .push(const_btn("e=2.71828", std::f64::consts::E)),
                        )
                        .push(
                            row()
                                .spacing(6)
                                .width(Length::Fill)
                                .push(const_btn("phi=1.61803", 1.6180339887498948))
                                .push(const_btn("sqrt2=1.414", std::f64::consts::SQRT_2)),
                        )
                        .push(
                            row()
                                .spacing(6)
                                .width(Length::Fill)
                                .push(const_btn("c=299792458", 299_792_458.0))
                                .push(const_btn("g=9.80665", 9.80665)),
                        )
                        .push(
                            row()
                                .spacing(6)
                                .width(Length::Fill)
                                .push(const_btn("h=6.626e-34", 6.62607015e-34))
                                .push(const_btn("Na=6.022e23", 6.02214076e23)),
                        ),
                )
                .padding(8)
                .style(popup_panel_style)
            }
            CalcMode::Programmer => {
                let base_val =
                    i64::from_str_radix(&self.display, self.prog_base.radix()).unwrap_or(0);
                let info_row = |label: &'static str, val: String| -> Element<'_, Message> {
                    container(
                        row()
                            .spacing(8)
                            .push(
                                text(label)
                                    .size(12)
                                    .shaping(Shaping::Advanced)
                                    .width(Length::Fixed(36.0)),
                            )
                            .push(text(val).size(12).shaping(Shaping::Advanced)),
                    )
                    .width(Length::Fill)
                    .padding([5, 8])
                    .style(popup_row_style)
                    .into()
                };
                // Bit pattern: group into nibbles
                let bits = format!("{:064b}", base_val as u64);
                let nibbles = bits
                    .chars()
                    .enumerate()
                    .fold(String::new(), |mut s, (i, c)| {
                        if i > 0 && i % 4 == 0 {
                            s.push(' ');
                        }
                        s.push(c);
                        s
                    });
                // Trim leading zero-nibbles but keep at least 4 bits
                let trimmed = nibbles.trim_start_matches("0000 ").to_string();
                let trimmed = if trimmed.is_empty() {
                    "0000".to_string()
                } else {
                    trimmed
                };
                container(
                    column()
                        .spacing(4)
                        .push(info_row("HEX", format!("{:X}", base_val)))
                        .push(info_row("DEC", format!("{}", base_val)))
                        .push(info_row("OCT", format!("{:o}", base_val)))
                        .push(info_row("BIN", trimmed)),
                )
                .padding(8)
                .style(popup_panel_style)
            }
            CalcMode::Rpn => {
                let mut col = column().spacing(4);
                if self.rpn_stack.is_empty() {
                    col = col.push(
                        container(text("Stack empty").size(12).shaping(Shaping::Advanced))
                            .width(Length::Fill)
                            .padding([5, 8])
                            .style(popup_row_style),
                    );
                } else {
                    for (i, val) in self.rpn_stack.iter().enumerate().rev() {
                        let label = format!("{}: {}", i + 1, Self::format_result(*val));
                        col = col.push(
                            container(text(label).size(12).shaping(Shaping::Advanced))
                                .width(Length::Fill)
                                .padding([5, 8])
                                .style(popup_row_style),
                        );
                    }
                }
                container(col).padding(8).style(popup_panel_style)
            }
            CalcMode::Statistics => {
                let n = self.stat_values.len();
                let sum = self.stat_values.iter().sum::<f64>();
                let mean = if n > 0 { sum / n as f64 } else { 0.0 };
                let var = if n > 1 {
                    self.stat_values
                        .iter()
                        .map(|v| (v - mean).powi(2))
                        .sum::<f64>()
                        / (n - 1) as f64
                } else {
                    0.0
                };
                let min = self
                    .stat_values
                    .iter()
                    .cloned()
                    .fold(f64::INFINITY, f64::min);
                let max = self
                    .stat_values
                    .iter()
                    .cloned()
                    .fold(f64::NEG_INFINITY, f64::max);
                let range = if n > 0 { max - min } else { 0.0 };
                let mut sorted = self.stat_values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let median = if n == 0 {
                    0.0
                } else if n % 2 == 0 {
                    (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
                } else {
                    sorted[n / 2]
                };

                let stat_row = |label: &'static str, val: String| -> Element<'_, Message> {
                    container(
                        row()
                            .spacing(8)
                            .push(
                                text(label)
                                    .size(12)
                                    .shaping(Shaping::Advanced)
                                    .width(Length::Fixed(52.0)),
                            )
                            .push(text(val).size(12).shaping(Shaping::Advanced)),
                    )
                    .width(Length::Fill)
                    .padding([5, 8])
                    .style(popup_row_style)
                    .into()
                };
                container(
                    column()
                        .spacing(4)
                        .push(stat_row("n", format!("{}", n)))
                        .push(stat_row("sum", Self::format_result(sum)))
                        .push(stat_row("mean", Self::format_result(mean)))
                        .push(stat_row("median", Self::format_result(median)))
                        .push(stat_row("var", Self::format_result(var)))
                        .push(stat_row("min", Self::format_result(min)))
                        .push(stat_row("max", Self::format_result(max)))
                        .push(stat_row("range", Self::format_result(range))),
                )
                .padding(8)
                .style(popup_panel_style)
            }
        };

        let toggle_btn = button::custom(
            container(
                text(panel_label)
                    .size(13)
                    .shaping(Shaping::Advanced)
                    .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding([6, 0])
            .style(
                move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(panel_btn_bg)),
                    text_color: Some(panel_btn_fg),
                    border: cosmic::iced::Border {
                        radius: rad_m,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::TogglePanel);

        let conv_toggle: Element<'_, Message> = {
            let p = cosmic::widget::popover(
                container(toggle_btn)
                    .style(|theme: &cosmic::Theme| {
                        let c = theme.cosmic();
                        cosmic::iced::widget::container::Style {
                            background: Some(cosmic::iced::Background::Color(
                                c.bg_component_color().into(),
                            )),
                            border: cosmic::iced::Border {
                                radius: c.corner_radii.radius_m.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })
                    .padding(4)
                    .width(Length::Fill),
            )
            .position(cosmic::widget::popover::Position::Bottom);
            if panel_active {
                p.popup(overlay_panel).into()
            } else {
                p.into()
            }
        };

        let top_bar = row()
            .spacing(8)
            .width(Length::Fill)
            .push(conv_toggle)
            .push(history_btn);
        let root_col = column()
            .spacing(10)
            .padding(12)
            .push(top_bar)
            .push(content)
            .align_x(Alignment::Center);
        container(root_col)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let current_mode_label = self.mode.label();
        let mode_btn = menu::menu_button(vec![
            text(current_mode_label).shaping(Shaping::Advanced).into(),
        ]);
        let mode_label: Element<'static, Message> = cosmic::iced::widget::tooltip(
            mode_btn,
            container(
                column()
                    .spacing(4)
                    .push(text("Switch mode:").size(12).shaping(Shaping::Advanced))
                    .push(text("Ctrl+1  Standard").size(11).shaping(Shaping::Advanced))
                    .push(
                        text("Ctrl+2  Scientific")
                            .size(11)
                            .shaping(Shaping::Advanced),
                    )
                    .push(
                        text("Ctrl+3  Programmer")
                            .size(11)
                            .shaping(Shaping::Advanced),
                    )
                    .push(text("Ctrl+4  RPN").size(11).shaping(Shaping::Advanced))
                    .push(
                        text("Ctrl+5  Statistics")
                            .size(11)
                            .shaping(Shaping::Advanced),
                    ),
            )
            .padding(8),
            TooltipPosition::Bottom,
        )
        .into();

        let item = |label: &'static str, action: MenuAction| -> menu::Tree<Message> {
            menu::Tree::new(Element::<'static, Message>::from(
                menu::menu_button(vec![text(label).shaping(Shaping::Advanced).into()])
                    .on_press(action.message()),
            ))
        };

        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            mode_label,
            vec![
                item("Standard", MenuAction::SetMode(CalcMode::Standard)),
                item("Scientific", MenuAction::SetMode(CalcMode::Scientific)),
                item("Programmer", MenuAction::SetMode(CalcMode::Programmer)),
                item("RPN", MenuAction::SetMode(CalcMode::Rpn)),
                item("Statistics", MenuAction::SetMode(CalcMode::Statistics)),
            ],
        )]);
        vec![menu_bar.into()]
    }
}

// ── View helpers ──────────────────────────────────────────────────────────────

impl CalcApp {
    fn view_history(
        &self,
        pill_acc_bg: cosmic::iced::Color,
        pill_acc_fg: cosmic::iced::Color,
        pill_std_bg: cosmic::iced::Color,
        pill_std_fg: cosmic::iced::Color,
        hist_fg: cosmic::iced::Color,
        hist_dim: cosmic::iced::Color,
        rad_m: cosmic::iced::border::Radius,
    ) -> Element<'_, Message> {
        let mut col = column().spacing(8).padding([4, 0]).width(Length::Fill);
        col = col.push(
            button::custom(
                container(
                    text("Clear History")
                        .size(13)
                        .shaping(Shaping::Advanced)
                        .align_x(Alignment::Center),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .padding([6, 0])
                .style(move |_: &cosmic::Theme| {
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Background::Color(pill_std_bg)),
                        text_color: Some(pill_std_fg),
                        border: cosmic::iced::Border {
                            radius: rad_m,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(Message::ClearHistory),
        );
        if self.history.is_empty() {
            col = col.push(
                container(
                    text("No history yet")
                        .size(13)
                        .shaping(Shaping::Advanced)
                        .align_x(Alignment::Center),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .style(move |_: &cosmic::Theme| {
                    cosmic::iced::widget::container::Style {
                        text_color: Some(hist_dim),
                        ..Default::default()
                    }
                }),
            );
        } else {
            for (i, entry) in self.history.iter().enumerate().rev() {
                let copy_val = entry.clone();
                let is_copied = self.copied_index == Some(i);
                let (txt, align, text_clr, bg_clr) = if is_copied {
                    (
                        "Copied to clipboard!".to_string(),
                        Alignment::Center,
                        pill_acc_fg,
                        Some(cosmic::iced::Background::Color(pill_acc_bg)),
                    )
                } else {
                    (entry.clone(), Alignment::End, hist_fg, None)
                };
                col = col.push(
                    button::custom(
                        container(text(txt).size(13).shaping(Shaping::Advanced).align_x(align))
                            .width(Length::Fill)
                            .align_x(align)
                            .padding([6, 10])
                            .style(move |_: &cosmic::Theme| {
                                cosmic::iced::widget::container::Style {
                                    background: bg_clr,
                                    text_color: Some(text_clr),
                                    border: cosmic::iced::Border {
                                        radius: rad_m,
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }
                            }),
                    )
                    .padding(0)
                    .width(Length::Fill)
                    .on_press(Message::CopyHistoryItem(i, copy_val)),
                );
            }
        }
        container(scrollable(col))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([4, 8])
            .into()
    }

    fn view_standard_sci<'a: 'b, 'b>(
        &'b self,
        disp_bg: cosmic::iced::Color,
        disp_fg: cosmic::iced::Color,
        rad_m: cosmic::iced::border::Radius,
        d: &impl Fn(&'static str) -> Element<'a, Message>,
        o: &impl Fn(&'static str) -> Element<'a, Message>,
        a: &impl Fn(&'static str) -> Element<'a, Message>,
        eq: &impl Fn() -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let font_size: u16 = if self.display.len() > 12 { 28 } else { 42 };
        let display_str: String = self.display.clone();
        let display_area = button::custom(
            container(
                text(display_str)
                    .size(font_size)
                    .shaping(Shaping::Advanced)
                    .align_x(Alignment::End),
            )
            .width(Length::Fill)
            .align_x(Alignment::End)
            .style(
                move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(disp_bg)),
                    text_color: Some(disp_fg),
                    border: cosmic::iced::Border {
                        radius: rad_m,
                        color: cosmic::iced::Color::TRANSPARENT,
                        width: 0.0,
                    },
                    ..Default::default()
                },
            ),
        )
        .padding(0)
        .on_press(Message::CopyResult)
        .width(Length::Fill);

        let r1 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(a("CE"))
            .push(a("C"))
            .push(a("DEL"))
            .push(o("div"));
        let r2 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("7"))
            .push(d("8"))
            .push(d("9"))
            .push(o("x"));
        let r3 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("4"))
            .push(d("5"))
            .push(d("6"))
            .push(o("-"));
        let r4 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("1"))
            .push(d("2"))
            .push(d("3"))
            .push(o("+"));
        let r5 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("+/-"))
            .push(d("0"))
            .push(d("."))
            .push(eq());
        let grid = column()
            .spacing(6)
            .height(Length::Fill)
            .push(r1)
            .push(r2)
            .push(r3)
            .push(r4)
            .push(r5);
        column()
            .spacing(10)
            .align_x(Alignment::Center)
            .height(Length::Fill)
            .push(display_area)
            .push(grid)
            .into()
    }

    #[allow(clippy::too_many_arguments)]
    fn view_programmer<'a: 'b, 'b>(
        &'b self,
        std_fg: cosmic::iced::Color,
        sug_bg: cosmic::iced::Color,
        sug_fg: cosmic::iced::Color,
        des_bg: cosmic::iced::Color,
        des_fg: cosmic::iced::Color,
        disp_bg: cosmic::iced::Color,
        disp_fg: cosmic::iced::Color,
        rad: cosmic::iced::border::Radius,
        rad_m: cosmic::iced::border::Radius,
        pill_acc_bg: cosmic::iced::Color,
        pill_acc_fg: cosmic::iced::Color,
        pill_std_bg: cosmic::iced::Color,
        pill_std_fg: cosmic::iced::Color,
        d: &impl Fn(&'static str) -> Element<'a, Message>,
        o: &impl Fn(&'static str) -> Element<'a, Message>,
        a: &impl Fn(&'static str) -> Element<'a, Message>,
        dim: &impl Fn(&'static str) -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let base = self.prog_base;
        let dec_val = i64::from_str_radix(&self.display, base.radix()).unwrap_or(0);
        let font_size: u16 = if self.display.len() > 10 { 24 } else { 36 };
        let prog_display = self.display.clone();

        let display_area = button::custom(
            container(
                column()
                    .spacing(2)
                    .push(
                        text(prog_display)
                            .size(font_size)
                            .shaping(Shaping::Advanced)
                            .align_x(Alignment::End),
                    )
                    .push(
                        text(format!("= {} (DEC)", dec_val))
                            .size(11)
                            .shaping(Shaping::Advanced)
                            .align_x(Alignment::End),
                    ),
            )
            .width(Length::Fill)
            .align_x(Alignment::End)
            .style(
                move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(disp_bg)),
                    text_color: Some(disp_fg),
                    border: cosmic::iced::Border {
                        radius: rad_m,
                        color: cosmic::iced::Color::TRANSPARENT,
                        width: 0.0,
                    },
                    ..Default::default()
                },
            ),
        )
        .padding(0)
        .on_press(Message::CopyResult)
        .width(Length::Fill);

        let base_pill = |b: Base| -> Element<'_, Message> {
            let active = b == base;
            let (bg, fg) = if active {
                (pill_acc_bg, pill_acc_fg)
            } else {
                (pill_std_bg, pill_std_fg)
            };
            button::custom(
                container(
                    text(b.label())
                        .size(12)
                        .shaping(Shaping::Advanced)
                        .align_x(Alignment::Center),
                )
                .padding([4, 0])
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .style(move |_: &cosmic::Theme| {
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Background::Color(bg)),
                        text_color: Some(fg),
                        border: cosmic::iced::Border {
                            radius: rad_m,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(Message::SetBase(b))
            .into()
        };

        let base_row = row()
            .spacing(4)
            .width(Length::Fill)
            .push(base_pill(Base::Hex))
            .push(base_pill(Base::Dec))
            .push(base_pill(Base::Oct))
            .push(base_pill(Base::Bin));

        let hex_ok = base == Base::Hex;
        let oct_ok = hex_ok || base == Base::Dec || base == Base::Oct;
        let dec_ok = hex_ok || base == Base::Dec;

        let hex_digit = |l: &'static str| -> Element<'_, Message> {
            let (bg, fg) = if hex_ok {
                (sug_bg, sug_fg)
            } else {
                (des_bg, des_fg)
            };
            let btn = button::custom(
                container(
                    text(l)
                        .size(15)
                        .shaping(Shaping::Advanced)
                        .align_x(Alignment::Center),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_: &cosmic::Theme| {
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Background::Color(bg)),
                        text_color: Some(fg),
                        border: cosmic::iced::Border {
                            radius: rad,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            )
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill);
            if hex_ok {
                btn.on_press(Message::Input(l)).into()
            } else {
                btn.into()
            }
        };

        let bw_btn = |label: &'static str| -> Element<'_, Message> {
            button::custom(
                container(
                    text(label)
                        .size(13)
                        .shaping(Shaping::Advanced)
                        .align_x(Alignment::Center),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_: &cosmic::Theme| {
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Background::Color(des_bg)),
                        text_color: Some(std_fg),
                        border: cosmic::iced::Border {
                            radius: rad,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            )
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Message::Input(label))
            .into()
        };

        let maybe =
            |l: &'static str, ok: bool| -> Element<'_, Message> { if ok { d(l) } else { dim(l) } };

        let hex_r1 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(hex_digit("A"))
            .push(hex_digit("B"))
            .push(hex_digit("C"))
            .push(hex_digit("D"));
        let hex_r2 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(hex_digit("E"))
            .push(hex_digit("F"))
            .push(bw_btn("<<"))
            .push(bw_btn(">>"));
        let r1 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(a("CE"))
            .push(a("C"))
            .push(a("DEL"))
            .push(bw_btn("NOT"));
        let r2 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(maybe("7", oct_ok))
            .push(maybe("8", dec_ok))
            .push(maybe("9", dec_ok))
            .push(bw_btn("AND"));
        let r3 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("4"))
            .push(d("5"))
            .push(d("6"))
            .push(bw_btn("OR"));
        let r4 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("1"))
            .push(d("2"))
            .push(d("3"))
            .push(bw_btn("XOR"));
        let r5 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(o("div"))
            .push(d("0"))
            .push(o("x"))
            .push(o("="));

        column()
            .spacing(6)
            .align_x(Alignment::Center)
            .height(Length::Fill)
            .push(display_area)
            .push(base_row)
            .push(
                column()
                    .spacing(6)
                    .height(Length::Fill)
                    .push(hex_r1)
                    .push(hex_r2)
                    .push(r1)
                    .push(r2)
                    .push(r3)
                    .push(r4)
                    .push(r5),
            )
            .into()
    }

    #[allow(clippy::too_many_arguments)]
    fn view_rpn<'a: 'b, 'b>(
        &'b self,
        sug_bg: cosmic::iced::Color,
        sug_fg: cosmic::iced::Color,
        des_bg: cosmic::iced::Color,
        des_fg: cosmic::iced::Color,
        disp_bg: cosmic::iced::Color,
        disp_fg: cosmic::iced::Color,
        rad_m: cosmic::iced::border::Radius,
        d: &impl Fn(&'static str) -> Element<'a, Message>,
        o: &impl Fn(&'static str) -> Element<'a, Message>,
        a: &impl Fn(&'static str) -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let font_size: u16 = if self.display.len() > 12 { 28 } else { 38 };
        let stack_items: Vec<String> = self
            .rpn_stack
            .iter()
            .rev()
            .take(3)
            .map(|v| Self::format_result(*v))
            .collect();
        let stack_str = if stack_items.is_empty() {
            "Stack empty".to_string()
        } else {
            stack_items
                .iter()
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .join("  |  ")
        };
        let rpn_display = self.display.clone();

        let display_area = button::custom(
            container(
                column()
                    .spacing(2)
                    .push(text(stack_str).size(11).shaping(Shaping::Advanced))
                    .push(
                        text(rpn_display)
                            .size(font_size)
                            .shaping(Shaping::Advanced)
                            .align_x(Alignment::End),
                    ),
            )
            .width(Length::Fill)
            .align_x(Alignment::End)
            .style(
                move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(disp_bg)),
                    text_color: Some(disp_fg),
                    border: cosmic::iced::Border {
                        radius: rad_m,
                        color: cosmic::iced::Color::TRANSPARENT,
                        width: 0.0,
                    },
                    ..Default::default()
                },
            ),
        )
        .padding(0)
        .on_press(Message::CopyResult)
        .width(Length::Fill);

        let special = |label: &'static str,
                       bg: cosmic::iced::Color,
                       fg: cosmic::iced::Color,
                       msg: Message|
         -> Element<'_, Message> {
            button::custom(
                container(
                    text(label)
                        .size(13)
                        .shaping(Shaping::Advanced)
                        .align_x(Alignment::Center),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_: &cosmic::Theme| {
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Background::Color(bg)),
                        text_color: Some(fg),
                        border: cosmic::iced::Border {
                            radius: rad_m,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            )
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(msg)
            .into()
        };

        let enter_btn = special("Enter", sug_bg, sug_fg, Message::Input("ENTER"));
        let drop_btn = special("Drop", des_bg, des_fg, Message::Input("DROP"));

        let r1 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(a("CE"))
            .push(a("C"))
            .push(drop_btn)
            .push(enter_btn);
        let r2 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("7"))
            .push(d("8"))
            .push(d("9"))
            .push(o("div"));
        let r3 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("4"))
            .push(d("5"))
            .push(d("6"))
            .push(o("x"));
        let r4 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("1"))
            .push(d("2"))
            .push(d("3"))
            .push(o("-"));
        let r5 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("+/-"))
            .push(d("0"))
            .push(d("."))
            .push(o("+"));
        let grid = column()
            .spacing(6)
            .height(Length::Fill)
            .push(r1)
            .push(r2)
            .push(r3)
            .push(r4)
            .push(r5);

        column()
            .spacing(10)
            .align_x(Alignment::Center)
            .height(Length::Fill)
            .push(display_area)
            .push(grid)
            .into()
    }

    #[allow(clippy::too_many_arguments)]
    fn view_statistics<'a: 'b, 'b>(
        &'b self,
        sug_bg: cosmic::iced::Color,
        sug_fg: cosmic::iced::Color,
        des_bg: cosmic::iced::Color,
        des_fg: cosmic::iced::Color,
        disp_bg: cosmic::iced::Color,
        disp_fg: cosmic::iced::Color,
        rad_m: cosmic::iced::border::Radius,
        pill_std_bg: cosmic::iced::Color,
        pill_std_fg: cosmic::iced::Color,
        d: &impl Fn(&'static str) -> Element<'a, Message>,
        _o: &impl Fn(&'static str) -> Element<'a, Message>,
        a: &impl Fn(&'static str) -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let n = self.stat_values.len();
        let sum = self.stat_values.iter().sum::<f64>();
        let mean = if n > 0 { sum / n as f64 } else { 0.0 };
        let var = if n > 1 {
            self.stat_values
                .iter()
                .map(|v| (v - mean).powi(2))
                .sum::<f64>()
                / (n - 1) as f64
        } else {
            0.0
        };
        let sd = var.sqrt();
        let stats_text = if n == 0 {
            "Enter values, press Add".to_string()
        } else {
            format!(
                "n={}  sum={}  mean={}  sd={}",
                n,
                Self::format_result(sum),
                Self::format_result(mean),
                Self::format_result(sd)
            )
        };

        let font_size: u16 = if self.display.len() > 12 { 28 } else { 42 };
        let stat_display = self.display.clone();
        let display_area = button::custom(
            container(
                column()
                    .spacing(4)
                    .push(text(stats_text).size(11).shaping(Shaping::Advanced))
                    .push(
                        text(stat_display)
                            .size(font_size)
                            .shaping(Shaping::Advanced)
                            .align_x(Alignment::End),
                    ),
            )
            .width(Length::Fill)
            .align_x(Alignment::End)
            .style(
                move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(disp_bg)),
                    text_color: Some(disp_fg),
                    border: cosmic::iced::Border {
                        radius: rad_m,
                        color: cosmic::iced::Color::TRANSPARENT,
                        width: 0.0,
                    },
                    ..Default::default()
                },
            ),
        )
        .padding(0)
        .on_press(Message::CopyResult)
        .width(Length::Fill);

        let action_btn = |label: &'static str,
                          msg: Message,
                          bg: cosmic::iced::Color,
                          fg: cosmic::iced::Color|
         -> Element<'_, Message> {
            button::custom(
                container(
                    text(label)
                        .size(13)
                        .shaping(Shaping::Advanced)
                        .align_x(Alignment::Center),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_: &cosmic::Theme| {
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Background::Color(bg)),
                        text_color: Some(fg),
                        border: cosmic::iced::Border {
                            radius: rad_m,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            )
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(msg)
            .into()
        };

        let values_str = if self.stat_values.is_empty() {
            "No values yet".to_string()
        } else {
            self.stat_values
                .iter()
                .map(|v| Self::format_result(*v))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let values_label = container(text(values_str).size(11).shaping(Shaping::Advanced))
            .width(Length::Fill)
            .padding([4, 6])
            .style(
                move |_: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    text_color: Some(pill_std_fg),
                    background: Some(cosmic::iced::Background::Color(pill_std_bg)),
                    border: cosmic::iced::Border {
                        radius: rad_m,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );

        let r1 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(a("CE"))
            .push(a("C"))
            .push(a("DEL"))
            .push(action_btn("Clr", Message::StatClear, des_bg, des_fg));
        let r2 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("7"))
            .push(d("8"))
            .push(d("9"))
            .push(d("DEL"));
        let r3 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("4"))
            .push(d("5"))
            .push(d("6"))
            .push(d("+/-"));
        let r4 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("1"))
            .push(d("2"))
            .push(d("3"))
            .push(d("."));
        let r5 = row()
            .spacing(6)
            .height(Length::Fill)
            .push(d("CE"))
            .push(d("0"))
            .push(d("C"))
            .push(action_btn("Add", Message::StatAdd, sug_bg, sug_fg));
        let grid = column()
            .spacing(6)
            .height(Length::Fill)
            .push(r1)
            .push(r2)
            .push(r3)
            .push(r4)
            .push(r5);

        column()
            .spacing(6)
            .align_x(Alignment::Center)
            .height(Length::Fill)
            .push(display_area)
            .push(values_label)
            .push(grid)
            .into()
    }
}

// ── Logic ─────────────────────────────────────────────────────────────────────

impl CalcApp {
    fn reset_all(&mut self) {
        self.display = "0".to_string();
        self.prev_value = 0.0;
        self.current_op = None;
        self.new_input = true;
    }
    fn clear_entry(&mut self) {
        self.display = "0".to_string();
        self.new_input = true;
    }
    fn push_history(&mut self, expr: &str, result: &str) {
        if self.history.len() >= 500 {
            self.history.remove(0);
        }
        self.history.push(format!("{} = {}", expr, result));
    }
    fn format_result(val: f64) -> String {
        if val.is_nan() || val.is_infinite() {
            return "Error".to_string();
        }
        if val.fract() == 0.0 && val.abs() < 1e15 {
            format!("{}", val as i64)
        } else {
            format!("{:.10}", val)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        }
    }
    fn format_in_base(n: i64, base: Base) -> String {
        match base {
            Base::Hex => format!("{:X}", n),
            Base::Dec => format!("{}", n),
            Base::Oct => format!("{:o}", n),
            Base::Bin => format!("{:b}", n),
        }
    }
    fn apply_op(op: char, a: f64, b: f64) -> f64 {
        match op {
            '+' => a + b,
            '-' => a - b,
            '*' => a * b,
            '/' => {
                if b != 0.0 {
                    a / b
                } else {
                    f64::INFINITY
                }
            }
            _ => b,
        }
    }
    fn try_commit_chain(&mut self) -> Option<f64> {
        let op = self.current_op?;
        let cur = self.display.parse::<f64>().ok()?;
        if self.new_input {
            return None;
        }
        Some(Self::apply_op(op, self.prev_value, cur))
    }

    fn handle_standard(&mut self, input: &str) {
        match input {
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                if self.new_input {
                    self.display = input.to_string();
                    self.new_input = false;
                } else {
                    self.display.push_str(input);
                }
            }
            "." => {
                if self.new_input {
                    self.display = "0.".to_string();
                    self.new_input = false;
                } else if !self.display.contains('.') {
                    self.display.push('.');
                }
            }
            "+/-" => {
                if let Ok(v) = self.display.parse::<f64>() {
                    self.display = Self::format_result(-v);
                }
            }
            "DEL" => {
                self.display.pop();
                if self.display.is_empty() || self.display == "-" {
                    self.display = "0".to_string();
                }
            }
            "+" | "-" | "x" | "div" => {
                let op = match input {
                    "x" => '*',
                    "div" => '/',
                    o => o.chars().next().unwrap_or('+'),
                };
                if let Some(res) = self.try_commit_chain() {
                    if res.is_infinite() || res.is_nan() {
                        self.display = "Error: div/0".to_string();
                        self.current_op = None;
                        self.new_input = true;
                        return;
                    }
                    self.display = Self::format_result(res);
                    self.prev_value = res;
                } else if let Ok(v) = self.display.parse::<f64>() {
                    self.prev_value = v;
                }
                self.current_op = Some(op);
                self.new_input = true;
            }
            "=" => {
                if let Some(res) = self.try_commit_chain() {
                    let lhs = self.prev_value;
                    let rhs = self.display.parse::<f64>().unwrap_or(0.0);
                    let op_sym = match self.current_op {
                        Some('+') => "+",
                        Some('-') => "-",
                        Some('*') => "x",
                        Some('/') => "div",
                        _ => "?",
                    };
                    let result = if res.is_infinite() || res.is_nan() {
                        "Error: div/0".to_string()
                    } else {
                        Self::format_result(res)
                    };
                    self.push_history(&format!("{} {} {}", lhs, op_sym, rhs), &result);
                    self.display = result;
                    self.current_op = None;
                    self.new_input = true;
                }
            }
            "CE" => self.clear_entry(),
            "C" => self.reset_all(),
            _ => {}
        }
    }

    fn handle_scientific(&mut self, input: &str) {
        match input {
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "." | "+" | "-" => {
                if self.new_input || self.display == "0" {
                    self.display = input.to_string();
                    self.new_input = false;
                } else {
                    self.display.push_str(input);
                }
            }
            "x" => self.sci_push('*'),
            "div" => self.sci_push('/'),
            "DEL" => {
                self.display.pop();
                if self.display.is_empty() || self.display == "-" {
                    self.display = "0".to_string();
                }
            }
            "CE" => self.clear_entry(),
            "C" => self.reset_all(),
            "=" => {
                let expr = self.display.clone();
                let result = match eval_number(&expr) {
                    Ok(r) => Self::format_result(r),
                    Err(_) => "Error".to_string(),
                };
                self.push_history(&expr, &result);
                self.display = result;
                self.new_input = true;
            }
            _ => {}
        }
    }
    fn sci_push(&mut self, op: char) {
        if self.new_input || self.display == "0" {
            self.display = op.to_string();
            self.new_input = false;
        } else {
            self.display.push(op);
        }
    }

    fn handle_programmer(&mut self, input: &str) {
        let base = self.prog_base;
        let valid = |ch: &str| match base {
            Base::Bin => matches!(ch, "0" | "1"),
            Base::Oct => matches!(ch, "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7"),
            Base::Dec => matches!(
                ch,
                "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
            ),
            Base::Hex => matches!(
                ch,
                "0" | "1"
                    | "2"
                    | "3"
                    | "4"
                    | "5"
                    | "6"
                    | "7"
                    | "8"
                    | "9"
                    | "A"
                    | "B"
                    | "C"
                    | "D"
                    | "E"
                    | "F"
            ),
        };
        match input {
            d if valid(d) => {
                if self.new_input || self.display == "0" {
                    self.display = d.to_string();
                    self.new_input = false;
                } else {
                    self.display.push_str(d);
                }
            }
            "DEL" => {
                self.display.pop();
                if self.display.is_empty() {
                    self.display = "0".to_string();
                }
            }
            "CE" => self.clear_entry(),
            "C" => self.reset_all(),
            "x" | "div" | "+" | "-" => {
                let op = match input {
                    "x" => '*',
                    "div" => '/',
                    o => o.chars().next().unwrap_or('+'),
                };
                if let Ok(n) = i64::from_str_radix(&self.display, base.radix()) {
                    self.prev_value = n as f64;
                }
                self.current_op = Some(op);
                self.new_input = true;
            }
            "NOT" => {
                if let Ok(n) = i64::from_str_radix(&self.display, base.radix()) {
                    let r = Self::format_in_base(!n, base);
                    self.push_history(&format!("NOT {}", self.display), &r);
                    self.display = r;
                    self.new_input = true;
                }
            }
            "AND" | "OR" | "XOR" | "<<" | ">>" => {
                if let Ok(n) = i64::from_str_radix(&self.display, base.radix()) {
                    self.prev_value = n as f64;
                }
                self.current_op = Some(match input {
                    "AND" => '&',
                    "OR" => '|',
                    "XOR" => '^',
                    "<<" => '<',
                    ">>" => '>',
                    _ => '&',
                });
                self.new_input = true;
            }
            "+/-" => {
                if let Ok(n) = i64::from_str_radix(&self.display, base.radix()) {
                    let neg = n.wrapping_neg();
                    let r = Self::format_in_base(neg, base);
                    self.push_history(&format!("neg {}", self.display), &r);
                    self.display = r;
                    self.new_input = true;
                }
            }
            "=" => {
                if let (Some(op), Ok(rhs)) = (
                    self.current_op,
                    i64::from_str_radix(&self.display, base.radix()),
                ) {
                    let lhs = self.prev_value as i64;
                    let result = match op {
                        '+' => lhs.wrapping_add(rhs),
                        '-' => lhs.wrapping_sub(rhs),
                        '*' => lhs.wrapping_mul(rhs),
                        '/' => {
                            if rhs != 0 {
                                lhs / rhs
                            } else {
                                0
                            }
                        }
                        '&' => lhs & rhs,
                        '|' => lhs | rhs,
                        '^' => lhs ^ rhs,
                        '<' => lhs << (rhs & 63),
                        '>' => lhs >> (rhs & 63),
                        _ => rhs,
                    };
                    let op_label = match op {
                        '+' => "+",
                        '-' => "-",
                        '*' => "x",
                        '/' => "div",
                        '&' => "AND",
                        '|' => "OR",
                        '^' => "XOR",
                        '<' => "<<",
                        '>' => ">>",
                        _ => "?",
                    };
                    let r = Self::format_in_base(result, base);
                    self.push_history(
                        &format!(
                            "{} {} {} ({})",
                            Self::format_in_base(lhs, base),
                            op_label,
                            Self::format_in_base(rhs, base),
                            base.label()
                        ),
                        &r,
                    );
                    self.display = r;
                    self.current_op = None;
                    self.new_input = true;
                }
            }
            _ => {}
        }
    }

    fn handle_rpn(&mut self, input: &str) {
        match input {
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                if self.new_input {
                    self.display = input.to_string();
                    self.new_input = false;
                } else {
                    self.display.push_str(input);
                }
            }
            "." => {
                if self.new_input {
                    self.display = "0.".to_string();
                    self.new_input = false;
                } else if !self.display.contains('.') {
                    self.display.push('.');
                }
            }
            "+/-" => {
                if let Ok(v) = self.display.parse::<f64>() {
                    self.display = Self::format_result(-v);
                }
            }
            "DEL" => {
                self.display.pop();
                if self.display.is_empty() || self.display == "-" {
                    self.display = "0".to_string();
                }
            }
            "ENTER" | "=" => {
                if let Ok(val) = self.display.parse::<f64>() {
                    self.rpn_stack.push(val);
                    self.new_input = true;
                    self.display = "0".to_string();
                }
            }
            "DROP" => {
                self.rpn_stack.pop();
            }
            "+" | "-" | "x" | "div" => {
                if !self.new_input {
                    if let Ok(v) = self.display.parse::<f64>() {
                        self.rpn_stack.push(v);
                    }
                }
                if self.rpn_stack.len() >= 2 {
                    let b = self.rpn_stack.pop().unwrap();
                    let a = self.rpn_stack.pop().unwrap();
                    let op = match input {
                        "x" => '*',
                        "div" => '/',
                        o => o.chars().next().unwrap_or('+'),
                    };
                    let res = Self::apply_op(op, a, b);
                    let r = Self::format_result(res);
                    self.push_history(
                        &format!(
                            "{} {} {}",
                            Self::format_result(a),
                            input,
                            Self::format_result(b)
                        ),
                        &r,
                    );
                    self.rpn_stack.push(res);
                    self.display = r;
                    self.new_input = true;
                }
            }
            "CE" => self.clear_entry(),
            "C" => {
                self.rpn_stack.clear();
                self.reset_all();
            }
            _ => {}
        }
    }

    fn handle_statistics_input(&mut self, input: &str) {
        match input {
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                if self.new_input {
                    self.display = input.to_string();
                    self.new_input = false;
                } else {
                    self.display.push_str(input);
                }
            }
            "." => {
                if self.new_input {
                    self.display = "0.".to_string();
                    self.new_input = false;
                } else if !self.display.contains('.') {
                    self.display.push('.');
                }
            }
            "+/-" => {
                if let Ok(v) = self.display.parse::<f64>() {
                    self.display = Self::format_result(-v);
                }
            }
            "DEL" => {
                self.display.pop();
                if self.display.is_empty() || self.display == "-" {
                    self.display = "0".to_string();
                }
            }
            "CE" => self.clear_entry(),
            "C" => self.reset_all(),
            _ => {}
        }
    }
}
