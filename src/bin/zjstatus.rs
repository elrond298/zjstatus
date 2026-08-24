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
        let output = self
            .hint_idle_parts
            .iter_mut()
            .fold(String::new(), |output, part| {
                output + &part.format_string_with_widgets(&self.widget_map, &self.state)
            });
        let used = console::measure_text_width(&output);
        let Some(formats) = &self.hint_formats else {
            return output;
        };
        output
            + &formats
                .space
                .format_string(&" ".repeat(cols.saturating_sub(used)))
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
}
