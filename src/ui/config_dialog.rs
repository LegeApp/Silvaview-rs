use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use iced::widget::{button, checkbox, column, container, row, slider, text, text_input};
use iced::{application, window, Element, Length, Task, Theme};

use crate::layout::LayoutConfig;
use crate::render::cushion::CushionConfig;

#[derive(Clone)]
pub struct DialogResult {
    pub scan_path: PathBuf,
    pub layout: LayoutConfig,
    pub cushion: CushionConfig,
    pub show_labels: bool,
    pub label_font_scale: f32,
    pub label_font_path: String,
}

pub fn run_config_dialog(
    title: &str,
    initial: DialogResult,
    show_path_input: bool,
) -> Option<DialogResult> {
    let output = Arc::new(Mutex::new(None));
    let output_for_app = output.clone();
    let initial_for_app = initial.clone();
    let title_owned = title.to_string();

    let run = application(
        move |_state: &ConfigDialog| title_owned.clone(),
        move |state: &mut ConfigDialog, message: Message| state.update(message),
        view,
    )
    .theme(|_| Theme::Dark)
    .window_size((760.0, 560.0))
    .run_with(move || {
        (
            ConfigDialog::new(initial_for_app, output_for_app, show_path_input),
            Task::none(),
        )
    });

    if run.is_err() {
        return None;
    }
    output.lock().ok().and_then(|g| g.clone())
}

#[derive(Debug, Clone)]
enum Message {
    PathChanged(String),
    BrowsePath,
    MinAreaChanged(f32),
    MinSideChanged(f32),
    RecurseSideChanged(f32),
    FramePxChanged(f32),
    HeaderPxChanged(f32),
    CushionHeightChanged(f32),
    CushionFalloffChanged(f32),
    ShowLabelsChanged(bool),
    LabelFontScaleChanged(f32),
    LabelFontPathChanged(String),
    Start,
    Cancel,
}

struct ConfigDialog {
    path_text: String,
    min_area: f32,
    min_side: f32,
    recurse_side: f32,
    frame_px: f32,
    header_px: f32,
    ambient: f32,
    diffuse: f32,
    show_labels: bool,
    label_font_scale: f32,
    label_font_path: String,
    output: Arc<Mutex<Option<DialogResult>>>,
    show_path_input: bool,
}

impl ConfigDialog {
    fn new(initial: DialogResult, output: Arc<Mutex<Option<DialogResult>>>, show_path_input: bool) -> Self {
        Self {
            path_text: initial.scan_path.to_string_lossy().to_string(),
            min_area: initial.layout.min_area,
            min_side: initial.layout.min_side,
            recurse_side: initial.layout.recurse_min_side,
            frame_px: initial.layout.dir_frame_px,
            header_px: initial.layout.dir_header_px,
            ambient: initial.cushion.ambient,
            diffuse: initial.cushion.diffuse,
            show_labels: initial.show_labels,
            label_font_scale: initial.label_font_scale,
            label_font_path: initial.label_font_path,
            output,
            show_path_input,
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PathChanged(value) => {
                self.path_text = value;
                Task::none()
            }
            Message::BrowsePath => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.path_text = path.to_string_lossy().to_string();
                }
                Task::none()
            }
            Message::MinAreaChanged(v) => {
                self.min_area = v;
                Task::none()
            }
            Message::MinSideChanged(v) => {
                self.min_side = v;
                Task::none()
            }
            Message::RecurseSideChanged(v) => {
                self.recurse_side = v;
                Task::none()
            }
            Message::FramePxChanged(v) => {
                self.frame_px = v;
                Task::none()
            }
            Message::HeaderPxChanged(v) => {
                self.header_px = v;
                Task::none()
            }
            Message::CushionHeightChanged(v) => {
                self.ambient = v;
                Task::none()
            }
            Message::CushionFalloffChanged(v) => {
                self.diffuse = v;
                Task::none()
            }
            Message::ShowLabelsChanged(v) => {
                self.show_labels = v;
                Task::none()
            }
            Message::LabelFontScaleChanged(v) => {
                self.label_font_scale = v;
                Task::none()
            }
            Message::LabelFontPathChanged(v) => {
                self.label_font_path = v;
                Task::none()
            }
            Message::Start => {
                let path = PathBuf::from(self.path_text.trim());
                if path.as_os_str().is_empty() {
                    return Task::none();
                }

                let mut layout = LayoutConfig::default();
                layout.min_area = self.min_area;
                layout.min_side = self.min_side;
                layout.recurse_min_side = self.recurse_side;
                layout.dir_frame_px = self.frame_px;
                layout.dir_header_px = self.header_px;

                let mut cushion = CushionConfig::default();
                cushion.ambient = self.ambient;
                cushion.diffuse = self.diffuse;

                if let Ok(mut guard) = self.output.lock() {
                    *guard = Some(DialogResult {
                        scan_path: path,
                        layout,
                        cushion,
                        show_labels: self.show_labels,
                        label_font_scale: self.label_font_scale,
                        label_font_path: self.label_font_path.clone(),
                    });
                }

                window::get_latest().then(|id| match id {
                    Some(id) => window::close::<Message>(id),
                    None => Task::none(),
                })
            }
            Message::Cancel => window::get_latest().then(|id| match id {
                Some(id) => window::close::<Message>(id),
                None => Task::none(),
            }),
        }
    }
}

