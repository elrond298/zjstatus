use zellij_tile::prelude::actions::Action;
use zellij_tile::prelude::*;

use chrono::Local;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

use zjstatus::{
    border::BorderPosition,
    config::{self, ModuleConfig, UpdateEventMask, ZellijState},
    frames, pipe,
    render::FormattedPart,
    widgets::{
        command::{CommandResult, CommandWidget},
        datetime::DateTimeWidget,
        mode::ModeWidget,
        notification::NotificationWidget,
        pipe::PipeWidget,
        session::SessionWidget,
        swap_layout::SwapLayoutWidget,
        tabs::TabsWidget,
        widget::Widget,
    },
};

// Matches the old incidental Zellij session scan cadence.
const REFRESH_INTERVAL_SECONDS: f64 = 1.0;
const HINT_DELAY: Duration = Duration::from_millis(500);
const HINT_DISMISS_DELAY: Duration = Duration::from_millis(20);

#[derive(Clone)]
struct HintFormats {
    mode: FormattedPart,
    key: FormattedPart,
    desc: FormattedPart,
    space: FormattedPart,
}

impl HintFormats {
    fn new(configuration: &BTreeMap<String, String>) -> Self {
        let format = |name: &str, default: &str| {
            FormattedPart::from_format_string(
                configuration
                    .get(name)
                    .map(String::as_str)
                    .unwrap_or(default),
                configuration,
            )
        };
        Self {
            mode: format("hint_mode_format", "#[fg=blue,bg=default,bold]"),
            key: format("hint_key_format", "#[fg=yellow,bg=default,bold]"),
            desc: format("hint_desc_format", "#[fg=white,bg=default]"),
            space: format("hint_space_format", "#[bg=default]"),
        }
    }
}

#[derive(Default)]
struct State {
    pending_events: Vec<Event>,
    got_permissions: bool,
    state: ZellijState,
    keybinds: KeybindsVec,
    userspace_configuration: BTreeMap<String, String>,
    module_config: config::ModuleConfig,
    widget_map: BTreeMap<String, Arc<dyn Widget>>,
    focus_cwd_commands: Vec<String>,
    hint_visible: bool,
    hint_dismissed: bool,
    hint_reveal_at: Option<Instant>,
    hint_dismiss_at: Option<Instant>,
    hint_ignore_input_until: Option<Instant>,
    hint_page: usize,
    hint_page_count: usize,
    hint_formats: Option<HintFormats>,
    hint_idle_parts: Vec<FormattedPart>,
    hint_idle_right_parts: Vec<FormattedPart>,
    err: Option<anyhow::Error>,
}

#[cfg(not(test))]
register_plugin!(State);

