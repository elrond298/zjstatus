use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use itertools::Itertools;
use regex::Regex;
use zellij_tile::prelude::*;

use crate::{
    border::{BorderConfig, BorderPosition, parse_border_config},
    render::FormattedPart,
    widgets::{command::CommandResult, notification, widget::Widget},
};
use chrono::{DateTime, Local};

#[derive(Default, Debug, Clone)]
pub struct ZellijState {
    pub cols: usize,
    pub command_results: BTreeMap<String, CommandResult>,
    pub pipe_results: BTreeMap<String, String>,
    pub mode: ModeInfo,
    pub panes: PaneManifest,
    pub plugin_uuid: String,
    pub tabs: Vec<TabInfo>,
    pub sessions: Vec<SessionInfo>,
    pub start_time: DateTime<Local>,
    pub incoming_notification: Option<notification::Message>,
    pub cache_mask: u8,
    pub focused_pane_id: Option<PaneId>,
    pub focused_pane_cwd: Option<std::path::PathBuf>,
}

#[derive(Clone, Debug, Hash, Ord, Eq, PartialEq, PartialOrd, Copy)]
pub enum Part {
    Left,
    Center,
    Right,
}

impl FromStr for Part {
    fn from_str(part: &str) -> Result<Self> {
        match part {
            "left" => Ok(Part::Left),
            "center" => Ok(Part::Center),
            "right" => Ok(Part::Right),
            _ => anyhow::bail!("Invalid region: {part}"),
        }
    }

    type Err = anyhow::Error;
}

impl Part {
    fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Center => 1,
            Self::Right => 2,
        }
    }
}

#[derive(Debug)]
struct BarOutput {
    left: String,
    center: String,
    right: String,
    levels: [usize; 3],
    clicks_disabled: bool,
}

pub enum UpdateEventMask {
    Always = 0b10000000,
    Mode = 0b00000001,
    Tab = 0b00000011,
    Command = 0b00000100,
    Session = 0b00001000,
    None = 0b00000000,
}

pub fn event_mask_from_widget_name(name: &str) -> u8 {
    match name {
        "command" => UpdateEventMask::Always as u8,
        "datetime" => UpdateEventMask::Always as u8,
        "mode" => UpdateEventMask::Mode as u8,
        "notifications" => UpdateEventMask::Always as u8,
        "session" => UpdateEventMask::Mode as u8,
        "swap_layout" => UpdateEventMask::Tab as u8,
        "tabs" => UpdateEventMask::Tab as u8,
        "pipe" => UpdateEventMask::Always as u8,
        _ => UpdateEventMask::None as u8,
    }
}

#[derive(Default, Debug)]
pub struct ModuleConfig {
    pub left_parts_config: String,
    pub left_parts: Vec<FormattedPart>,
    pub center_parts_config: String,
    pub center_parts: Vec<FormattedPart>,
    pub right_parts_config: String,
    pub right_parts: Vec<FormattedPart>,
    pub format_space: FormattedPart,
    pub hide_frame_for_single_pane: bool,
    pub hide_frame_except_for_search: bool,
    pub hide_frame_except_for_fullscreen: bool,
    pub hide_frame_except_for_scroll: bool,
    pub border: BorderConfig,
    pub shrink_order: Vec<Part>,
    pub hide_on_overlength: bool,
    responsive_parts: BTreeMap<Part, Vec<Vec<FormattedPart>>>,
    notification_show_interval: i64,
}