fn setting_slider<'a>(
    label: &'a str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        text(format!("{label}: {value:.2}")).size(16),
        slider(range, value, on_change).step(0.5)
    ]
    .spacing(6)
    .into()
}

fn view(state: &ConfigDialog) -> Element<'_, Message> {
    let mut body = column![text("Treemap Startup + Graphics Settings").size(26)].spacing(14);

    if state.show_path_input {
        body = body.push(
            column![
                text("Drive / Path").size(18),
                row![
                    text_input("C:\\ or D:\\Rust-projects", &state.path_text)
                        .on_input(Message::PathChanged)
                        .padding(8)
                        .width(Length::Fill),
                    button("Browse").on_press(Message::BrowsePath)
                ]
                .spacing(8)
            ]
            .spacing(8),
        );
    }

    let settings = column![
        text("Layout").size(18),
        setting_slider("Min Area (px²)", state.min_area, 4.0..=400.0, Message::MinAreaChanged),
        setting_slider("Min Side (px)", state.min_side, 1.0..=24.0, Message::MinSideChanged),
        setting_slider(
            "Recurse Min Side (px)",
            state.recurse_side,
            8.0..=160.0,
            Message::RecurseSideChanged
        ),
        setting_slider("Directory Frame (px)", state.frame_px, 0.0..=8.0, Message::FramePxChanged),
        setting_slider(
            "Directory Header (px)",
            state.header_px,
            6.0..=36.0,
            Message::HeaderPxChanged
        ),
        text("Cushion").size(18),
        setting_slider(
            "Ambient Light",
            state.ambient,
            0.05..=0.95,
            Message::CushionHeightChanged
        ),
        setting_slider(
            "Diffuse Light",
            state.diffuse,
            0.05..=1.20,
            Message::CushionFalloffChanged
        ),
        checkbox("Show folder labels", state.show_labels).on_toggle(Message::ShowLabelsChanged),
        setting_slider(
            "Label Font Scale",
            state.label_font_scale,
            0.6..=2.5,
            Message::LabelFontScaleChanged
        ),
        text_input("Custom font path (optional, .ttf)", &state.label_font_path)
            .on_input(Message::LabelFontPathChanged)
            .padding(8)
    ]
    .spacing(10);

    body = body.push(container(settings).padding(12));

    body = body.push(
        row![
            button("Cancel").on_press(Message::Cancel),
            button(if state.show_path_input {
                "Start Scan"
            } else {
                "Apply Settings"
            })
            .on_press(Message::Start)
        ]
        .spacing(10),
    );

    container(body)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