#[cfg(feature = "tracing")]
fn init_tracing() {
    use std::fs::File;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let file = File::create("/host/.zjstatus.log");
    let file = match file {
        Ok(file) => file,
        Err(error) => panic!("Error: {:?}", error),
    };
    let debug_log = tracing_subscriber::fmt::layer().with_writer(Arc::new(file));

    tracing_subscriber::registry().with(debug_log).init();

    tracing::info!("tracing initialized");
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        #[cfg(feature = "tracing")]
        init_tracing();

        // we need the ReadApplicationState permission to receive the ModeUpdate and TabUpdate
        // events
        // we need the RunCommands permission to run "cargo test" in a floating window
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::RunCommands,
        ]);

        subscribe(&[
            EventType::Mouse,
            EventType::InputReceived,
            EventType::InitialKeybinds,
            EventType::ModeUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::Timer,
            EventType::TabUpdate,
            EventType::SessionUpdate,
            EventType::RunCommandResult,
            EventType::CwdChanged,
        ]);
        set_timeout(REFRESH_INTERVAL_SECONDS);

        self.module_config = match ModuleConfig::new(&configuration) {
            Ok(mc) => mc,
            Err(e) => {
                self.err = Some(e);
                return;
            }
        };
        self.hint_formats = Some(HintFormats::new(&configuration));
        self.hint_idle_parts = FormattedPart::multiple_from_format_string(
            configuration
                .get("hint_idle_format")
                .map(String::as_str)
                .unwrap_or_default(),
            &configuration,
        );
        self.hint_idle_right_parts = FormattedPart::multiple_from_format_string(
            configuration
                .get("hint_idle_right_format")
                .map(String::as_str)
                .unwrap_or_default(),
            &configuration,
        );
        self.widget_map = register_widgets(&configuration);
        self.focus_cwd_commands =
            zjstatus::widgets::command::focus_cwd_command_names(&configuration);
        self.userspace_configuration = configuration;
        self.pending_events = Vec::new();
        self.keybinds = Vec::new();
        self.got_permissions = false;
        self.hint_visible = false;
        self.hint_dismissed = false;
        self.hint_reveal_at = None;
        self.hint_dismiss_at = None;
        self.hint_ignore_input_until = None;
        self.hint_page = 0;
        self.hint_page_count = 1;
        let uid = Uuid::new_v4();

        self.state = ZellijState {
            cols: 0,
            command_results: BTreeMap::new(),
            pipe_results: BTreeMap::new(),
            mode: ModeInfo::default(),
            panes: PaneManifest::default(),
            plugin_uuid: uid.to_string(),
            tabs: Vec::new(),
            sessions: Vec::new(),
            start_time: Local::now(),
            cache_mask: 0,
            incoming_notification: None,
            focused_pane_id: None,
            focused_pane_cwd: None,
        };
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.name == "key-hints-next-page" {
            return self.next_hint_page();
        }
        let mut should_render = false;

        match pipe_message.source {
            PipeSource::Cli(_) => {
                if let Some(input) = pipe_message.payload {
                    should_render = pipe::parse_protocol(&mut self.state, &input);
                }
            }
            PipeSource::Plugin(_) => {
                if let Some(input) = pipe_message.payload {
                    should_render = pipe::parse_protocol(&mut self.state, &input);
                }
            }
            PipeSource::Keybind => {
                if let Some(input) = pipe_message.payload {
                    should_render = pipe::parse_protocol(&mut self.state, &input);
                }
            }
        }

        should_render
    }

    #[tracing::instrument(skip_all, fields(event_type))]
    fn update(&mut self, event: Event) -> bool {
        if let Event::PermissionRequestResult(PermissionStatus::Granted) = event {
            self.got_permissions = true;

            while !self.pending_events.is_empty() {
                tracing::debug!("processing cached event");
                let ev = self.pending_events.pop();

                self.handle_event(ev.unwrap());
            }
        }

        if !self.got_permissions {
            tracing::debug!("caching event");
            self.pending_events.push(event);

            return false;
        }

        self.handle_event(event)
    }

    #[tracing::instrument(skip_all)]
    fn render(&mut self, _rows: usize, cols: usize) {
        if !self.got_permissions {
            return;
        }

        if let Some(err) = &self.err {
            println!("Error: {:?}", err);

            return;
        }

        self.state.cols = cols;

        tracing::debug!("{:?}", self.state.mode.session_name);

        let hint = scrub_markers(self.hint_line(cols));
        let output = scrub_change_markers(
            self.module_config
                .render_bar(self.state.clone(), self.widget_map.clone()),
        );
        if self.module_config.border.enabled
            && matches!(self.module_config.border.position, BorderPosition::Top)
            && let Some((border, status)) = output.split_once('\n')
        {
            print!("{border}\n{hint}\n{status}");
        } else {
            print!("{hint}\n{output}");
        }
    }
}

impl State {
    fn mode_changed(&mut self) {
        self.hint_visible = false;
        self.hint_dismissed = false;
        self.hint_dismiss_at = None;
        self.hint_ignore_input_until = None;
        self.hint_page = 0;
        if shows_hints(&self.state.mode.mode) {
            self.hint_reveal_at = Some(Instant::now() + HINT_DELAY);
            set_timeout(HINT_DELAY.as_secs_f64());
        } else {
            self.hint_reveal_at = None;
        }
    }

    fn input_received(&mut self) {
        let now = Instant::now();
        if self
            .hint_ignore_input_until
            .take()
            .is_some_and(|ignore_until| now <= ignore_until)
        {
            return;
        }
        if self.hint_visible {
            self.hint_dismiss_at = Some(now + HINT_DISMISS_DELAY);
            set_timeout(HINT_DISMISS_DELAY.as_secs_f64());
        } else if self.hint_reveal_at.is_some() && !self.hint_dismissed {
            self.hint_reveal_at = Some(now + HINT_DELAY);
            set_timeout(HINT_DELAY.as_secs_f64());
        }
    }

    fn update_hint_timers(&mut self) -> bool {
        let now = Instant::now();
        if self
            .hint_dismiss_at
            .is_some_and(|dismiss_at| now >= dismiss_at)
        {
            self.hint_visible = false;
            self.hint_dismissed = true;
            self.hint_dismiss_at = None;
            self.hint_reveal_at = None;
            return true;
        }
        if self
            .hint_reveal_at
            .is_some_and(|reveal_at| now >= reveal_at)
        {
            self.hint_visible = true;
            self.hint_reveal_at = None;
            self.hint_page = 0;
            return true;
        }
        false
    }

    fn next_hint_page(&mut self) -> bool {
        if !shows_hints(&self.state.mode.mode) {
            return false;
        }
        let now = Instant::now();
        if self
            .hint_ignore_input_until
            .is_some_and(|ignore_until| now <= ignore_until)
        {
            return false;
        }
        self.hint_reveal_at = None;
        self.hint_dismiss_at = None;
        self.hint_page = if self.hint_visible {
            (self.hint_page + 1) % self.hint_page_count.max(1)
        } else {
            0
        };
        self.hint_visible = true;
        self.hint_dismissed = true;
        self.hint_ignore_input_until = Some(now + Duration::from_millis(50));
        true
    }