impl ModuleConfig {
    pub fn new(config: &BTreeMap<String, String>) -> anyhow::Result<Self> {
        let format_space_config = match config.get("format_space") {
            Some(space_config) => space_config,
            None => "",
        };

        let hide_frame_for_single_pane = match config.get("hide_frame_for_single_pane") {
            Some(toggle) => toggle == "true",
            None => false,
        };
        let hide_frame_except_for_search = match config.get("hide_frame_except_for_search") {
            Some(toggle) => toggle == "true",
            None => false,
        };
        let hide_frame_except_for_fullscreen = match config.get("hide_frame_except_for_fullscreen")
        {
            Some(toggle) => toggle == "true",
            None => false,
        };
        let hide_frame_except_for_scroll = match config.get("hide_frame_except_for_scroll") {
            Some(toggle) => toggle == "true",
            None => false,
        };

        let left_parts_config = match config.get("format_left") {
            Some(conf) => conf,
            None => "",
        };

        let right_parts_config = match config.get("format_right") {
            Some(conf) => conf,
            None => "",
        };

        let center_parts_config = match config.get("format_center") {
            Some(conf) => conf,
            None => "",
        };

        reject_removed_responsive_config(config)?;
        let shrink_levels = parse_shrink_levels(config)?;
        let responsive_enabled = !shrink_levels.is_empty();
        let shrink_order = parse_shrink_order(config, responsive_enabled)?;
        let responsive_parts = if responsive_enabled {
            BTreeMap::from([
                (
                    Part::Left,
                    responsive_formats("format_left", left_parts_config, &shrink_levels, config),
                ),
                (
                    Part::Center,
                    responsive_formats(
                        "format_center",
                        center_parts_config,
                        &shrink_levels,
                        config,
                    ),
                ),
                (
                    Part::Right,
                    responsive_formats("format_right", right_parts_config, &shrink_levels, config),
                ),
            ])
        } else {
            BTreeMap::new()
        };
        let notification_show_interval = config
            .get("notification_show_interval")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(5);

        let hide_on_overlength = match config.get("format_hide_on_overlength") {
            Some(opt) => opt == "true",
            None => false,
        };

        let border_config = parse_border_config(config).unwrap_or_default();

        Ok(Self {
            left_parts_config: left_parts_config.to_owned(),
            left_parts: parts_from_config(Some(&left_parts_config.to_owned()), config),
            center_parts_config: center_parts_config.to_owned(),
            center_parts: parts_from_config(Some(&center_parts_config.to_owned()), config),
            right_parts_config: right_parts_config.to_owned(),
            right_parts: parts_from_config(Some(&right_parts_config.to_owned()), config),
            format_space: FormattedPart::from_format_string(format_space_config, config),
            hide_frame_for_single_pane,
            hide_frame_except_for_search,
            hide_frame_except_for_fullscreen,
            hide_frame_except_for_scroll,
            border: border_config,
            shrink_order,
            hide_on_overlength,
            responsive_parts,
            notification_show_interval,
        })
    }

    fn select_bar_output(
        &mut self,
        state: &ZellijState,
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
    ) -> BarOutput {
        if self.responsive_parts.is_empty() {
            let mut output = BarOutput {
                left: render_parts(&mut self.left_parts, widget_map, state, 0),
                center: render_parts(&mut self.center_parts, widget_map, state, 0),
                right: render_parts(&mut self.right_parts, widget_map, state, 0),
                levels: [0; 3],
                clicks_disabled: false,
            };
            if self.hide_on_overlength {
                (output.left, output.center, output.right) =
                    self.trim_output(&output.left, &output.center, &output.right, state.cols);
            }
            return output;
        }

        let max_level = self
            .responsive_parts
            .values()
            .next()
            .map(|levels| levels.len().saturating_sub(1))
            .unwrap_or_default();
        let mut levels = [0; 3];
        let mut output = self.render_responsive_levels(state, widget_map, levels);
        if bar_outputs_fit(&output.left, &output.center, &output.right, state.cols) {
            return output;
        }

        let reduction_order = self.shrink_order.clone();
        for target_level in 1..=max_level {
            for part in &reduction_order {
                levels[part.index()] = target_level;
                output = self.render_responsive_levels(state, widget_map, levels);
                if bar_outputs_fit(&output.left, &output.center, &output.right, state.cols) {
                    return output;
                }
            }
        }

        self.minimum_bar_output(state, widget_map, output)
    }

    fn render_responsive_levels(
        &mut self,
        state: &ZellijState,
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
        levels: [usize; 3],
    ) -> BarOutput {
        let notification_is_rendered = [Part::Left, Part::Center, Part::Right]
            .into_iter()
            .enumerate()
            .any(|(index, part)| {
                parts_contain_widget(
                    &self.responsive_parts[&part][levels[index]],
                    "notifications",
                )
            });
        let mut clicks_disabled = false;
        let left = render_parts(
            &mut self.responsive_parts.get_mut(&Part::Left).unwrap()[levels[0]],
            widget_map,
            state,
            levels[0],
        );
        let center = render_parts(
            &mut self.responsive_parts.get_mut(&Part::Center).unwrap()[levels[1]],
            widget_map,
            state,
            levels[1],
        );
        let mut right = render_parts(
            &mut self.responsive_parts.get_mut(&Part::Right).unwrap()[levels[2]],
            widget_map,
            state,
            levels[2],
        );
        if self.notification_visible(state)
            && self.configured_widget_region("notifications").is_some()
            && !notification_is_rendered
            && let Some(notifications) = widget_map.get("notifications")
        {
            let notification = notifications.process("notifications", state);
            if !notification.is_empty() {
                let separator = (!right.is_empty()).then(|| self.format_space.format_string(" "));
                right = notification + separator.as_deref().unwrap_or_default() + &right;
                clicks_disabled = true;
            }
        }
        BarOutput {
            left,
            center,
            right,
            levels,
            clicks_disabled,
        }
    }

