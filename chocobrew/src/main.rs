use choco::{
    petgraph::{
        graph::NodeIndex,
        visit::{self, EdgeRef as _},
    },
    Story,
};
use eframe::{
    egui::{self, RichText},
    epaint::Color32,
};
use std::{
    collections::HashMap,
    fs, io, ops,
    path::{Path, PathBuf},
};

fn main() -> eframe::Result<()> {
    env_logger::builder()
        .format_timestamp(None)
        .filter_level(log::LevelFilter::Error)
        .init();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "chocobrew",
        native_options,
        Box::new(|cc| Box::new(App::new(cc))),
    )
}

struct App {
    has_unsaved_changes: bool,
    opened_file_path: Option<PathBuf>,
    content: String,
    story: Story,
    guide: HashMap<String, NodeIndex>,
    starting_bookmark: String,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            has_unsaved_changes: true,
            opened_file_path: None,
            content: String::new(),
            story: Story::new(),
            guide: HashMap::new(),
            starting_bookmark: String::new(),
        }
    }

    fn write<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        if let Some(dir) = path.as_ref().parent() {
            fs::create_dir_all(dir)?;
        }

        fs::write(path, &self.content)?;
        Ok(())
    }

    fn update_state(&mut self) {
        let (guide, story) = choco::read([self.content.as_str()]);
        let guide = guide
            .into_iter()
            .map(|(prompt, value)| (prompt.to_owned(), value))
            .collect();
        self.story = story;
        self.guide = guide;
    }

    fn read<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        self.content = fs::read_to_string(path)?;
        self.update_state();
        Ok(())
    }

    fn show_menu(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().visuals.button_frame = false;
        ui.columns(2, |ui| {
            ui[0].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if ui.small_button("Open..").clicked() {
                    self.opened_file_path = rfd::FileDialog::new()
                        .add_filter("choco source file", &["choco"])
                        .pick_file();
                    if let Some(path) = &self.opened_file_path {
                        if let Err(err) = self.read(path.clone()) {
                            log::error!("when opening file: {err}");
                        }
                        self.has_unsaved_changes = false;
                    }
                }
            });
            ui[1].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut save_text = RichText::new("Save");
                if !self.has_unsaved_changes || self.opened_file_path.is_none() {
                    save_text = save_text.weak();
                }
                if ui.small_button(save_text).clicked() {
                    if let Some(path) = &self.opened_file_path {
                        if let Err(err) = self.write(path) {
                            log::error!("when saving file: {err}");
                        } else {
                            self.has_unsaved_changes = false;
                        }
                    }
                }
                if ui.small_button("Save As..").clicked() {
                    let path = rfd::FileDialog::new()
                        .set_file_name("untitled.choco")
                        .save_file();
                    let mut ok = true;
                    if let Some(path) = &path {
                        if let Err(err) = self.write(path) {
                            log::error!("when saving file: {err}");
                            ok = false;
                        }
                    }
                    if ok && self.opened_file_path.is_none() {
                        self.opened_file_path = path;
                        self.has_unsaved_changes = false;
                    }
                }
            })
        });
    }

    fn show_guide(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.active.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        ui.style_mut().visuals.widgets.active.bg_stroke = egui::Stroke::NONE;

        ui.horizontal_wrapped(|ui| {
            let mut bookmarks: Vec<_> = self.guide.keys().collect();
            bookmarks.sort_unstable();

            for bookmark in bookmarks {
                let mut text = RichText::new(bookmark).monospace();
                let was_selected = bookmark == &self.starting_bookmark;
                if was_selected {
                    text = text.underline();
                }
                if ui.button(text).clicked() {
                    if was_selected {
                        self.starting_bookmark = String::new();
                    } else {
                        self.starting_bookmark = bookmark.to_owned();
                    }
                }
            }
        });
    }

    fn show_events(&self, range: ops::Range<usize>, ui: &mut egui::Ui) {
        let events = choco::event_iter(self.content.get(range).unwrap_or_default());
        for event in events {
            match event {
                choco::Event::Signal(choco::Signal::Ping) => {
                    ui.label(RichText::new('@').weak());
                }
                choco::Event::Signal(choco::Signal::Prompt(prompt)) => {
                    ui.add(
                        egui::Label::new(RichText::new(format!("@{}", prompt.slice)).weak())
                            .truncate(true),
                    );
                }
                choco::Event::Signal(choco::Signal::Param(param)) => {
                    ui.add(
                        egui::Label::new(RichText::new(format!("@{{{}}}", param.slice)).weak())
                            .truncate(true),
                    );
                }
                choco::Event::Signal(choco::Signal::Call { prompt, param }) => {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("@{}{{{}}}", prompt.slice, param.slice)).weak(),
                        )
                        .truncate(true),
                    );
                }
                choco::Event::Text { style, content } => {
                    let mut text = RichText::new(content.slice);
                    if style.contains(choco::Style::BOLD) {
                        text = text.strong();
                    }
                    if style.contains(choco::Style::CODE) {
                        text = text.code();
                    }
                    if style.contains(choco::Style::ITALIC) {
                        text = text.italics();
                    }
                    if style.contains(choco::Style::SCRATCH) {
                        text = text.strikethrough();
                    }
                    if style.contains(choco::Style::UNDERLINE) {
                        text = text.underline();
                    }
                    if style.contains(choco::Style::PANEL) {
                        text = text.background_color(ui.style().visuals.extreme_bg_color);
                    }
                    if style.contains(choco::Style::QUOTE) {
                        text = text.color(ui.style().visuals.hyperlink_color);
                    }
                    ui.add(egui::Label::new(text).truncate(true));
                }
                choco::Event::Break => {
                    ui.separator();
                }
            }
        }
    }

    fn show_preview(&self, ui: &mut egui::Ui) {
        if let Some(start) = self.guide.get(&self.starting_bookmark) {
            let index_to_name: HashMap<_, _> =
                self.guide.iter().map(|entry| (entry.1, entry.0)).collect();
            let mut bfs = visit::Bfs::new(&self.story, *start);
            while let Some(index) = bfs.next(&self.story) {
                egui::Frame::default()
                    .outer_margin(egui::Margin {
                        right: 16.0,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        egui::CollapsingHeader::new(index_to_name[&index])
                            .default_open(true)
                            .show(ui, |ui| {
                                self.show_events(self.story[index].clone(), ui);
                                for edge in self.story.edges(index) {
                                    egui::Frame::default()
                                        .outer_margin(egui::Margin {
                                            right: 16.0,
                                            ..Default::default()
                                        })
                                        .show(ui, |ui| {
                                            egui::CollapsingHeader::new(
                                                index_to_name[&edge.target()],
                                            )
                                            .default_open(true)
                                            .show(
                                                ui,
                                                |ui| {
                                                    self.show_events(
                                                        self.story[edge.id()].clone(),
                                                        ui,
                                                    );
                                                },
                                            );
                                        });
                                }
                            });
                    });
            }
        }
    }

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().visuals.extreme_bg_color = Color32::TRANSPARENT;
        if egui::TextEdit::multiline(&mut self.content)
            .code_editor()
            .margin(egui::Vec2::ZERO)
            .hint_text("Let it brew..")
            .desired_rows(200)
            .desired_width(f32::INFINITY)
            .frame(false)
            .show(ui)
            .response
            .changed()
        {
            self.has_unsaved_changes = true;
            self.update_state();
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::new(egui::panel::TopBottomSide::Top, "menu")
            .resizable(false)
            .show(ctx, |ui| self.show_menu(ui));
        egui::SidePanel::new(egui::panel::Side::Left, "guide")
            .min_width(ctx.screen_rect().width() * 0.19)
            .default_width(ctx.screen_rect().width() * 0.1914)
            .max_width(ctx.screen_rect().width() * 0.193)
            .resizable(false)
            .show(ctx, |ui| {
                egui::ScrollArea::new([false, true])
                    .auto_shrink(true)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| self.show_guide(ui))
            });
        egui::SidePanel::new(egui::panel::Side::Right, "preview")
            .min_width(ctx.screen_rect().width() * 0.2985)
            .default_width(ctx.screen_rect().width() * 0.301)
            .max_width(ctx.screen_rect().width() * 0.3025)
            .resizable(false)
            .show(ctx, |ui| {
                egui::ScrollArea::new([false, true])
                    .auto_shrink(true)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| self.show_preview(ui))
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::new([false, true])
                .auto_shrink(false)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| self.show_editor(ui))
        });
    }
}