    fn idle_line(&mut self, cols: usize) -> String {
        let left = self
            .hint_idle_parts
            .iter_mut()
            .fold(String::new(), |output, part| {
                output + &part.format_string_with_widgets(&self.widget_map, &self.state)
            });
        let right = self
            .hint_idle_right_parts
            .iter_mut()
            .fold(String::new(), |output, part| {
                output + &part.format_string_with_widgets(&self.widget_map, &self.state)
            });
        let (left, right) = fit_idle_status(&left, &right, cols);
        let Some(formats) = &self.hint_formats else {
            return left + &right;
        };
        let gap = cols.saturating_sub(
            console::measure_text_width(&left) + console::measure_text_width(&right),
        );
        left + &formats.space.format_string(&" ".repeat(gap)) + &right
    }

    fn hint_line(&mut self, cols: usize) -> String {
        let pages = hint_pages(&self.state.mode.mode, &self.keybinds, cols);
        self.hint_page_count = pages.len().max(1);
        self.hint_page %= self.hint_page_count;
        if !self.hint_visible || !shows_hints(&self.state.mode.mode) {
            return self.idle_line(cols);
        }
        let Some(formats) = &self.hint_formats else {
            return String::new();
        };

        let mode = format!("{:?}", self.state.mode.mode).to_uppercase();
        let header = fit(
            &format!(
                " {mode} {}/{}  Alt+\\ next  ",
                self.hint_page + 1,
                self.hint_page_count
            ),
            cols,
        );
        let mut used = header.chars().count();
        let mut output = formats.mode.format_string(&header);
        for (index, (key, desc)) in pages[self.hint_page].iter().enumerate() {
            let separator = if index == 0 { "" } else { "   " };
            used += separator.chars().count() + key.chars().count() + 2 + desc.chars().count();
            output.push_str(&formats.space.format_string(separator));
            output.push_str(&formats.key.format_string(key));
            output.push_str(&formats.space.format_string("  "));
            output.push_str(&formats.desc.format_string(desc));
        }
        output.push_str(
            &formats
                .space
                .format_string(&" ".repeat(cols.saturating_sub(used))),
        );
        output
    }
    fn update_focused_pane(&mut self) {
        let active_tab = self.state.tabs.iter().find(|t| t.active);

        let new_id = active_tab
            .and_then(|tab| self.state.panes.panes.get(&tab.position))
            .and_then(|panes| panes.iter().find(|p| p.is_focused && !p.is_plugin))
            .map(|p| PaneId::Terminal(p.id));

        if new_id == self.state.focused_pane_id {
            return;
        }

        self.state.focused_pane_id = new_id;

        let new_cwd = match new_id {
            Some(pane_id) => match get_pane_cwd(pane_id) {
                Ok(cwd) => Some(cwd),
                Err(e) => {
                    tracing::debug!("could not get pane cwd: {e}");
                    None
                }
            },
            None => None,
        };

        self.set_focused_pane_cwd(new_cwd);
    }

    fn set_focused_pane_cwd(&mut self, new_cwd: Option<PathBuf>) -> bool {
        if new_cwd == self.state.focused_pane_cwd {
            return false;
        }

        self.state.focused_pane_cwd = new_cwd;

        if self.focus_cwd_commands.is_empty() {
            return false;
        }

        self.invalidate_focus_cwd_commands();
        true
    }

    fn invalidate_focus_cwd_commands(&mut self) {
        for name in &self.focus_cwd_commands {
            pipe::invalidate_command_result(&mut self.state, name);
            zjstatus::widgets::command::release_command_lock(&self.state, name);
        }
    }