    fn configured_widget_region(&self, widget: &str) -> Option<Part> {
        [Part::Left, Part::Center, Part::Right]
            .into_iter()
            .find(|part| {
                self.responsive_parts.get(part).is_some_and(|levels| {
                    levels
                        .iter()
                        .any(|parts| parts_contain_widget(parts, widget))
                })
            })
    }

    fn minimum_bar_output(
        &self,
        state: &ZellijState,
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
        mut output: BarOutput,
    ) -> BarOutput {
        let tab_region = self.configured_widget_region("tabs");
        let tab = tab_region
            .and_then(|_| widget_map.get("tabs"))
            .map(|tabs| tabs.process_at_level("tabs", state, 4))
            .unwrap_or_default();
        if let Some(tab_region) = tab_region
            && !tab.is_empty()
        {
            match tab_region {
                Part::Left => output.left.clear(),
                Part::Center => output.center.clear(),
                Part::Right => output.right.clear(),
            }
            output.center.clone_from(&tab);
        }

        let notification = if self.notification_visible(state)
            && self.configured_widget_region("notifications").is_some()
        {
            widget_map
                .get("notifications")
                .map(|widget| widget.process("notifications", state))
                .unwrap_or_default()
        } else {
            String::new()
        };
        if !notification.is_empty() {
            let tab_width = console::measure_text_width(&tab);
            if tab_width >= state.cols {
                output.left.clear();
                output.center = console::truncate_str(&tab, state.cols, "").into_owned();
                output.right.clear();
                output.clicks_disabled = true;
                return output;
            }
            let separator = if tab.is_empty() || notification.is_empty() {
                String::new()
            } else {
                self.format_space.format_string(" ")
            };
            let available = state
                .cols
                .saturating_sub(tab_width + console::measure_text_width(&separator));
            output.left = tab + &separator;
            output.center.clear();
            output.right = console::truncate_str(&notification, available, "…").into_owned();
            output.clicks_disabled = true;
            return output;
        }

        if bar_outputs_fit(&output.left, &output.center, &output.right, state.cols) {
            output.clicks_disabled = true;
            return output;
        }

        for part in &self.shrink_order {
            if *part == Part::Center {
                continue;
            }
            match part {
                Part::Left => output.left.clear(),
                Part::Right => output.right.clear(),
                Part::Center => {}
            }
            if bar_outputs_fit(&output.left, &output.center, &output.right, state.cols) {
                output.clicks_disabled = true;
                return output;
            }
        }
        output.left.clear();
        output.right.clear();
        output.center = console::truncate_str(&output.center, state.cols, "").into_owned();
        output.clicks_disabled = true;
        output
    }

    fn notification_visible(&self, state: &ZellijState) -> bool {
        state.incoming_notification.as_ref().is_some_and(|message| {
            message.received_at.timestamp() + self.notification_show_interval
                >= Local::now().timestamp()
        })
    }

    fn parts_at(&self, part: Part, level: usize) -> &[FormattedPart] {
        if !self.responsive_parts.is_empty() {
            return &self.responsive_parts[&part][level];
        }
        match part {
            Part::Left => &self.left_parts,
            Part::Center => &self.center_parts,
            Part::Right => &self.right_parts,
        }
    }

