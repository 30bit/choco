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

    fn open_file(&mut self) {
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

    fn save_file(&mut self) {
        if !self.has_unsaved_changes {
            if let Some(path) = &self.opened_file_path {
                if let Err(err) = self.write(path) {
                    log::error!("when saving file: {err}");
                } else {
                    self.has_unsaved_changes = false;
                }
            }
        }
    }

    fn save_file_as(&mut self) {
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

    fn show_menu(&mut self, ui: &mut egui::Ui, shortcuts: &CommandShortcuts) {
        ui.style_mut().visuals.button_frame = false;
        ui.menu_button("File", |ui| {
            if command_button(ui, RichText::new("Open.."), shortcuts.open) {
                self.open_file();
            }
            let mut save_text = RichText::new("Save");
            if !self.has_unsaved_changes || self.opened_file_path.is_none() {
                save_text = save_text.weak();
            }
            if command_button(ui, save_text, shortcuts.save) {
                self.save_file();
            }
            if command_button(ui, RichText::new("Save as.."), shortcuts.save_as) {
                self.save_file_as();
            }
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
        let shortcuts = CommandShortcuts::consume_in(ctx);
        if shortcuts.do_open {
            self.open_file()
        } else if shortcuts.do_save {
            self.save_file()
        } else if shortcuts.do_save_as {
            self.save_file_as()
        }
        egui::TopBottomPanel::new(egui::panel::TopBottomSide::Top, "menu")
            .resizable(false)
            .show(ctx, |ui| self.show_menu(ui, &shortcuts));
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

struct CommandShortcuts {
    do_open: bool,
    open: egui::KeyboardShortcut,
    do_save: bool,
    save: egui::KeyboardShortcut,
    do_save_as: bool,
    save_as: egui::KeyboardShortcut,
}

impl CommandShortcuts {
    pub fn consume_in(ctx: &egui::Context) -> Self {
        let open = command_shortcut(egui::Key::O, false);
        let save = command_shortcut(egui::Key::S, false);
        let save_as = command_shortcut(egui::Key::S, true);
        ctx.input_mut(|input| Self {
            do_open: input.consume_shortcut(&open),
            open,
            do_save: input.consume_shortcut(&save),
            save,
            do_save_as: input.consume_shortcut(&save_as),
            save_as,
        })
    }
}

fn command_shortcut(key: egui::Key, shift: bool) -> egui::KeyboardShortcut {
    #[cfg(target_os = "macos")]
    let mut modifier = egui::Modifiers::MAC_CMD;
    #[cfg(not(target_os = "macos"))]
    let mut modifier = egui::Modifiers::CTRL;
    if shift {
        modifier = modifier | egui::Modifiers::SHIFT;
    }
    egui::KeyboardShortcut::new(modifier, key)
}

fn command_button(ui: &mut egui::Ui, text: RichText, shortcut: egui::KeyboardShortcut) -> bool {
    let shortcut_text = ui.ctx().format_shortcut(&shortcut);
    ui.add(egui::Button::new(text).small().shortcut_text(shortcut_text))
        .clicked()
}
