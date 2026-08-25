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
    render::{FormattedPart, formatted_parts_from_string_cached},
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
    hint_idle_row: Option<IdleRow>,
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
        self.hint_idle_row = match IdleRow::from_config(&configuration) {
            Ok(row) => row,
            Err(error) => {
                self.err = Some(error);
                return;
            }
        };
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

        let hint = self.hint_line(cols);
        let output = self
            .module_config
            .render_bar(self.state.clone(), self.widget_map.clone());
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
        let (left, right) = if let Some(row) = &mut self.hint_idle_row {
            row.fit(
                &self.widget_map,
                &self.state,
                &self.userspace_configuration,
                cols,
            )
        } else {
            let left = self
                .hint_idle_parts
                .iter_mut()
                .fold(String::new(), |output, part| {
                    output + &part.format_string_with_widgets(&self.widget_map, &self.state)
                });
            let right =
                self.hint_idle_right_parts
                    .iter_mut()
                    .fold(String::new(), |output, part| {
                        output + &part.format_string_with_widgets(&self.widget_map, &self.state)
                    });
            fit_idle_pair(left, right, cols)
        };
        let Some(formats) = &self.hint_formats else {
            return left + &right;
        };
        let gap = cols.saturating_sub(
            console::measure_text_width(&left) + console::measure_text_width(&right),
        );
        left + &formats.space.format_string(&" ".repeat(gap)) + &right
    }

    fn hint_line(&mut self, cols: usize) -> String {
        let mode = format!("{:?}", self.state.mode.mode).to_uppercase();
        let hint_sizes = self
            .keybinds
            .iter()
            .find(|(input_mode, _)| input_mode == &self.state.mode.mode)
            .map(|(_, bindings)| {
                bindings
                    .iter()
                    .map(|(key, actions)| {
                        (
                            console::measure_text_width(&key.to_string()),
                            console::measure_text_width(&actions_label(actions)),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let max_pages = hint_sizes.len().max(1);
        let (header_kind, header_width) = hint_header_layout(&mode, max_pages, &hint_sizes, cols);
        let content_width = cols.saturating_sub(header_width);
        let pages = hint_pages(&self.state.mode.mode, &self.keybinds, content_width);
        self.hint_page_count = pages.len().max(1);
        self.hint_page %= self.hint_page_count;
        if !self.hint_visible || !shows_hints(&self.state.mode.mode) {
            return self.idle_line(cols);
        }
        let Some(formats) = &self.hint_formats else {
            return String::new();
        };

        let header =
            format_hint_header(header_kind, &mode, self.hint_page + 1, self.hint_page_count);
        let mut used = console::measure_text_width(&header);
        let mut output = formats.mode.format_string(&header);
        let (key_desc_separator, entry_separator) = hint_gaps(content_width);
        for (index, (key, desc)) in pages[self.hint_page].iter().enumerate() {
            let separator = if index == 0 { "" } else { entry_separator };
            used += console::measure_text_width(separator) + console::measure_text_width(key);
            output.push_str(&formats.space.format_string(separator));
            output.push_str(&formats.key.format_string(key));
            used +=
                console::measure_text_width(key_desc_separator) + console::measure_text_width(desc);
            output.push_str(&formats.space.format_string(key_desc_separator));
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
                    let Some(run_id) = context.get("run_id") else {
                        tracing::debug!("discarding command result without run id for {name}");
                        return false;
                    };
                    if !zjstatus::widgets::command::release_command_lock(&self.state, name, run_id)
                    {
                        tracing::debug!("discarding superseded command result for {name}");
                        return false;
                    }
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

const MIN_HINT_DESCRIPTION_WIDTH: usize = 4;

#[derive(Clone, Copy)]
enum HintHeader {
    Full,
    Compact,
    Page,
    None,
}

fn hint_header_layout(
    mode: &str,
    max_pages: usize,
    hint_sizes: &[(usize, usize)],
    cols: usize,
) -> (HintHeader, usize) {
    let full = format!(" {mode} {max_pages}/{max_pages}  Alt+\\ next  ");
    let compact = format!(" {mode} {max_pages}/{max_pages} ");
    let page = format!(" {max_pages}/{max_pages} ");
    for (kind, header) in [
        (HintHeader::Full, full),
        (HintHeader::Compact, compact),
        (HintHeader::Page, page),
    ] {
        let width = console::measure_text_width(&header);
        let content_width = cols.saturating_sub(width);
        if minimum_hint_content_width(hint_sizes) <= content_width {
            return (kind, width);
        }
    }
    (HintHeader::None, 0)
}

fn format_hint_header(kind: HintHeader, mode: &str, page: usize, pages: usize) -> String {
    match kind {
        HintHeader::Full => format!(" {mode} {page}/{pages}  Alt+\\ next  "),
        HintHeader::Compact => format!(" {mode} {page}/{pages} "),
        HintHeader::Page => format!(" {page}/{pages} "),
        HintHeader::None => String::new(),
    }
}

fn hint_gaps(width: usize) -> (&'static str, &'static str) {
    if width < 24 {
        (" ", " ")
    } else {
        ("  ", "   ")
    }
}

fn minimum_hint_content_width(hint_sizes: &[(usize, usize)]) -> usize {
    let (key_desc_separator, entry_separator) = hint_gaps(24);
    let key_desc_width = console::measure_text_width(key_desc_separator);
    let entry_width = console::measure_text_width(entry_separator);
    let mut hint_widths = hint_sizes
        .iter()
        .map(|(key_width, description_width)| key_width + key_desc_width + description_width)
        .collect::<Vec<_>>();
    hint_widths.sort_unstable();
    match hint_widths.as_slice() {
        [] => 1,
        [width] => *width,
        widths => (widths[0] + widths[1] + entry_width).max(widths[widths.len() - 1]),
    }
}

fn hint_pages(
    mode: &InputMode,
    keybinds: &KeybindsVec,
    width: usize,
) -> Vec<Vec<(String, String)>> {
    if width < 3 {
        return vec![Vec::new()];
    }
    let mut bindings: Vec<_> = keybinds
        .iter()
        .find(|(input_mode, _)| input_mode == mode)
        .map(|(_, bindings)| bindings.iter().collect())
        .unwrap_or_default();
    bindings.sort_by_key(|(key, _)| !key.key_modifiers.is_empty());

    let (key_desc_separator, entry_separator) = hint_gaps(width);
    let key_desc_width = console::measure_text_width(key_desc_separator);
    let entry_separator_width = console::measure_text_width(entry_separator);
    let mut pages = Vec::new();
    let mut page = Vec::new();
    let mut page_width = 0;
    for (key, actions) in bindings {
        let mut key = key.to_string();
        let mut description = actions_label(actions);
        let mut entry_width = console::measure_text_width(&key)
            + key_desc_width
            + console::measure_text_width(&description);
        if entry_width > width {
            let minimum_description_width =
                console::measure_text_width(&description).clamp(1, MIN_HINT_DESCRIPTION_WIDTH);
            key = fit(
                &key,
                width
                    .saturating_sub(key_desc_width + minimum_description_width)
                    .max(1),
            );
            description = fit(
                &description,
                width
                    .saturating_sub(console::measure_text_width(&key) + key_desc_width)
                    .max(1),
            );
            entry_width = console::measure_text_width(&key)
                + key_desc_width
                + console::measure_text_width(&description);
        }
        if !page.is_empty() && page_width + entry_separator_width + entry_width > width {
            pages.push(page);
            page = Vec::new();
            page_width = 0;
        }
        page_width += usize::from(!page.is_empty()) * entry_separator_width + entry_width;
        page.push((key, description));
    }
    if !page.is_empty() {
        pages.push(page);
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }
    pages
}

fn action_label(action: &Action) -> String {
    match action {
        Action::SwitchToMode { input_mode } => format!("{input_mode:?} mode"),
        action => humanize(&action.to_string()),
    }
}

fn actions_label(actions: &[Action]) -> String {
    let labels = actions
        .iter()
        .filter(|action| actions.len() == 1 || !matches!(action, Action::SwitchToMode { .. }))
        .map(action_label)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        actions
            .first()
            .map(action_label)
            .unwrap_or_else(|| "Action".to_owned())
    } else {
        labels.join(" + ")
    }
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

#[derive(Clone, Copy)]
enum IdleSide {
    Left,
    Right,
}

struct IdleItem {
    command: String,
    reductions: Vec<usize>,
}

#[derive(Default)]
struct IdleRow {
    left: Vec<IdleItem>,
    right: Vec<IdleItem>,
    separator: FormattedPart,
}

impl IdleRow {
    fn from_config(configuration: &BTreeMap<String, String>) -> anyhow::Result<Option<Self>> {
        if !configuration.contains_key("hint_idle_left")
            && !configuration.contains_key("hint_idle_right")
        {
            return Ok(None);
        }

        let mut seen = BTreeMap::new();
        let left = idle_items("hint_idle_left", configuration, &mut seen)?;
        let right = idle_items("hint_idle_right", configuration, &mut seen)?;
        let separator = FormattedPart::from_format_string(
            configuration
                .get("hint_idle_separator")
                .map(String::as_str)
                .unwrap_or_default(),
            configuration,
        );

        Ok(Some(Self {
            left,
            right,
            separator,
        }))
    }

    fn fit(
        &self,
        widgets: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
        configuration: &BTreeMap<String, String>,
        cols: usize,
    ) -> (String, String) {
        let left_variants = render_idle_variants(&self.left, widgets, state, configuration);
        let right_variants = render_idle_variants(&self.right, widgets, state, configuration);
        let separator = self.separator.format_string(&self.separator.content);
        let mut left_levels = vec![0; self.left.len()];
        let mut right_levels = vec![0; self.right.len()];
        let mut steps = Vec::new();
        let mut order = 0;

        for (side, items) in [(IdleSide::Left, &self.left), (IdleSide::Right, &self.right)] {
            for (item_index, item) in items.iter().enumerate() {
                for priority in &item.reductions {
                    steps.push((*priority, order, side, item_index));
                    order += 1;
                }
            }
        }
        steps.sort_by_key(|(priority, order, _, _)| (*priority, *order));

        let render = |left_levels: &[usize], right_levels: &[usize]| {
            (
                join_idle_variants(&left_variants, left_levels, &separator),
                join_idle_variants(&right_variants, right_levels, &separator),
            )
        };
        let (left, right) = render(&left_levels, &right_levels);
        if status_fits(&left, &right, cols) {
            return (left, right);
        }

        let mut last = (left, right);
        for (_, _, side, item_index) in steps {
            let levels = match side {
                IdleSide::Left => &mut left_levels,
                IdleSide::Right => &mut right_levels,
            };
            levels[item_index] += 1;
            last = render(&left_levels, &right_levels);
            if status_fits(&last.0, &last.1, cols) {
                return last;
            }
        }

        fit_idle_pair(last.0, last.1, cols)
    }
}

fn idle_items(
    side: &str,
    configuration: &BTreeMap<String, String>,
    seen: &mut BTreeMap<String, ()>,
) -> anyhow::Result<Vec<IdleItem>> {
    let mut items = Vec::new();
    for name in configuration
        .get(side)
        .map(String::as_str)
        .unwrap_or_default()
        .split_whitespace()
    {
        if !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        {
            anyhow::bail!("Invalid idle item name: {name}");
        }
        if seen.insert(name.to_owned(), ()).is_some() {
            anyhow::bail!("Duplicate idle item: {name}");
        }

        let command_key = format!("hint_idle_{name}_command");
        let command = configuration
            .get(&command_key)
            .filter(|command| !command.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Missing {command_key}"))?;
        let command = if command.starts_with("command_") {
            command.to_owned()
        } else {
            format!("command_{command}")
        };
        let configured_command = format!("{command}_command");
        if !configuration.contains_key(&configured_command) {
            anyhow::bail!("Missing {configured_command}");
        }

        let reductions_key = format!("hint_idle_{name}_reductions");
        let reductions = configuration
            .get(&reductions_key)
            .map(String::as_str)
            .unwrap_or_default()
            .split(|character: char| character.is_whitespace() || character == ',')
            .filter(|priority| !priority.is_empty())
            .map(|priority| {
                priority.parse::<usize>().map_err(|_| {
                    anyhow::anyhow!("Invalid priority in {reductions_key}: {priority}")
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        if reductions
            .windows(2)
            .any(|priorities| priorities[0] >= priorities[1])
        {
            anyhow::bail!("{reductions_key} must be strictly increasing");
        }

        items.push(IdleItem {
            command,
            reductions,
        });
    }
    Ok(items)
}
fn normalize_idle_variants(mut variants: Vec<String>, expected: usize) -> Vec<String> {
    variants.truncate(expected);
    variants.resize(expected, variants.last().cloned().unwrap_or_default());
    for index in 1..variants.len() {
        if console::measure_text_width(&variants[index])
            > console::measure_text_width(&variants[index - 1])
        {
            variants[index] = variants[index - 1].clone();
        }
    }
    variants
}
fn render_idle_output(output: &str, configuration: &BTreeMap<String, String>) -> Vec<String> {
    if output.is_empty() {
        return vec![String::new()];
    }
    let mut previous = String::new();
    output
        .lines()
        .map(|variant| {
            if variant.is_empty() {
                return previous.clone();
            }
            let rendered = if variant == "@hide" {
                String::new()
            } else {
                formatted_parts_from_string_cached(variant, configuration)
                    .iter()
                    .map(|part| part.format_string(&part.content))
                    .collect()
            };
            previous.clone_from(&rendered);
            rendered
        })
        .collect()
}

fn render_idle_variants(
    items: &[IdleItem],
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &ZellijState,
    configuration: &BTreeMap<String, String>,
) -> Vec<Vec<String>> {
    items
        .iter()
        .map(|item| {
            let output = widgets
                .get("command")
                .map(|widget| widget.process(&item.command, state))
                .unwrap_or_default();
            let variants = render_idle_output(&output, configuration);
            normalize_idle_variants(variants, item.reductions.len() + 1)
        })
        .collect()
}

fn join_idle_variants(variants: &[Vec<String>], levels: &[usize], separator: &str) -> String {
    variants
        .iter()
        .zip(levels)
        .filter_map(|(variants, level)| variants.get(*level))
        .filter(|variant| !variant.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(separator)
}

fn status_fits(left: &str, right: &str, cols: usize) -> bool {
    console::measure_text_width(left) + console::measure_text_width(right) <= cols
}

fn fit_idle_pair(left: String, right: String, cols: usize) -> (String, String) {
    if status_fits(&left, &right, cols) {
        return (left, right);
    }
    let right_width = console::measure_text_width(&right);
    if right_width >= cols {
        return (
            String::new(),
            console::truncate_str(&right, cols, "…").into_owned(),
        );
    }
    (
        console::truncate_str(&left, cols - right_width, "…").into_owned(),
        right,
    )
}

fn fit(text: &str, width: usize) -> String {
    console::truncate_str(text, width, "…").into_owned()
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
    fn compresses_headers_before_complete_hints() {
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

        let pages = hint_pages(&mode.mode, &mode.keybinds, 20);
        assert_eq!(
            pages,
            vec![
                vec![("a".into(), "Normal mode".into())],
                vec![("Ctrl b".into(), "Tmux mode".into())],
            ]
        );
        let narrow = hint_pages(&mode.mode, &mode.keybinds, 7);
        assert_eq!(narrow[0][0], ("a".into(), "Norm…".into()));
        assert_eq!(narrow[1][0], ("C…".into(), "Tmu…".into()));
        for width in 3..=30 {
            let pages = hint_pages(&mode.mode, &mode.keybinds, width);
            assert!(
                pages
                    .iter()
                    .flatten()
                    .all(|(key, description)| !key.is_empty() && !description.is_empty()),
                "incomplete hint at width {width}",
            );
            let (key_desc_separator, entry_separator) = hint_gaps(width);
            for page in pages {
                let page_width = page
                    .iter()
                    .map(|(key, description)| {
                        console::measure_text_width(key)
                            + console::measure_text_width(key_desc_separator)
                            + console::measure_text_width(description)
                    })
                    .sum::<usize>()
                    + console::measure_text_width(entry_separator) * page.len().saturating_sub(1);
                assert!(page_width <= width, "overflow at width {width}");
            }
        }
        for width in 0..3 {
            assert_eq!(
                hint_pages(&mode.mode, &mode.keybinds, width),
                vec![Vec::new()]
            );
        }
        let unicode_mode = ModeInfo {
            mode: InputMode::Pane,
            keybinds: vec![(
                InputMode::Pane,
                vec![(
                    "界".parse().unwrap(),
                    vec![Action::SwitchToMode {
                        input_mode: InputMode::Search,
                    }],
                )],
            )],
            ..ModeInfo::default()
        };
        assert_eq!(
            hint_pages(&unicode_mode.mode, &unicode_mode.keybinds, 14),
            vec![vec![("界".into(), "Search mode".into())]],
        );
        for width in 3..=30 {
            let pages = hint_pages(&unicode_mode.mode, &unicode_mode.keybinds, width);
            let (key_desc_separator, entry_separator) = hint_gaps(width);
            for page in pages {
                let page_width = page
                    .iter()
                    .map(|(key, description)| {
                        assert!(!key.is_empty() && !description.is_empty());
                        console::measure_text_width(key)
                            + console::measure_text_width(key_desc_separator)
                            + console::measure_text_width(description)
                    })
                    .sum::<usize>()
                    + console::measure_text_width(entry_separator) * page.len().saturating_sub(1);
                assert!(page_width <= width, "unicode overflow at width {width}");
            }
        }
        assert!(matches!(
            hint_header_layout("PANE", 30, &[(1, 11), (6, 9)], 60).0,
            HintHeader::Full
        ));
        assert!(matches!(
            hint_header_layout("PANE", 30, &[(1, 11), (6, 9)], 46).0,
            HintHeader::Compact
        ));
        let (header, _) = hint_header_layout("PANE", 30, &[(1, 11), (6, 9)], 41);
        assert!(matches!(header, HintHeader::Page));
        assert_eq!(format_hint_header(header, "PANE", 1, 30), " 1/30 ");
        assert!(matches!(
            hint_header_layout("PANE", 30, &[(1, 11), (6, 9)], 33).0,
            HintHeader::None
        ));
        assert!(matches!(
            hint_header_layout("PANE", 30, &[(1, 4), (1, 4), (30, 4)], 36).0,
            HintHeader::None
        ));
        let mut previous_header = 0;
        for cols in 0..=80 {
            let header = hint_header_layout("PANE", 30, &[(1, 9), (1, 9)], cols).0;
            let header_rank = match header {
                HintHeader::None => 0,
                HintHeader::Page => 1,
                HintHeader::Compact => 2,
                HintHeader::Full => 3,
            };
            assert!(
                header_rank >= previous_header,
                "header regressed at width {cols}",
            );
            previous_header = header_rank;
        }
        let mut state = State {
            keybinds: mode.keybinds.clone(),
            hint_visible: true,
            hint_formats: Some(HintFormats::new(&BTreeMap::new())),
            ..State::default()
        };
        state.state.mode = mode;
        for cols in 0..=80 {
            state.hint_page = 0;
            let line = state.hint_line(cols);
            assert!(
                console::measure_text_width(&line) <= cols,
                "rendered line overflow at width {cols}",
            );
        }
    }

    struct TestCommands(BTreeMap<String, String>);

    impl Widget for TestCommands {
        fn process(&self, name: &str, _state: &ZellijState) -> String {
            self.0.get(name).cloned().unwrap_or_default()
        }

        fn process_click(&self, _name: &str, _state: &ZellijState, _pos: usize) {}
    }

    fn idle_row_fixture() -> (
        IdleRow,
        BTreeMap<String, String>,
        BTreeMap<String, Arc<dyn Widget>>,
    ) {
        let configuration = BTreeMap::from([
            ("hint_idle_left".into(), "vcs pi".into()),
            ("hint_idle_right".into(), "load".into()),
            ("hint_idle_separator".into(), "|".into()),
            ("hint_idle_vcs_command".into(), "vcs".into()),
            ("hint_idle_vcs_reductions".into(), "0 4 5 6".into()),
            ("hint_idle_load_command".into(), "load".into()),
            ("hint_idle_load_reductions".into(), "1 2 3".into()),
            ("hint_idle_pi_command".into(), "pi".into()),
            ("hint_idle_pi_reductions".into(), "7 8 9".into()),
            ("command_vcs_command".into(), "ignored".into()),
            ("command_load_command".into(), "ignored".into()),
            ("command_pi_command".into(), "ignored".into()),
        ]);
        let row = IdleRow::from_config(&configuration).unwrap().unwrap();
        let commands = TestCommands(BTreeMap::from([
            ("command_vcs".into(), "VVVVVV\nVVVV\nVVV\nVV\n@hide".into()),
            ("command_load".into(), "LLLLLL\nLLLL\nLL\n@hide".into()),
            ("command_pi".into(), "PPPPPP\nPPPP\nPPP\nP".into()),
        ]));
        let widgets = BTreeMap::from([("command".into(), Arc::new(commands) as Arc<dyn Widget>)]);
        (row, configuration, widgets)
    }

    #[test]
    fn idle_commands_reduce_in_configured_global_order() {
        let (row, configuration, widgets) = idle_row_fixture();
        let state = ZellijState::default();
        let expected = [
            (19, ("VVVVVV|PPPPPP", "LLLLLL")),
            (17, ("VVVV|PPPPPP", "LLLLLL")),
            (15, ("VVVV|PPPPPP", "LLLL")),
            (13, ("VVVV|PPPPPP", "LL")),
            (11, ("VVVV|PPPPPP", "")),
            (10, ("VVV|PPPPPP", "")),
            (9, ("VV|PPPPPP", "")),
            (6, ("PPPPPP", "")),
            (4, ("PPPP", "")),
            (3, ("PPP", "")),
            (1, ("P", "")),
        ];

        for (cols, (left, right)) in expected {
            assert_eq!(
                row.fit(&widgets, &state, &configuration, cols),
                (left.to_owned(), right.to_owned()),
            );
        }
    }

    #[test]
    fn idle_configuration_rejects_invalid_items_and_priorities() {
        let mut configuration = BTreeMap::from([
            ("hint_idle_left".into(), "vcs".into()),
            ("hint_idle_vcs_command".into(), "vcs".into()),
            ("command_vcs_command".into(), "ignored".into()),
            ("hint_idle_vcs_reductions".into(), "2 1".into()),
        ]);
        assert!(
            IdleRow::from_config(&configuration)
                .err()
                .unwrap()
                .to_string()
                .contains("strictly increasing")
        );

        configuration.insert("hint_idle_vcs_reductions".into(), "0".into());
        configuration.insert("hint_idle_right".into(), "vcs".into());
        assert!(
            IdleRow::from_config(&configuration)
                .err()
                .unwrap()
                .to_string()
                .contains("Duplicate idle item")
        );
    }
    #[test]
    fn malformed_idle_variants_never_hide_or_expand_implicitly() {
        assert_eq!(
            normalize_idle_variants(vec!["long".into(), "x".into()], 4),
            vec!["long", "x", "x", "x"]
        );
        assert_eq!(
            normalize_idle_variants(vec!["x".into(), "longer".into(), "ignored".into()], 2),
            vec!["x", "x"]
        );
        assert_eq!(
            render_idle_output("long\n\nx\n@hide", &BTreeMap::new()),
            vec!["long", "long", "x", ""]
        );
    }

    #[test]
    fn static_idle_fallback_keeps_right_side_visible() {
        assert_eq!(
            fit_idle_pair("abcdef".into(), "XYZ".into(), 6),
            ("ab…".into(), "XYZ".into()),
        );
        assert_eq!(
            fit_idle_pair("abcdef".into(), "XYZ".into(), 2),
            (String::new(), "X…".into()),
        );
    }
}