    pub fn handle_mouse_action(
        &mut self,
        state: ZellijState,
        mouse: Mouse,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
    ) {
        let click_pos = match mouse {
            Mouse::ScrollUp(_)
            | Mouse::ScrollDown(_)
            | Mouse::ScrollLeft(_)
            | Mouse::ScrollRight(_) => return,
            Mouse::LeftClick(_, y) => y,
            Mouse::RightClick(_, y) => y,
            Mouse::Hold(_, y) => y,
            Mouse::Release(_, y) => y,
            Mouse::Hover(_, _) => return,
        };

        let output = self.select_bar_output(&state, &widget_map);
        if output.clicks_disabled {
            return;
        }
        let BarOutput {
            left: output_left,
            center: output_center,
            right: output_right,
            levels,
            ..
        } = output;

        let mut offset = console::measure_text_width(&output_left);

        self.process_widget_click(
            click_pos,
            self.parts_at(Part::Left, levels[0]),
            &widget_map,
            &state,
            0,
            levels[0],
        );

        if click_pos <= offset {
            return;
        }

        if !output_center.is_empty() {
            tracing::debug!("widgetclick center");
            offset += console::measure_text_width(&self.get_spacer_left(
                &output_left,
                &output_center,
                state.cols,
            ));

            offset += self.process_widget_click(
                click_pos,
                self.parts_at(Part::Center, levels[1]),
                &widget_map,
                &state,
                offset,
                levels[1],
            );

            if click_pos <= offset {
                return;
            }

            offset += console::measure_text_width(&self.get_spacer_right(
                &output_right,
                &output_center,
                state.cols,
            ));
        } else {
            offset += console::measure_text_width(&self.get_spacer(
                &output_left,
                &output_right,
                state.cols,
            ));
        }

        self.process_widget_click(
            click_pos,
            self.parts_at(Part::Right, levels[2]),
            &widget_map,
            &state,
            offset,
            levels[2],
        );
    }

    fn process_widget_click(
        &self,
        click_pos: usize,
        widgets: &[FormattedPart],
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
        offset: usize,
        level: usize,
    ) -> usize {
        let widget_string = widgets.iter().fold(String::new(), |a, b| a + &b.content);

        let mut rendered_output = widget_string.clone();

        let tokens: Vec<String> = widget_map.keys().map(|k| k.to_owned()).collect();

        let widgets_regex = Regex::new("(\\{[a-z_0-9]+\\})").unwrap();
        for widget in widgets_regex.captures_iter(widget_string.as_str()) {
            let match_name = widget.get(0).unwrap().as_str();
            let widget_key = match_name.trim_matches(|c| c == '{' || c == '}');
            let mut widget_key_name = widget_key;

            if widget_key.starts_with("command_") {
                widget_key_name = "command";
            }

            if widget_key.starts_with("pipe_") {
                widget_key_name = "pipe";
            }

            if !tokens.contains(&widget_key_name.to_owned()) {
                continue;
            }

            let wid = match widget_map.get(widget_key_name) {
                Some(wid) => wid,
                None => continue,
            };

            let pos = match rendered_output.find(match_name) {
                Some(_pos) => {
                    let pref = rendered_output.split(match_name).collect::<Vec<&str>>()[0];
                    console::measure_text_width(pref)
                }
                None => continue,
            };

            let wid_res = wid.process_at_level(widget_key, state, level);
            rendered_output = rendered_output.replace(match_name, &wid_res);

            if click_pos < pos + offset
                || click_pos > pos + offset + console::measure_text_width(&wid_res)
            {
                continue;
            }

            wid.process_click_at_level(widget_key, state, click_pos - (pos + offset), level);
        }

        console::measure_text_width(&rendered_output)
    }

    pub fn render_bar(
        &mut self,
        state: ZellijState,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
    ) -> String {
        if self.left_parts.is_empty() && self.center_parts.is_empty() && self.right_parts.is_empty()
        {
            return "No configuration found. See https://github.com/dj95/zjstatus/wiki/3-%E2%80%90-Configuration for more info".to_string();
        }

        let BarOutput {
            left: output_left,
            center: output_center,
            right: output_right,
            ..
        } = self.select_bar_output(&state, &widget_map);

        if self.border.enabled {
            let mut border_top = "".to_owned();
            if self.border.enabled && self.border.position == BorderPosition::Top {
                border_top = format!("{}\n", self.border.draw(state.cols));
            }

            let mut border_bottom = "".to_owned();
            if self.border.enabled && self.border.position == BorderPosition::Bottom {
                border_bottom = format!("\n{}", self.border.draw(state.cols));
            }

            if !output_center.is_empty() {
                return format!(
                    "{}{}{}{}{}{}{}",
                    border_top,
                    output_left,
                    self.get_spacer_left(&output_left, &output_center, state.cols),
                    output_center,
                    self.get_spacer_right(&output_right, &output_center, state.cols),
                    output_right,
                    border_bottom,
                );
            }

            return format!(
                "{}{}{}{}{}",
                border_top,
                output_left,
                self.get_spacer(&output_left, &output_right, state.cols),
                output_right,
                border_bottom,
            );
        }

        if !output_center.is_empty() {
            return format!(
                "{}{}{}{}{}",
                output_left,
                self.get_spacer_left(&output_left, &output_center, state.cols),
                output_center,
                self.get_spacer_right(&output_right, &output_center, state.cols),
                output_right,
            );
        }

        format!(
            "{}{}{}",
            output_left,
            self.get_spacer(&output_left, &output_right, state.cols),
            output_right,
        )
    }