    fn handle_event(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::Mouse(mouse_info) => {
                tracing::Span::current().record("event_type", "Event::Mouse");
                tracing::debug!(mouse = ?mouse_info);

                self.module_config.handle_mouse_action(
                    self.state.clone(),
                    mouse_info,
                    self.widget_map.clone(),
                );
            }
            Event::InputReceived => {
                tracing::Span::current().record("event_type", "Event::InputReceived");
                self.input_received();
            }
            Event::InitialKeybinds(keybinds) => {
                tracing::Span::current().record("event_type", "Event::InitialKeybinds");
                self.keybinds = keybinds;
                should_render = true;
            }
            Event::ModeUpdate(mode_info) => {
                tracing::Span::current().record("event_type", "Event::ModeUpdate");
                tracing::debug!(mode = ?mode_info.mode);
                tracing::debug!(mode = ?mode_info.session_name);

                let mode_changed = self.state.mode.mode != mode_info.mode;
                self.state.mode = mode_info;
                if mode_changed {
                    self.mode_changed();
                }
                self.state.cache_mask = UpdateEventMask::Mode as u8;

                should_render = true;
            }
            Event::PaneUpdate(pane_info) => {
                tracing::Span::current().record("event_type", "Event::PaneUpdate");
                tracing::debug!(pane_count = ?pane_info.panes.len());

                frames::hide_frames_conditionally(
                    &frames::FrameConfig::new(
                        self.module_config.hide_frame_for_single_pane,
                        self.module_config.hide_frame_except_for_search,
                        self.module_config.hide_frame_except_for_fullscreen,
                        self.module_config.hide_frame_except_for_scroll,
                    ),
                    &self.state.tabs,
                    &pane_info,
                    &self.state.mode,
                    get_plugin_ids(),
                    false,
                );

                self.state.panes = pane_info;
                self.state.cache_mask = UpdateEventMask::Tab as u8;

                self.update_focused_pane();

                should_render = true;
            }
            Event::CwdChanged(pane_id, cwd, _clients) => {
                tracing::Span::current().record("event_type", "Event::CwdChanged");
                tracing::debug!(pane_id = ?pane_id, cwd = ?cwd);

                if Some(pane_id) == self.state.focused_pane_id
                    && self.set_focused_pane_cwd(Some(cwd))
                {
                    self.state.cache_mask = UpdateEventMask::Command as u8;
                    should_render = true;
                }
            }
            Event::PermissionRequestResult(result) => {
                tracing::Span::current().record("event_type", "Event::PermissionRequestResult");
                tracing::debug!(result = ?result);
                set_selectable(false);
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                tracing::Span::current().record("event_type", "Event::RunCommandResult");
                tracing::debug!(
                    exit_code = ?exit_code,
                    stdout = ?String::from_utf8(stdout.clone()),
                    stderr = ?String::from_utf8(stderr.clone()),
                    context = ?context
                );

                self.state.cache_mask = UpdateEventMask::Command as u8;

                if let Some(name) = context.get("name") {
                    if self.focus_cwd_commands.iter().any(|n| n == name)
                        && context.get("cwd").map(PathBuf::from) != self.state.focused_pane_cwd
                    {
                        tracing::debug!("discarding stale command result for {name}");
                        return false;
                    }

                    let stdout = match String::from_utf8(stdout) {
                        Ok(s) => s,
                        Err(_) => "".to_owned(),
                    };

                    let stderr = match String::from_utf8(stderr) {
                        Ok(s) => s,
                        Err(_) => "".to_owned(),
                    };

                    self.state.command_results.insert(
                        name.to_owned(),
                        CommandResult {
                            exit_code,
                            stdout,
                            stderr,
                            context,
                        },
                    );
                }
            }
            Event::SessionUpdate(session_info, _) => {
                tracing::Span::current().record("event_type", "Event::SessionUpdate");

                let current_session = session_info.iter().find(|s| s.is_current_session);

                if let Some(current_session) = current_session {
                    frames::hide_frames_conditionally(
                        &frames::FrameConfig::new(
                            self.module_config.hide_frame_for_single_pane,
                            self.module_config.hide_frame_except_for_search,
                            self.module_config.hide_frame_except_for_fullscreen,
                            self.module_config.hide_frame_except_for_scroll,
                        ),
                        &current_session.tabs,
                        &current_session.panes,
                        &self.state.mode,
                        get_plugin_ids(),
                        false,
                    );
                }

                self.state.sessions = session_info;
                self.state.cache_mask = UpdateEventMask::Session as u8;

                should_render = true;
            }
            Event::TabUpdate(tab_info) => {
                tracing::Span::current().record("event_type", "Event::TabUpdate");
                tracing::debug!(tab_count = ?tab_info.len());

                self.state.cache_mask = UpdateEventMask::Tab as u8;
                self.state.tabs = tab_info;

                should_render = true;
            }
            Event::Timer(_) => {
                tracing::Span::current().record("event_type", "Event::Timer");
                set_timeout(REFRESH_INTERVAL_SECONDS);
                self.update_hint_timers();
                self.state.cache_mask = 0;

                should_render = true;
            }
            _ => (),
        };
        should_render
    }
}

fn shows_hints(mode: &InputMode) -> bool {
    matches!(
        mode,
        InputMode::Resize
            | InputMode::Pane
            | InputMode::Tab
            | InputMode::Scroll
            | InputMode::Search
            | InputMode::Session
            | InputMode::Move
            | InputMode::Tmux
    )
}

fn hint_pages(mode: &InputMode, keybinds: &KeybindsVec, cols: usize) -> Vec<Vec<(String, String)>> {
    let width = cols.saturating_sub(28).max(1);
    let mut bindings: Vec<_> = keybinds
        .iter()
        .find(|(input_mode, _)| input_mode == mode)
        .map(|(_, bindings)| bindings.iter().collect())
        .unwrap_or_default();
    bindings.sort_by_key(|(key, _)| !key.key_modifiers.is_empty());

    let mut pages = Vec::new();
    let mut page = Vec::new();
    let mut page_width = 0;
    for (key, actions) in bindings {
        let key = fit(&key.to_string(), width);
        let desc = fit(
            &actions_label(actions),
            width.saturating_sub(key.chars().count() + 2),
        );
        let entry_width = key.chars().count() + 2 + desc.chars().count();
        if !page.is_empty() && page_width + 3 + entry_width > width {
            pages.push(page);
            page = Vec::new();
            page_width = 0;
        }
        page_width += usize::from(!page.is_empty()) * 3 + entry_width;
        page.push((key, desc));
    }
    if !page.is_empty() {
        pages.push(page);
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }
    pages
}

