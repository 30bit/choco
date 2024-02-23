use choco::{
    petgraph::{
        graph::NodeIndex,
        visit::{self, EdgeRef as _},
    },
    Story,
};
use copypasta::{ClipboardContext, ClipboardProvider};
use eframe::{
    egui::{
        self,
        mutex::Mutex,
        text::{CCursor, CCursorRange},
        RichText,
    },
    epaint::Color32,
};
use std::{
    collections::HashMap,
    fs, io, ops,
    path::{Path, PathBuf},
    sync::Arc,
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
    state: Arc<Mutex<State>>,
    clipboard: Option<ClipboardContext>,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            clipboard: ClipboardContext::new().ok(),
        }
    }

    fn show_menu(
        &mut self,
        ui: &mut egui::Ui,
        shortcuts: &CommandShortcuts,
    ) -> (SelectionCommands, UndoerCommands) {
        ui.style_mut().visuals.button_frame = false;
        ui.horizontal(|ui| {
            ui.columns(2, |ui| {
                ui[0].with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    if command_button(ui, RichText::new("Open.."), shortcuts.open) {
                        State::open_file(self.state.clone());
                    }
                    let mut save_text = RichText::new("Save");
                    if !self.state.lock().has_unsaved_changes
                        || self.state.lock().opened_file_path.is_none()
                    {
                        save_text = save_text.strikethrough();
                    }
                    if command_button(ui, save_text, shortcuts.save) {
                        State::save_file(self.state.clone());
                    }
                    if command_button(ui, RichText::new("Save as.."), shortcuts.save_as) {
                        State::save_file_as(self.state.clone());
                    }
                });
                ui[1]
                    .with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        let _state = self.state.lock();
                        (
                            SelectionCommands::show_menu_button_in(
                                ui,
                                shortcuts,
                                self.clipboard.is_some(),
                            ),
                            UndoerCommands::show_menu_button_in(
                                ui, shortcuts,
                                // FIXME: Nothing is being undone
                                // !state.has_undo,
                                // !state.has_redo,
                                true, true,
                            ),
                        )
                    })
                    .inner
            })
        })
        .inner
    }

    fn show_guide(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.active.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        ui.style_mut().visuals.widgets.active.bg_stroke = egui::Stroke::NONE;

        ui.horizontal_wrapped(|ui| {
            let mut state = self.state.lock();
            let mut bookmarks: Vec<_> = state.guide.keys().map(String::to_owned).collect();
            bookmarks.sort_unstable();

            for bookmark in bookmarks {
                let mut text = RichText::new(&bookmark).monospace();
                let was_selected = bookmark == state.starting_bookmark;
                if was_selected {
                    text = text.underline();
                }
                if ui.button(text).clicked() {
                    if was_selected {
                        state.starting_bookmark = String::new();
                    } else {
                        state.starting_bookmark = bookmark.clone();
                    }
                }
            }
        });
    }

    fn show_events(&self, range: ops::Range<usize>, ui: &mut egui::Ui) {
        let state = self.state.lock();
        let events = choco::event_iter(state.content.get(range).unwrap_or_default());
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
        let state = self.state.lock();
        if let Some(start) = state.guide.get(&state.starting_bookmark) {
            let index_to_name: HashMap<_, _> =
                state.guide.iter().map(|entry| (entry.1, entry.0)).collect();
            let mut bfs = visit::Bfs::new(&state.story, *start);
            while let Some(index) = bfs.next(&state.story) {
                egui::Frame::default()
                    .outer_margin(egui::Margin {
                        right: 16.0,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        egui::CollapsingHeader::new(index_to_name[&index])
                            .default_open(true)
                            .show(ui, |ui| {
                                self.show_events(state.story[index].clone(), ui);
                                for edge in state.story.edges(index) {
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
                                                        state.story[edge.id()].clone(),
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

    fn show_editor(
        &mut self,
        ui: &mut egui::Ui,
        selection: &SelectionCommands,
        _undo: &UndoerCommands,
    ) {
        let mut state = self.state.lock();
        ui.style_mut().visuals.extreme_bg_color = Color32::TRANSPARENT;
        let editor_id = egui::Id::new("choco-editor");
        if selection.do_copy {
            if let Some(text) = egui::TextEdit::load_state(ui.ctx(), editor_id) {
                if let Some(selection_range) = text.ccursor_range() {
                    if let Some(clipboard) = &mut self.clipboard {
                        let byte_range =
                            char_cursor_range_to_byte_range(&state.content, selection_range);
                        let slice = &state.content[byte_range];
                        if let Err(err) = clipboard.set_contents(slice.to_owned()) {
                            log::error!("when clipboard copying: {err}");
                        }
                    }
                }
            }
        }
        if selection.do_paste {
            if let Some(text) = egui::TextEdit::load_state(ui.ctx(), editor_id) {
                if let Some(selection_range) = text.ccursor_range() {
                    if let Some(clipboard) = &mut self.clipboard {
                        match clipboard.get_contents() {
                            Ok(paste) => {
                                let byte_range = char_cursor_range_to_byte_range(
                                    &state.content,
                                    selection_range,
                                );
                                state.content.replace_range(byte_range, &paste);
                            }
                            Err(err) => log::error!("when clipboard pasting: {err}"),
                        }
                    }
                }
            }
        }
        let editor = egui::TextEdit::multiline(&mut state.content)
            .code_editor()
            .margin(egui::Vec2::ZERO)
            .hint_text("Let it brew..")
            .desired_rows(200)
            .desired_width(f32::INFINITY)
            .frame(false)
            .id(editor_id);
        let editor_output = editor.show(ui);
        // let mut editor_state = editor_output.state;
        // let content_state = (
        //     editor_state.ccursor_range().unwrap_or_default(),
        //     state.content.clone(),
        // );
        // let mut editor_undoer = editor_state.undoer();
        // editor_undoer.feed_state(
        //     SystemTime::UNIX_EPOCH
        //         .elapsed()
        //         .unwrap_or_default()
        //         .as_secs_f64(),
        //     &content_state,
        // );
        // if editor_undoer.has_undo(&content_state) {
        //     state.has_undo = true;
        // }
        // if editor_undoer.has_redo(&content_state) {
        //     state.has_redo = true;
        // }
        // if state.has_undo && undo.do_undo {
        //     state.has_redo = editor_undoer.undo(&content_state).is_some();
        // }
        // if state.has_redo && undo.do_redo {
        //     editor_undoer.redo(&content_state);
        // }
        // editor_state.set_undoer(editor_undoer);

        if editor_output.response.changed() {
            state.has_unsaved_changes = true;
            // state.has_undo = true;
            state.update_state();
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let shortcuts = CommandShortcuts::consume_in(ctx);
        if shortcuts.do_open {
            State::open_file(self.state.clone());
        } else if shortcuts.do_save {
            State::save_file(self.state.clone());
        } else if shortcuts.do_save_as {
            State::save_file_as(self.state.clone());
        }
        let (selection, undo) = egui::TopBottomPanel::new(egui::panel::TopBottomSide::Top, "menu")
            .resizable(false)
            .show(ctx, |ui| self.show_menu(ui, &shortcuts))
            .inner;
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
                .show(ui, |ui| self.show_editor(ui, &selection, &undo))
        });
    }
}

struct State {
    has_unsaved_changes: bool,
    // has_undo: bool,
    // has_redo: bool,
    opened_file_path: Option<PathBuf>,
    content: String,
    story: Story,
    guide: HashMap<String, NodeIndex>,
    starting_bookmark: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            has_unsaved_changes: true,
            // has_undo: false,
            // has_redo: false,
            opened_file_path: None,
            content: String::new(),
            story: Story::new(),
            guide: HashMap::new(),
            starting_bookmark: String::new(),
        }
    }
}

impl State {
    fn read<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        self.content = fs::read_to_string(path)?;
        self.update_state();
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

    fn write<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        if let Some(dir) = path.as_ref().parent() {
            fs::create_dir_all(dir)?;
        }

        fs::write(path, &*self.content)?;
        Ok(())
    }

    #[allow(clippy::needless_pass_by_value)]
    fn save_file(self_: Arc<Mutex<Self>>) {
        // thread::spawn(move || {
        let mut lock = self_.lock();
        if !lock.has_unsaved_changes {
            if let Some(path) = &lock.opened_file_path {
                let path = path.clone();
                if let Err(err) = lock.write(path) {
                    log::error!("when saving file: {err}");
                } else {
                    lock.has_unsaved_changes = false;
                }
            }
        }
        // });
    }

    #[allow(clippy::needless_pass_by_value)]
    fn save_file_as(self_: Arc<Mutex<Self>>) {
        // thread::spawn(move || {
        let mut lock = self_.lock();
        let path = rfd::FileDialog::new()
            .set_file_name("untitled.choco")
            .save_file();
        let mut ok = true;
        if let Some(path) = &path {
            if let Err(err) = lock.write(path) {
                log::error!("when saving file: {err}");
                ok = false;
            }
        }
        if ok && lock.opened_file_path.is_none() {
            lock.opened_file_path = path;
            lock.has_unsaved_changes = false;
        }
        // });
    }

    #[allow(clippy::needless_pass_by_value)]
    fn open_file(self_: Arc<Mutex<Self>>) {
        // thread::spawn(move || {
        let mut lock = self_.lock();
        lock.opened_file_path = rfd::FileDialog::new()
            .add_filter("choco source file", &["choco"])
            .pick_file();
        if let Some(path) = &lock.opened_file_path {
            let path = path.clone();
            if let Err(err) = lock.read(path) {
                log::error!("when opening file: {err}");
            }
            lock.has_unsaved_changes = false;
        }
        // });
    }
}

struct CommandShortcuts {
    do_open: bool,
    open: egui::KeyboardShortcut,
    do_save: bool,
    save: egui::KeyboardShortcut,
    do_save_as: bool,
    save_as: egui::KeyboardShortcut,
    copy: egui::KeyboardShortcut,
    paste: egui::KeyboardShortcut,
    undo: egui::KeyboardShortcut,
    redo: egui::KeyboardShortcut,
}

impl CommandShortcuts {
    pub fn consume_in(ctx: &egui::Context) -> Self {
        let open = command_shortcut(egui::Key::O, false);
        let save = command_shortcut(egui::Key::S, false);
        let save_as = command_shortcut(egui::Key::S, true);
        let copy = command_shortcut(egui::Key::C, false);
        let paste = command_shortcut(egui::Key::V, false);
        let undo = command_shortcut(egui::Key::Z, false);
        let redo = command_shortcut(egui::Key::Z, true);
        ctx.input_mut(|input| Self {
            do_open: input.consume_shortcut(&open),
            do_save_as: input.consume_shortcut(&save_as),
            do_save: input.consume_shortcut(&save),
            open,
            save,
            save_as,
            copy,
            paste,
            undo,
            redo,
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

#[derive(Default)]
pub struct SelectionCommands {
    do_copy: bool,
    do_paste: bool,
}

impl SelectionCommands {
    fn show_menu_button_in(
        ui: &mut egui::Ui,
        shortcuts: &CommandShortcuts,
        has_clipboard: bool,
    ) -> Self {
        let mut output = Self::default();
        if has_clipboard && command_button(ui, RichText::new("Copy"), shortcuts.copy) {
            output.do_copy = true;
        }
        if has_clipboard && command_button(ui, RichText::new("Paste"), shortcuts.paste) {
            output.do_paste = true;
        }
        output
    }
}

fn char_cursor_range_to_byte_range(s: &str, range: CCursorRange) -> ops::Range<usize> {
    let find_byte_index = |char_cursor: CCursor| {
        s.char_indices()
            .nth(char_cursor.index)
            .map(|(index, _)| index)
    };
    let [char_left, char_right] = range.sorted();
    let secondary = find_byte_index(range.secondary);
    let left = find_byte_index(char_left).or(secondary).unwrap_or(s.len());
    let right = find_byte_index(char_right).unwrap_or(left);
    left..right
}

#[derive(Default)]
pub struct UndoerCommands {
    do_undo: bool,
    do_redo: bool,
}

impl UndoerCommands {
    fn show_menu_button_in(
        ui: &mut egui::Ui,
        shortcuts: &CommandShortcuts,
        nothing_to_undo: bool,
        nothing_to_redo: bool,
    ) -> Self {
        let mut output = Self::default();
        let mut undo_text = RichText::new("Undo");
        if nothing_to_undo {
            undo_text = undo_text.strikethrough();
        }
        if command_button(ui, undo_text, shortcuts.undo) && !nothing_to_undo {
            output.do_undo = true;
        }
        let mut redo_text = RichText::new("Redo");
        if nothing_to_redo {
            redo_text = redo_text.strikethrough();
        }
        if command_button(ui, redo_text, shortcuts.redo) && !nothing_to_redo {
            output.do_redo = true;
        }
        output
    }
}