    fn trim_output(
        &self,
        output_left: &str,
        output_center: &str,
        output_right: &str,
        cols: usize,
    ) -> (String, String, String) {
        let center_pos = (cols as f32 / 2.0).floor() as usize;

        let mut output = BTreeMap::from([
            (Part::Left, output_left.to_owned()),
            (Part::Center, output_center.to_owned()),
            (Part::Right, output_right.to_owned()),
        ]);

        let combinations = [
            (self.shrink_order[0], self.shrink_order[1]),
            (self.shrink_order[1], self.shrink_order[2]),
            (self.shrink_order[0], self.shrink_order[2]),
        ];

        for win in combinations.iter() {
            let (a, b) = win;

            let part_a = output.get(a).unwrap();
            let part_b = output.get(b).unwrap();

            let a_count = console::measure_text_width(part_a);
            let b_count = console::measure_text_width(part_b);

            let overlap = match (a, b) {
                (Part::Left, Part::Right) => a_count + b_count > cols,
                (Part::Right, Part::Left) => a_count + b_count > cols,
                (Part::Left, Part::Center) => a_count > center_pos - (b_count / 2),
                (Part::Center, Part::Left) => b_count > center_pos - (a_count / 2),
                (Part::Right, Part::Center) => a_count > center_pos - (b_count / 2),
                (Part::Center, Part::Right) => b_count > center_pos - (a_count / 2),
                _ => false,
            };

            if overlap {
                output.insert(*a, "".to_owned());
            }
        }

        output.values().cloned().collect_tuple().unwrap()
    }

    #[tracing::instrument(skip_all)]
    fn get_spacer_left(&self, output_left: &str, output_center: &str, cols: usize) -> String {
        let text_count = console::measure_text_width(output_left)
            + (console::measure_text_width(output_center) as f32 / 2.0).floor() as usize;

        let center_pos = (cols as f32 / 2.0).floor() as usize;

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = center_pos.saturating_sub(text_count);

        tracing::debug!("space_count: {:?}", space_count);
        self.format_space.format_string(&" ".repeat(space_count))
    }

    #[tracing::instrument(skip_all)]
    fn get_spacer_right(&self, output_right: &str, output_center: &str, cols: usize) -> String {
        let text_count = console::measure_text_width(output_right)
            + (console::measure_text_width(output_center) as f32 / 2.0).ceil() as usize;

        let center_pos = (cols as f32 / 2.0).ceil() as usize;

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = center_pos.saturating_sub(text_count);

        tracing::debug!("space_count: {:?}", space_count);
        self.format_space.format_string(&" ".repeat(space_count))
    }

    fn get_spacer(&self, output_left: &str, output_right: &str, cols: usize) -> String {
        let text_count =
            console::measure_text_width(output_left) + console::measure_text_width(output_right);

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = cols.saturating_sub(text_count);

        self.format_space.format_string(&" ".repeat(space_count))
    }
}

fn render_parts(
    parts: &mut [FormattedPart],
    widget_map: &BTreeMap<String, Arc<dyn Widget>>,
    state: &ZellijState,
    level: usize,
) -> String {
    parts.iter_mut().fold(String::new(), |mut output, part| {
        output.push_str(&part.format_string_with_widgets_at_level(widget_map, state, level));
        output
    })
}

fn parts_contain_widget(parts: &[FormattedPart], widget: &str) -> bool {
    let token = format!("{{{widget}}}");
    parts.iter().any(|part| part.content.contains(&token))
}

fn bar_outputs_fit(left: &str, center: &str, right: &str, cols: usize) -> bool {
    let left_width = console::measure_text_width(left);
    let center_width = console::measure_text_width(center);
    let right_width = console::measure_text_width(right);
    if center.is_empty() {
        return left_width + right_width <= cols;
    }
    let center_start = (cols / 2).saturating_sub(center_width / 2);
    let center_end = center_start + center_width;
    let right_start = cols.saturating_sub(right_width);
    left_width <= center_start && center_end <= right_start && left_width + right_width <= cols
}

fn reject_removed_responsive_config(config: &BTreeMap<String, String>) -> anyhow::Result<()> {
    for key in ["format_responsive", "format_precedence"] {
        if config.contains_key(key) {
            anyhow::bail!("{key} was removed; use format_shrink_levels and format_shrink_order");
        }
    }
    if let Some(key) = config.keys().find(|key| {
        ["format_left_", "format_center_", "format_right_"]
            .iter()
            .any(|prefix| {
                key.strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.parse::<usize>().is_ok())
            })
    }) {
        anyhow::bail!("{key} was removed; use a named format shrink level");
    }
    Ok(())
}