fn actions_label(actions: &[Action]) -> String {
    actions
        .iter()
        .filter(|action| actions.len() == 1 || !matches!(action, Action::SwitchToMode { .. }))
        .map(|action| match action {
            Action::SwitchToMode { input_mode } => format!("{input_mode:?} mode"),
            action => humanize(&action.to_string()),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn humanize(name: &str) -> String {
    let mut output = String::with_capacity(name.len() + 4);
    for (index, character) in name.chars().enumerate() {
        if index > 0 && character.is_uppercase() {
            output.push(' ');
        }
        output.push(character);
    }
    output
}

const PI_DETAIL_START: char = '\u{fe00}';
const PI_DETAIL_END: char = '\u{fe01}';
const VCS_START: char = '\u{fe02}';
const VCS_END: char = '\u{fe03}';
const VCS_DESC_START: char = '\u{fe04}';
const VCS_DESC_END: char = '\u{fe05}';
const VCS_CHANGES_START: char = '\u{e0100}';
const VCS_CHANGES_END: char = '\u{e0101}';
const PI_PROGRESS_START: char = '\u{fe06}';
const PI_PROGRESS_END: char = '\u{fe07}';
const PI_FULL_START: char = '\u{fe08}';
const PI_FULL_END: char = '\u{fe09}';
const PI_AGGREGATE_START: char = '\u{fe0a}';
const PI_AGGREGATE_END: char = '\u{fe0b}';
const METRIC_HISTORY_START: char = '\u{fe0c}';
const METRIC_HISTORY_END: char = '\u{fe0d}';
const METRIC_IO_START: char = '\u{fe0e}';
const METRIC_IO_END: char = '\u{fe0f}';

#[derive(Clone, Copy)]
enum VcsLevel {
    Full,
    Compact,
    NoChanges,
    Hidden,
}

#[derive(Clone, Copy)]
enum PiLevel {
    Full,
    Progress,
    State,
    Aggregate,
}

#[derive(Clone, Copy)]
enum LoadLevel {
    Full,
    NoHistory,
    LoadOnly,
    Hidden,
}

fn map_sections(text: &str, start: char, end: char, mut map: impl FnMut(&str) -> String) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start_index) = rest.find(start) {
        output.push_str(&rest[..start_index]);
        let section = &rest[start_index + start.len_utf8()..];
        let Some(end_index) = section.find(end) else {
            output.push_str(section);
            return output;
        };
        output.push_str(&map(&section[..end_index]));
        rest = &section[end_index + end.len_utf8()..];
    }
    output.push_str(rest);
    output
}

fn optional_sections(text: &str, start: char, end: char, keep: bool) -> String {
    map_sections(text, start, end, |section| {
        if keep {
            section.to_owned()
        } else {
            String::new()
        }
    })
}

fn semantic_prefix(description: &str) -> &str {
    let word = description.split_whitespace().next().unwrap_or_default();
    let end = word.find(['(', ':', '!']).unwrap_or(word.len());
    if end == 0 { word } else { &word[..end] }
}

fn vcs_description(text: &str, level: VcsLevel, max_width: Option<usize>) -> String {
    map_sections(
        text,
        VCS_DESC_START,
        VCS_DESC_END,
        |description| match level {
            VcsLevel::Compact | VcsLevel::NoChanges => semantic_prefix(description).to_owned(),
            VcsLevel::Full => max_width.map_or_else(
                || description.to_owned(),
                |width| console::truncate_str(description, width, "…").into_owned(),
            ),
            VcsLevel::Hidden => String::new(),
        },
    )
}

fn left_variant(
    text: &str,
    vcs_level: VcsLevel,
    pi_level: PiLevel,
    vcs_desc_width: Option<usize>,
) -> String {
    let text = vcs_description(text, vcs_level, vcs_desc_width);
    let text = optional_sections(
        &text,
        VCS_CHANGES_START,
        VCS_CHANGES_END,
        matches!(vcs_level, VcsLevel::Full | VcsLevel::Compact),
    );
    let text = optional_sections(
        &text,
        VCS_START,
        VCS_END,
        !matches!(vcs_level, VcsLevel::Hidden),
    );
    let text = optional_sections(
        &text,
        PI_FULL_START,
        PI_FULL_END,
        !matches!(pi_level, PiLevel::Aggregate),
    );
    let text = optional_sections(
        &text,
        PI_AGGREGATE_START,
        PI_AGGREGATE_END,
        matches!(pi_level, PiLevel::Aggregate),
    );
    let keep_details = matches!(pi_level, PiLevel::Full);
    let text = optional_sections(&text, PI_DETAIL_START, PI_DETAIL_END, keep_details);
    optional_sections(
        &text,
        PI_PROGRESS_START,
        PI_PROGRESS_END,
        matches!(pi_level, PiLevel::Full | PiLevel::Progress),
    )
}

fn right_variant(text: &str, level: LoadLevel) -> String {
    if matches!(level, LoadLevel::Hidden) {
        return String::new();
    }
    let text = optional_sections(
        text,
        METRIC_HISTORY_START,
        METRIC_HISTORY_END,
        matches!(level, LoadLevel::Full),
    );
    optional_sections(
        &text,
        METRIC_IO_START,
        METRIC_IO_END,
        matches!(level, LoadLevel::Full | LoadLevel::NoHistory),
    )
}

fn status_fits(left: &str, right: &str, cols: usize) -> bool {
    console::measure_text_width(left) + console::measure_text_width(right) <= cols
}

fn section_width(text: &str, start: char, end: char) -> usize {
    let mut width = 0;
    map_sections(text, start, end, |section| {
        width += console::measure_text_width(section);
        String::new()
    });
    width
}

fn is_change_marker(character: char) -> bool {
    matches!(character, '\u{e0100}'..='\u{e01ef}')
}

fn is_reserved_marker(character: char) -> bool {
    matches!(character, '\u{e000}'..='\u{e013}' | '\u{fe00}'..='\u{fe0f}')
        || is_change_marker(character)
}

fn scrub_change_markers(mut text: String) -> String {
    text.retain(|character| !is_change_marker(character));
    text
}

fn scrub_markers(mut text: String) -> String {
    text.retain(|character| !is_reserved_marker(character));
    text
}

fn status_pair(left: String, right: String) -> (String, String) {
    (scrub_markers(left), scrub_markers(right))
}

fn fit_idle_status(left: &str, right: &str, cols: usize) -> (String, String) {
    let full_left = left_variant(left, VcsLevel::Full, PiLevel::Full, None);
    let full_right = right_variant(right, LoadLevel::Full);
    if status_fits(&full_left, &full_right, cols) {
        return status_pair(full_left, full_right);
    }

    let description_width = section_width(left, VCS_DESC_START, VCS_DESC_END);
    let mut compact_width = 0;
    drop(map_sections(
        left,
        VCS_DESC_START,
        VCS_DESC_END,
        |description| {
            compact_width += console::measure_text_width(semantic_prefix(description));
            String::new()
        },
    ));
    let overflow = console::measure_text_width(&full_left)
        .saturating_add(console::measure_text_width(&full_right))
        .saturating_sub(cols);
    let truncated_width = description_width.saturating_sub(overflow);
    if truncated_width > compact_width {
        let truncated = left_variant(left, VcsLevel::Full, PiLevel::Full, Some(truncated_width));
        if status_fits(&truncated, &full_right, cols) {
            return status_pair(truncated, full_right);
        }
    }

    let levels = [
        (VcsLevel::Compact, PiLevel::Full, LoadLevel::Full),
        (VcsLevel::Compact, PiLevel::Full, LoadLevel::NoHistory),
        (VcsLevel::Compact, PiLevel::Full, LoadLevel::LoadOnly),
        (VcsLevel::Compact, PiLevel::Full, LoadLevel::Hidden),
        (VcsLevel::NoChanges, PiLevel::Full, LoadLevel::Hidden),
        (VcsLevel::Hidden, PiLevel::Full, LoadLevel::Hidden),
        (VcsLevel::Hidden, PiLevel::Progress, LoadLevel::Hidden),
        (VcsLevel::Hidden, PiLevel::State, LoadLevel::Hidden),
        (VcsLevel::Hidden, PiLevel::Aggregate, LoadLevel::Hidden),
    ];
    for (vcs_level, pi_level, load_level) in levels {
        let left = left_variant(left, vcs_level, pi_level, None);
        let right = right_variant(right, load_level);
        if status_fits(&left, &right, cols) {
            return status_pair(left, right);
        }
    }

    let aggregate = left_variant(left, VcsLevel::Hidden, PiLevel::Aggregate, None);
    status_pair(
        console::truncate_str(&aggregate, cols, "…").into_owned(),
        String::new(),
    )
}

fn fit(text: &str, width: usize) -> String {
    let mut characters = text.chars();
    let mut output: String = characters.by_ref().take(width).collect();
    if characters.next().is_some() && width > 0 {
        output.pop();
        output.push('…');
    }
    output
}

fn register_widgets(configuration: &BTreeMap<String, String>) -> BTreeMap<String, Arc<dyn Widget>> {
    let mut widget_map = BTreeMap::<String, Arc<dyn Widget>>::new();

    widget_map.insert(
        "command".to_owned(),
        Arc::new(CommandWidget::new(configuration)),
    );
    widget_map.insert(
        "datetime".to_owned(),
        Arc::new(DateTimeWidget::new(configuration)),
    );
    widget_map.insert("pipe".to_owned(), Arc::new(PipeWidget::new(configuration)));
    widget_map.insert(
        "swap_layout".to_owned(),
        Arc::new(SwapLayoutWidget::new(configuration)),
    );
    widget_map.insert("mode".to_owned(), Arc::new(ModeWidget::new(configuration)));
    widget_map.insert(
        "session".to_owned(),
        Arc::new(SessionWidget::new(configuration)),
    );
    widget_map.insert("tabs".to_owned(), Arc::new(TabsWidget::new(configuration)));
    widget_map.insert(
        "notifications".to_owned(),
        Arc::new(NotificationWidget::new(configuration)),
    );

    tracing::debug!("registered widgets: {:?}", widget_map.keys());

    widget_map
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn set_focused_pane_cwd_only_invalidates_on_change() {
        let mut state = State {
            focus_cwd_commands: vec!["command_branch".to_owned()],
            ..State::default()
        };
        state.state.focused_pane_cwd = Some(PathBuf::from("/tmp"));
        state.state.command_results.insert(
            "command_branch".to_owned(),
            CommandResult {
                context: BTreeMap::from([(
                    "timestamp".to_owned(),
                    Local::now()
                        .format(zjstatus::widgets::command::TIMESTAMP_FORMAT)
                        .to_string(),
                )]),
                ..CommandResult::default()
            },
        );

        let original_timestamp =
            state.state.command_results["command_branch"].context["timestamp"].clone();

        assert!(!state.set_focused_pane_cwd(Some(PathBuf::from("/tmp"))));
        assert_eq!(
            state.state.command_results["command_branch"].context["timestamp"],
            original_timestamp
        );

        assert!(state.set_focused_pane_cwd(Some(PathBuf::from("/var"))));
        assert_eq!(state.state.focused_pane_cwd, Some(PathBuf::from("/var")));
        assert_ne!(
            state.state.command_results["command_branch"].context["timestamp"],
            original_timestamp
        );
    }

    #[test]
    fn paginates_single_key_before_modified_key() {
        let mode = ModeInfo {
            mode: InputMode::Pane,
            keybinds: vec![(
                InputMode::Pane,
                vec![
                    (
                        "Ctrl b".parse().unwrap(),
                        vec![Action::SwitchToMode {
                            input_mode: InputMode::Tmux,
                        }],
                    ),
                    (
                        "a".parse().unwrap(),
                        vec![Action::SwitchToMode {
                            input_mode: InputMode::Normal,
                        }],
                    ),
                ],
            )],
            ..ModeInfo::default()
        };

        let pages = hint_pages(&mode.mode, &mode.keybinds, 48);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0], vec![("a".into(), "Normal mode".into())]);
        assert_eq!(pages[1], vec![("Ctrl b".into(), "Tmux mode".into())]);
    }

    #[test]
    fn status_variants_reduce_vcs_pi_and_load_sections() {
        let left = format!(
            "{VCS_START} main @- {VCS_DESC_START}feat(status): improve{VCS_DESC_END}\
             {VCS_CHANGES_START}  +2 -1{VCS_CHANGES_END}  {VCS_END}\
             {PI_FULL_START}π [debug] ●{PI_PROGRESS_START} 1/3{PI_DETAIL_START} ▶ task{PI_DETAIL_END}{PI_PROGRESS_END}\
             {PI_DETAIL_START} bash{PI_DETAIL_END}{PI_FULL_END}\
             {PI_AGGREGATE_START}π2 ●1 ○1{PI_AGGREGATE_END}",
        );
        assert_eq!(
            left_variant(&left, VcsLevel::Full, PiLevel::Full, None),
            " main @- feat(status): improve  +2 -1  π [debug] ● 1/3 ▶ task bash"
        );
        assert_eq!(
            left_variant(&left, VcsLevel::Compact, PiLevel::Progress, None),
            " main @- feat  +2 -1  π [debug] ● 1/3"
        );
        assert_eq!(
            left_variant(&left, VcsLevel::NoChanges, PiLevel::Progress, None),
            " main @- feat  π [debug] ● 1/3"
        );
        assert_eq!(
            left_variant(&left, VcsLevel::Hidden, PiLevel::State, None),
            "π [debug] ●"
        );
        assert_eq!(
            left_variant(&left, VcsLevel::Hidden, PiLevel::Aggregate, None),
            "π2 ●1 ○1"
        );

        let right = format!(
            "{METRIC_IO_START}D{METRIC_HISTORY_START}h{METRIC_HISTORY_END} \
             N{METRIC_HISTORY_START}h{METRIC_HISTORY_END} {METRIC_IO_END}\
             L {METRIC_HISTORY_START}H {METRIC_HISTORY_END}",
        );
        assert_eq!(right_variant(&right, LoadLevel::Full), "Dh Nh L H ");
        assert_eq!(right_variant(&right, LoadLevel::NoHistory), "D N L ");
        assert_eq!(right_variant(&right, LoadLevel::LoadOnly), "L ");
        assert_eq!(right_variant(&right, LoadLevel::Hidden), "");
    }

    #[test]
    fn idle_status_truncates_commit_before_reducing_groups() {
        let left = format!(
            "{VCS_START}@- {VCS_DESC_START}feat(status): improve responsive rendering{VCS_DESC_END}  {VCS_END}\
             {PI_FULL_START}π [debug] ●{PI_FULL_END}{PI_AGGREGATE_START}π ●{PI_AGGREGATE_END}",
        );
        let full = left_variant(&left, VcsLevel::Full, PiLevel::Full, None);
        let cols = console::measure_text_width(&full) + 1 - 8;
        let (fitted_left, fitted_right) = fit_idle_status(&left, "R", cols);
        assert_eq!(fitted_right, "R");
        assert!(fitted_left.contains("@- feat(status):"));
        assert!(fitted_left.contains('…'));
        assert!(fitted_left.ends_with("π [debug] ●"));
    }

    #[test]
    fn idle_status_reduces_load_then_vcs_then_pi() {
        let left = format!(
            "{VCS_START}V {VCS_DESC_START}feat{VCS_DESC_END}{VCS_CHANGES_START} C{VCS_CHANGES_END} {VCS_END}\
             {PI_FULL_START}PI{PI_PROGRESS_START} 1/3{PI_DETAIL_START} task{PI_DETAIL_END}{PI_PROGRESS_END}\
             {PI_DETAIL_START} tool{PI_DETAIL_END}{PI_FULL_END}\
             {PI_AGGREGATE_START}A{PI_AGGREGATE_END}",
        );
        let right = format!(
            "{METRIC_IO_START}D{METRIC_HISTORY_START}h{METRIC_HISTORY_END}\
             N{METRIC_HISTORY_START}h{METRIC_HISTORY_END}{METRIC_IO_END}\
             L{METRIC_HISTORY_START}H{METRIC_HISTORY_END}",
        );
        let width = |left: &str, right: &str| {
            console::measure_text_width(left) + console::measure_text_width(right)
        };
        let compact = left_variant(&left, VcsLevel::Compact, PiLevel::Full, None);
        let no_history = right_variant(&right, LoadLevel::NoHistory);
        assert_eq!(
            fit_idle_status(&left, &right, width(&compact, &no_history)),
            (compact.clone(), no_history)
        );

        let load_only = right_variant(&right, LoadLevel::LoadOnly);
        assert_eq!(
            fit_idle_status(&left, &right, width(&compact, &load_only)),
            (compact.clone(), load_only)
        );
        assert_eq!(
            fit_idle_status(&left, &right, width(&compact, "")),
            (compact, String::new())
        );

        let no_changes = left_variant(&left, VcsLevel::NoChanges, PiLevel::Full, None);
        assert_eq!(
            fit_idle_status(&left, &right, width(&no_changes, "")),
            (no_changes, String::new())
        );

        let pi_full = left_variant(&left, VcsLevel::Hidden, PiLevel::Full, None);
        assert_eq!(
            fit_idle_status(&left, &right, width(&pi_full, "")),
            (pi_full, String::new())
        );
        let pi_progress = left_variant(&left, VcsLevel::Hidden, PiLevel::Progress, None);
        assert_eq!(
            fit_idle_status(&left, &right, width(&pi_progress, "")),
            (pi_progress, String::new())
        );
        let pi_state = left_variant(&left, VcsLevel::Hidden, PiLevel::State, None);
        assert_eq!(
            fit_idle_status(&left, &right, width(&pi_state, "")),
            (pi_state, String::new())
        );
        assert_eq!(
            fit_idle_status(&left, &right, 1),
            ("A".to_owned(), String::new())
        );
    }

    #[test]
    fn compact_commit_description_keeps_semantic_prefix() {
        assert_eq!(semantic_prefix("feat(status): improve"), "feat");
        assert_eq!(semantic_prefix("fix!: break API"), "fix");
        assert_eq!(semantic_prefix("修复 状态栏"), "修复");
    }

    #[test]
    fn final_status_scrubs_injected_and_malformed_markers() {
        let left = format!("safe{VCS_END}\u{e010}\u{e0102}{PI_DETAIL_START}detail");
        let right = format!("right{METRIC_HISTORY_END}\u{e000}");
        let (left, right) = fit_idle_status(&left, &right, usize::MAX);
        assert_eq!(left, "safedetail");
        assert_eq!(right, "right");
        assert!(!left.chars().chain(right.chars()).any(is_reserved_marker));

        let ordinary = format!("☀\u{fe0f}\u{e000}\u{e0100}");
        assert_eq!(scrub_change_markers(ordinary), format!("☀\u{fe0f}\u{e000}"));
    }
}