fn parse_shrink_levels(config: &BTreeMap<String, String>) -> anyhow::Result<Vec<String>> {
    let Some(configured) = config.get("format_shrink_levels") else {
        if let Some(key) = configured_named_format(config, &[]) {
            anyhow::bail!("{key} requires format_shrink_levels");
        }
        return Ok(Vec::new());
    };
    let levels = configured
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if levels.is_empty() {
        anyhow::bail!("format_shrink_levels must list at least one named level");
    }
    for level in &levels {
        if level == "full"
            || !level.starts_with(|character: char| character.is_ascii_alphabetic())
            || !level.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
        {
            anyhow::bail!("Invalid format shrink level: {level}");
        }
    }
    if levels.iter().unique().count() != levels.len() {
        anyhow::bail!("format_shrink_levels must contain unique names");
    }
    if let Some(key) = configured_named_format(config, &levels) {
        anyhow::bail!("Unknown format shrink level in {key}");
    }
    Ok(levels)
}

fn configured_named_format(config: &BTreeMap<String, String>, levels: &[String]) -> Option<String> {
    config.keys().find_map(|key| {
        ["format_left_", "format_center_", "format_right_"]
            .iter()
            .find_map(|prefix| key.strip_prefix(prefix))
            .filter(|suffix| !levels.iter().any(|level| level == suffix))
            .map(|_| key.clone())
    })
}

fn parse_shrink_order(
    config: &BTreeMap<String, String>,
    responsive_enabled: bool,
) -> anyhow::Result<Vec<Part>> {
    let Some(configured) = config.get("format_shrink_order") else {
        if responsive_enabled {
            anyhow::bail!("Missing format_shrink_order");
        }
        return Ok(vec![Part::Right, Part::Center, Part::Left]);
    };
    if !responsive_enabled {
        anyhow::bail!("format_shrink_order requires format_shrink_levels");
    }
    let order = configured
        .split_whitespace()
        .map(Part::from_str)
        .collect::<anyhow::Result<Vec<_>>>()?;
    if order.len() != 3 || order.iter().unique().count() != 3 {
        anyhow::bail!("format_shrink_order must contain left, center, and right exactly once");
    }
    Ok(order)
}

fn responsive_formats(
    name: &str,
    base: &str,
    levels: &[String],
    config: &BTreeMap<String, String>,
) -> Vec<Vec<FormattedPart>> {
    let mut current = base.to_owned();
    let mut formats = vec![parts_from_config(Some(&current), config)];
    for level in levels {
        if let Some(format) = config.get(&format!("{name}_{level}")) {
            current.clone_from(format);
        }
        formats.push(parts_from_config(Some(&current), config));
    }
    // Tabs reserve levels 3 and 4 for the full and compact locators.
    while formats.len() < 5 {
        formats.push(parts_from_config(Some(&current), config));
    }
    formats
}
fn parts_from_config(
    format: Option<&String>,
    config: &BTreeMap<String, String>,
) -> Vec<FormattedPart> {
    match format {
        Some(format) => match format.is_empty() {
            true => vec![],
            false => format
                .split("#[")
                .map(|s| FormattedPart::from_format_string(s, config))
                .collect(),
        },
        None => vec![],
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::widgets::{notification::NotificationWidget, tabs::TabsWidget};
    use anstyle::{Effects, RgbColor};
    #[test]
    fn test_formatted_part_from_string() {
        let input = "#[fg=#ff0000,bg=#00ff00,bold,italic]foo";

        let part = FormattedPart::from_format_string(input, &BTreeMap::new());

        assert_eq!(
            part,
            FormattedPart {
                fg: Some(RgbColor(255, 0, 0).into()),
                bg: Some(RgbColor(0, 255, 0).into()),
                effects: Effects::BOLD | Effects::ITALIC,
                content: "foo".to_owned(),
                ..Default::default()
            },
        )
    }

    fn responsive_config() -> BTreeMap<String, String> {
        [
            ("format_shrink_levels", "compact minimal locator tiny"),
            ("format_shrink_order", "right center left"),
            ("format_left", "LLLL"),
            ("format_left_compact", "L1"),
            ("format_left_minimal", "L"),
            ("format_center", "CCCC"),
            ("format_center_compact", "C1"),
            ("format_center_minimal", "C"),
            ("format_right", "RRRR"),
            ("format_right_compact", "R1"),
            ("format_right_minimal", "R"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
    }

    #[test]
    fn responsive_bar_completes_a_round_before_the_next_level() {
        let mut config = ModuleConfig::new(&responsive_config()).unwrap();
        let widgets = BTreeMap::<String, Arc<dyn Widget>>::new();

        let level_one = config.select_bar_output(
            &ZellijState {
                cols: 9,
                ..Default::default()
            },
            &widgets,
        );
        assert_eq!(level_one.levels, [1, 1, 1]);
        assert_eq!(
            (level_one.left, level_one.center, level_one.right),
            ("L1".to_owned(), "C1".to_owned(), "R1".to_owned())
        );

        let level_two = config.select_bar_output(
            &ZellijState {
                cols: 4,
                ..Default::default()
            },
            &widgets,
        );
        assert_eq!(level_two.levels, [1, 2, 2]);
        assert_eq!(
            (level_two.left, level_two.center, level_two.right),
            ("L1".to_owned(), "C".to_owned(), "R".to_owned())
        );
    }

    #[test]
    fn responsive_right_drops_datetime_before_hostname() {
        let config = BTreeMap::from([
            (
                "format_shrink_levels".to_owned(),
                "compact minimal".to_owned(),
            ),
            (
                "format_shrink_order".to_owned(),
                "right center left".to_owned(),
            ),
            (
                "format_right".to_owned(),
                "notification host datetime".to_owned(),
            ),
            (
                "format_right_compact".to_owned(),
                "notification host".to_owned(),
            ),
            ("format_right_minimal".to_owned(), "notification".to_owned()),
        ]);
        let mut module = ModuleConfig::new(&config).unwrap();
        let output = module.select_bar_output(
            &ZellijState {
                cols: 18,
                ..Default::default()
            },
            &BTreeMap::new(),
        );

        assert_eq!(output.levels, [0, 0, 1]);
        assert_eq!(output.right, "notification host");
    }

    #[test]
    fn responsive_levels_cannot_drop_a_current_notification() {
        let config = BTreeMap::from([
            ("format_shrink_levels".to_owned(), "compact".to_owned()),
            (
                "format_shrink_order".to_owned(),
                "right center left".to_owned(),
            ),
            (
                "format_right".to_owned(),
                "{notifications} datetime".to_owned(),
            ),
            ("format_right_compact".to_owned(), "host".to_owned()),
            (
                "notification_format_unread".to_owned(),
                "{message}".to_owned(),
            ),
            ("notification_show_interval".to_owned(), "60".to_owned()),
        ]);
        let mut module = ModuleConfig::new(&config).unwrap();
        let widgets = BTreeMap::from([(
            "notifications".to_owned(),
            Arc::new(NotificationWidget::new(&config)) as Arc<dyn Widget>,
        )]);
        let output = module.select_bar_output(
            &ZellijState {
                cols: 11,
                incoming_notification: Some(notification::Message {
                    body: "deploy".to_owned(),
                    received_at: Local::now(),
                }),
                ..Default::default()
            },
            &widgets,
        );

        assert_eq!(output.levels, [0, 0, 1]);
        assert_eq!(output.right, "deploy host");
        assert!(output.clicks_disabled);
    }

    #[test]
    fn notification_configured_only_in_named_level_is_still_retained() {
        let config = BTreeMap::from([
            (
                "format_shrink_levels".to_owned(),
                "compact minimal".to_owned(),
            ),
            (
                "format_shrink_order".to_owned(),
                "right center left".to_owned(),
            ),
            ("format_left".to_owned(), "LLLL".to_owned()),
            ("format_right".to_owned(), "datetime".to_owned()),
            (
                "format_right_compact".to_owned(),
                "{notifications} host".to_owned(),
            ),
            ("format_right_minimal".to_owned(), String::new()),
            (
                "notification_format_unread".to_owned(),
                "{message}".to_owned(),
            ),
            ("notification_show_interval".to_owned(), "60".to_owned()),
        ]);
        let mut module = ModuleConfig::new(&config).unwrap();
        let widgets = BTreeMap::from([(
            "notifications".to_owned(),
            Arc::new(NotificationWidget::new(&config)) as Arc<dyn Widget>,
        )]);
        let output = module.select_bar_output(
            &ZellijState {
                cols: 10,
                incoming_notification: Some(notification::Message {
                    body: "deploy".to_owned(),
                    received_at: Local::now(),
                }),
                ..Default::default()
            },
            &widgets,
        );

        assert_eq!(output.levels, [1, 1, 2]);
        assert_eq!(output.right, "deploy");
        assert!(output.clicks_disabled);
    }

    #[test]
    fn named_shrink_config_rejects_ambiguous_or_removed_keys() {
        let invalid_order = BTreeMap::from([
            ("format_shrink_levels".to_owned(), "compact".to_owned()),
            (
                "format_shrink_order".to_owned(),
                "left left right".to_owned(),
            ),
        ]);
        assert!(ModuleConfig::new(&invalid_order).is_err());

        let removed = BTreeMap::from([
            ("format_responsive".to_owned(), "true".to_owned()),
            ("format_left_1".to_owned(), "compact".to_owned()),
        ]);
        let error = ModuleConfig::new(&removed).err().unwrap().to_string();
        assert!(error.contains("format_responsive was removed"));

        let unnamed = BTreeMap::from([("format_left_compact".to_owned(), "compact".to_owned())]);
        let error = ModuleConfig::new(&unnamed).err().unwrap().to_string();
        assert!(error.contains("requires format_shrink_levels"));
    }

    #[test]
    fn responsive_levels_show_navigation_for_long_names() {
        let config = [
            ("format_shrink_levels", "compact minimal"),
            ("format_shrink_order", "right center left"),
            ("format_left", "{tabs}"),
            ("format_center", ""),
            ("format_right", ""),
            ("tab_active", "{name}"),
            ("tab_normal", "{name}"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect::<BTreeMap<_, _>>();
        let mut module = ModuleConfig::new(&config).unwrap();
        let widgets = BTreeMap::from([(
            "tabs".to_owned(),
            Arc::new(TabsWidget::new(&config)) as Arc<dyn Widget>,
        )]);
        let state = ZellijState {
            cols: 8,
            tabs: vec![
                TabInfo {
                    position: 0,
                    name: "a very long tab".to_owned(),
                    ..Default::default()
                },
                TabInfo {
                    position: 1,
                    name: "another very long tab".to_owned(),
                    active: true,
                    ..Default::default()
                },
                TabInfo {
                    position: 2,
                    name: "yet another very long tab".to_owned(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let output = module.select_bar_output(&state, &widgets);
        assert_eq!(output.levels, [2, 2, 2]);
        assert!(output.left.starts_with("<- "));
        assert!(output.left.ends_with(" ->"));
    }

    #[test]
    fn minimum_layout_keeps_tab_position_and_prioritizes_notification() {
        let config = [
            ("format_shrink_levels", "compact minimal locator tiny"),
            ("format_shrink_order", "right center left"),
            ("format_left", "{tabs}"),
            ("format_right", "{notifications} clock"),
            ("format_right_compact", "{notifications} host"),
            ("format_right_minimal", "{notifications}"),
            ("notification_format_unread", "{message}"),
            ("notification_show_interval", "60"),
            ("tab_normal", "{name} "),
            ("tab_active", "{name}"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect::<BTreeMap<_, _>>();
        let mut module = ModuleConfig::new(&config).unwrap();
        let widgets = BTreeMap::<String, Arc<dyn Widget>>::from([
            (
                "tabs".to_owned(),
                Arc::new(TabsWidget::new(&config)) as Arc<dyn Widget>,
            ),
            (
                "notifications".to_owned(),
                Arc::new(NotificationWidget::new(&config)) as Arc<dyn Widget>,
            ),
        ]);
        let state = ZellijState {
            cols: 12,
            tabs: vec![
                TabInfo {
                    position: 0,
                    name: "one".to_owned(),
                    ..Default::default()
                },
                TabInfo {
                    position: 1,
                    name: "two".to_owned(),
                    active: true,
                    ..Default::default()
                },
                TabInfo {
                    position: 2,
                    name: "three".to_owned(),
                    ..Default::default()
                },
            ],
            incoming_notification: Some(notification::Message {
                body: "deployment completed".to_owned(),
                received_at: Local::now(),
            }),
            ..Default::default()
        };

        let output = module.select_bar_output(&state, &widgets);
        assert!(output.clicks_disabled);
        assert_eq!(output.left, "2 ");
        assert!(output.center.is_empty());
        assert!(output.right.starts_with("deploy"));
        assert!(!output.right.contains("clock"));
        assert!(!output.right.contains("host"));
        assert!(
            console::measure_text_width(&output.left) + console::measure_text_width(&output.right)
                <= state.cols
        );
    }
}
