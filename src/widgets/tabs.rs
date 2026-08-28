use std::{cmp, collections::BTreeMap};

use zellij_tile::{
    prelude::{InputMode, ModeInfo, PaneInfo, PaneManifest, TabInfo},
    shim::switch_tab_to,
};

use crate::{config::ZellijState, render::FormattedPart};

use super::widget::Widget;

pub struct TabsWidget {
    active_tab_format: Vec<FormattedPart>,
    active_tab_fullscreen_format: Vec<FormattedPart>,
    active_tab_sync_format: Vec<FormattedPart>,
    normal_tab_format: Vec<FormattedPart>,
    normal_tab_fullscreen_format: Vec<FormattedPart>,
    normal_tab_sync_format: Vec<FormattedPart>,
    normal_tab_bell_format: Option<Vec<FormattedPart>>,
    normal_tab_flashing_bell_format: Option<Vec<FormattedPart>>,
    rename_tab_format: Vec<FormattedPart>,
    separator: Option<FormattedPart>,
    fullscreen_indicator: Option<String>,
    floating_indicator: Option<String>,
    sync_indicator: Option<String>,
    bell_indicator: Option<String>,
    flashing_bell_indicator: Option<String>,
    tab_display_count: Option<usize>,
    tab_truncate_start_format: Vec<FormattedPart>,
    tab_truncate_end_format: Vec<FormattedPart>,
    tab_zero_based_index: bool,
    active_index_format: Vec<FormattedPart>,
    active_index_only_format: Vec<FormattedPart>,
}

impl TabsWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let mut normal_tab_format: Vec<FormattedPart> = Vec::new();
        if let Some(form) = config.get("tab_normal") {
            normal_tab_format = FormattedPart::multiple_from_format_string(form, config);
        }

        let normal_tab_fullscreen_format = match config.get("tab_normal_fullscreen") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => normal_tab_format.clone(),
        };

        let normal_tab_sync_format = match config.get("tab_normal_sync") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => normal_tab_format.clone(),
        };

        let normal_tab_bell_format = config
            .get("tab_normal_bell")
            .map(|form| FormattedPart::multiple_from_format_string(form, config));

        let normal_tab_flashing_bell_format = config
            .get("tab_normal_flashing_bell")
            .map(|form| FormattedPart::multiple_from_format_string(form, config));

        let mut active_tab_format = normal_tab_format.clone();
        if let Some(form) = config.get("tab_active") {
            active_tab_format = FormattedPart::multiple_from_format_string(form, config);
        }

        let active_tab_fullscreen_format = match config.get("tab_active_fullscreen") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let active_tab_sync_format = match config.get("tab_active_sync") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let rename_tab_format = match config.get("tab_rename") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let tab_display_count = match config.get("tab_display_count") {
            Some(count) => count.parse::<usize>().ok(),
            None => None,
        };

        let tab_truncate_start_format = config
            .get("tab_truncate_start_format")
            .map(|form| FormattedPart::multiple_from_format_string(form, config))
            .unwrap_or_default();

        let tab_truncate_end_format = config
            .get("tab_truncate_end_format")
            .map(|form| FormattedPart::multiple_from_format_string(form, config))
            .unwrap_or_default();

        let tab_zero_based_index = match config.get("tab_zero_based_index") {
            Some(e) => matches!(e.as_str(), "true"),
            None => false,
        };

        let active_index_format = FormattedPart::multiple_from_format_string(
            config
                .get("tab_locator_format")
                .map(String::as_str)
                .unwrap_or("{left_arrow}{index}{right_arrow}"),
            config,
        );
        let active_index_only_format = FormattedPart::multiple_from_format_string(
            config
                .get("tab_locator_compact_format")
                .map(String::as_str)
                .unwrap_or("{index}"),
            config,
        );

        let separator = config
            .get("tab_separator")
            .map(|s| FormattedPart::from_format_string(s, config));

        let bell_indicator = config.get("tab_bell_indicator").cloned();
        let flashing_bell_indicator = config
            .get("tab_flashing_bell_indicator")
            .cloned()
            .or_else(|| bell_indicator.clone());

        Self {
            normal_tab_format,
            normal_tab_fullscreen_format,
            normal_tab_sync_format,
            normal_tab_bell_format,
            normal_tab_flashing_bell_format,
            active_tab_format,
            active_tab_fullscreen_format,
            active_tab_sync_format,
            rename_tab_format,
            separator,
            floating_indicator: config.get("tab_floating_indicator").cloned(),
            sync_indicator: config.get("tab_sync_indicator").cloned(),
            fullscreen_indicator: config.get("tab_fullscreen_indicator").cloned(),
            bell_indicator,
            flashing_bell_indicator,
            tab_display_count,
            tab_truncate_start_format,
            tab_truncate_end_format,
            tab_zero_based_index,
            active_index_format,
            active_index_only_format,
        }
    }
}

impl Widget for TabsWidget {
    fn process(&self, _name: &str, state: &ZellijState) -> String {
        self.render_at_level(state, 0)
    }

    fn process_at_level(&self, _name: &str, state: &ZellijState, level: usize) -> String {
        self.render_at_level(state, level)
    }

    fn process_click(&self, _name: &str, state: &ZellijState, pos: usize) {
        self.click_at_level(state, pos, 0);
    }

    fn process_click_at_level(&self, _name: &str, state: &ZellijState, pos: usize, level: usize) {
        self.click_at_level(state, pos, level);
    }
}

impl TabsWidget {
    fn render_at_level(&self, state: &ZellijState, level: usize) -> String {
        if level == 2 {
            return self.render_active_tab_with_navigation(state, true);
        }
        if level >= 3 {
            return self.render_active_index(state, level >= 4);
        }

        if level == 1 {
            let count = self.tab_level_one_count(state);
            return if count == 1 {
                self.render_active_tab_with_navigation(state, false)
            } else {
                self.render_tab_window(state, count)
            };
        }
        if level == 0 {
            return self
                .render_tab_window(state, self.tab_display_count.unwrap_or(state.tabs.len()));
        }
        String::new()
    }

    fn render_tab_window(&self, state: &ZellijState, count: usize) -> String {
        let (truncated_start, truncated_end, tabs) = get_tab_window(&state.tabs, Some(count));
        let mut output = String::new();
        if truncated_start > 0 {
            for part in &self.tab_truncate_start_format {
                let content = part
                    .content
                    .replace("{count}", &truncated_start.to_string());
                output.push_str(&part.format_string(&content));
            }
        }
        for (index, tab) in tabs.iter().enumerate() {
            output.push_str(&self.render_tab(tab, &state.panes, &state.mode));
            if index + 1 < tabs.len()
                && let Some(separator) = &self.separator
            {
                output.push_str(&separator.format_string(&separator.content));
            }
        }
        if truncated_end > 0 {
            for part in &self.tab_truncate_end_format {
                let content = part.content.replace("{count}", &truncated_end.to_string());
                output.push_str(&part.format_string(&content));
            }
        }
        output
    }

    fn click_at_level(&self, state: &ZellijState, pos: usize, level: usize) {
        if level == 2 || (level == 1 && self.tab_level_one_count(state) == 1) {
            self.click_active_tab_with_navigation(state, pos, level == 2);
            return;
        }
        if level >= 3 {
            if let Some(tab) = self.locator_navigation_target(state, pos, level >= 4) {
                switch_tab_to(tab);
            }
            return;
        }

        let (truncated_start, truncated_end, tabs) = self.tab_window_at_level(state, level);
        let Some(active_pos) = state
            .tabs
            .iter()
            .find(|tab| tab.active)
            .map(|tab| tab.position + 1)
        else {
            return;
        };
        let mut offset = 0;

        if truncated_start > 0 {
            for part in &self.tab_truncate_start_format {
                let content = part
                    .content
                    .replace("{count}", &truncated_start.to_string());
                offset += console::measure_text_width(&part.format_string(&content));
                if pos <= offset {
                    switch_tab_to(active_pos.saturating_sub(1) as u32);
                }
            }
        }

        for (index, tab) in tabs.iter().enumerate() {
            let mut content = self.render_tab(tab, &state.panes, &state.mode);
            if index + 1 < tabs.len()
                && let Some(separator) = &self.separator
            {
                content.push_str(&separator.format_string(&separator.content));
            }
            let width = console::measure_text_width(&content);
            if pos > offset && pos < offset + width {
                switch_tab_to(tab.position as u32 + 1);
                return;
            }
            offset += width;
        }

        if truncated_end > 0 {
            for part in &self.tab_truncate_end_format {
                let content = part.content.replace("{count}", &truncated_end.to_string());
                offset += console::measure_text_width(&part.format_string(&content));
                if pos <= offset {
                    switch_tab_to(cmp::min(active_pos + 1, state.tabs.len()) as u32);
                }
            }
        }
    }

    fn click_active_tab_with_navigation(
        &self,
        state: &ZellijState,
        pos: usize,
        truncate_name: bool,
    ) {
        let rendered = self.render_active_tab_with_navigation(state, truncate_name);
        self.click_navigation(state, pos, console::measure_text_width(&rendered));
    }

    fn click_navigation(&self, state: &ZellijState, pos: usize, total_width: usize) {
        if let Some(tab) = navigation_target(state, pos, total_width) {
            switch_tab_to(tab);
        }
    }

    fn tab_level_one_count(&self, state: &ZellijState) -> usize {
        let max_count = self
            .tab_display_count
            .map(|count| count.saturating_sub(1))
            .unwrap_or(state.tabs.len())
            .clamp(1, 3);
        (1..=max_count)
            .rev()
            .find(|count| {
                state.cols == 0
                    || console::measure_text_width(&self.render_tab_window(state, *count))
                        <= state.cols
            })
            .unwrap_or(1)
    }

    fn tab_window_at_level(
        &self,
        state: &ZellijState,
        level: usize,
    ) -> (usize, usize, Vec<TabInfo>) {
        match level {
            0 => get_tab_window(&state.tabs, self.tab_display_count),
            1 => get_tab_window(&state.tabs, Some(self.tab_level_one_count(state))),
            2 => get_tab_window(&state.tabs, Some(2)),
            _ => active_tab_window(&state.tabs),
        }
    }

    fn render_active_tab_with_navigation(
        &self,
        state: &ZellijState,
        truncate_name: bool,
    ) -> String {
        let Some((active_index, active_tab)) =
            state.tabs.iter().enumerate().find(|(_, tab)| tab.active)
        else {
            return String::new();
        };
        let left_arrow = if active_index > 0 { "<- " } else { "" };
        let right_arrow = if active_index + 1 < state.tabs.len() {
            " ->"
        } else {
            ""
        };
        let mut active_tab = active_tab.clone();
        if truncate_name && state.cols > 0 {
            let arrow_width =
                console::measure_text_width(left_arrow) + console::measure_text_width(right_arrow);
            active_tab.name = console::truncate_str(
                &active_tab.name,
                state.cols.saturating_sub(arrow_width),
                "…",
            )
            .into_owned();
        }
        format!(
            "{left_arrow}{}{right_arrow}",
            self.render_tab(&active_tab, &state.panes, &state.mode)
        )
    }

    fn render_active_index(&self, state: &ZellijState, index_only: bool) -> String {
        let Some((active_index, active_tab)) =
            state.tabs.iter().enumerate().find(|(_, tab)| tab.active)
        else {
            return String::new();
        };
        let index = active_tab.position + usize::from(!self.tab_zero_based_index);
        let left_arrow = if active_index > 0 { "<- " } else { "" };
        let right_arrow = if active_index + 1 < state.tabs.len() {
            " ->"
        } else {
            ""
        };
        let format = if index_only {
            &self.active_index_only_format
        } else {
            &self.active_index_format
        };

        format.iter().fold(String::new(), |mut output, part| {
            let content = part
                .content
                .replace("{index}", &index.to_string())
                .replace("{left_arrow}", left_arrow)
                .replace("{right_arrow}", right_arrow);
            output.push_str(&part.format_string(&content));
            output
        })
    }

    fn locator_navigation_target(
        &self,
        state: &ZellijState,
        pos: usize,
        index_only: bool,
    ) -> Option<u32> {
        let (active_index, active_tab) =
            state.tabs.iter().enumerate().find(|(_, tab)| tab.active)?;
        let index = active_tab.position + usize::from(!self.tab_zero_based_index);
        let left_target =
            (active_index > 0).then(|| state.tabs[active_index - 1].position as u32 + 1);
        let right_target = (active_index + 1 < state.tabs.len())
            .then(|| state.tabs[active_index + 1].position as u32 + 1);
        let format = if index_only {
            &self.active_index_only_format
        } else {
            &self.active_index_format
        };
        locator_target_at(format, &index.to_string(), left_target, right_target, pos)
    }

    fn select_format(&self, info: &TabInfo, mode: &ModeInfo) -> &Vec<FormattedPart> {
        if info.active && mode.mode == InputMode::RenameTab {
            return &self.rename_tab_format;
        }

        if !info.active && info.is_flashing_bell {
            let fmt = self
                .normal_tab_flashing_bell_format
                .as_ref()
                .or(self.normal_tab_bell_format.as_ref());
            if let Some(fmt) = fmt {
                return fmt;
            }
        }

        if !info.active
            && info.has_bell_notification
            && let Some(fmt) = self.normal_tab_bell_format.as_ref()
        {
            return fmt;
        }

        if info.active && info.is_fullscreen_active {
            return &self.active_tab_fullscreen_format;
        }

        if info.active && info.is_sync_panes_active {
            return &self.active_tab_sync_format;
        }

        if info.active {
            return &self.active_tab_format;
        }

        if info.is_fullscreen_active {
            return &self.normal_tab_fullscreen_format;
        }

        if info.is_sync_panes_active {
            return &self.normal_tab_sync_format;
        }

        &self.normal_tab_format
    }

    fn render_tab(&self, tab: &TabInfo, panes: &PaneManifest, mode: &ModeInfo) -> String {
        let formatters = self.select_format(tab, mode);
        let mut output = "".to_owned();

        for f in formatters.iter() {
            let mut content = f.content.clone();

            let tab_name = match mode.mode {
                InputMode::RenameTab => match tab.name.is_empty() {
                    true => "Enter name...",
                    false => tab.name.as_str(),
                },
                _name => tab.name.as_str(),
            };

            if content.contains("{name}") {
                content = content.replace("{name}", tab_name);
            }

            if content.contains("{index}") {
                let index = match self.tab_zero_based_index {
                    true => tab.position,
                    false => tab.position + 1,
                };
                content = content.replace("{index}", index.to_string().as_str());
            }

            if content.contains("{floating_total_count}") {
                let panes_for_tab: Vec<PaneInfo> =
                    panes.panes.get(&tab.position).cloned().unwrap_or_default();

                content = content.replace(
                    "{floating_total_count}",
                    &format!("{}", panes_for_tab.iter().filter(|p| p.is_floating).count()),
                );
            }

            if content.contains("{focused_pane_title}") {
                let panes_for_tab: Vec<PaneInfo> =
                    panes.panes.get(&tab.position).cloned().unwrap_or_default();

                let focused_pane_title = panes_for_tab
                    .iter()
                    .find(|pane| pane.is_focused)
                    .map(|pane| pane.title.clone())
                    .unwrap_or_default();

                content = content.replace("{focused_pane_title}", &focused_pane_title);
            }

            content = self.replace_indicators(content, tab, panes);

            output = format!("{}{}", output, f.format_string(&content));
        }

        output.to_owned()
    }

    fn replace_indicators(&self, content: String, tab: &TabInfo, panes: &PaneManifest) -> String {
        let mut content = content;
        if content.contains("{fullscreen_indicator}")
            && let Some(fullscreen_indicator) = self.fullscreen_indicator.clone()
        {
            content = content.replace(
                "{fullscreen_indicator}",
                if tab.is_fullscreen_active {
                    fullscreen_indicator.as_ref()
                } else {
                    ""
                },
            );
        }

        if content.contains("{sync_indicator}")
            && let Some(sync_indicator) = self.sync_indicator.clone()
        {
            content = content.replace(
                "{sync_indicator}",
                if tab.is_sync_panes_active {
                    sync_indicator.as_ref()
                } else {
                    ""
                },
            );
        }

        if content.contains("{floating_indicator}")
            && let Some(floating_indicator) = self.floating_indicator.clone()
        {
            let panes_for_tab: Vec<PaneInfo> =
                panes.panes.get(&tab.position).cloned().unwrap_or_default();

            let is_floating = panes_for_tab.iter().any(|p| p.is_floating);

            content = content.replace(
                "{floating_indicator}",
                if is_floating {
                    floating_indicator.as_ref()
                } else {
                    ""
                },
            );
        }

        if content.contains("{bell_indicator}")
            && (self.bell_indicator.is_some() || self.flashing_bell_indicator.is_some())
        {
            let indicator = if tab.is_flashing_bell {
                self.flashing_bell_indicator.as_deref().unwrap_or("")
            } else if tab.has_bell_notification {
                self.bell_indicator.as_deref().unwrap_or("")
            } else {
                ""
            };

            content = content.replace("{bell_indicator}", indicator);
        }

        content
    }
}

fn navigation_target(state: &ZellijState, pos: usize, total_width: usize) -> Option<u32> {
    let active_index = state.tabs.iter().position(|tab| tab.active)?;
    if active_index > 0 && pos < 3 {
        return Some(state.tabs[active_index - 1].position as u32 + 1);
    }
    if active_index + 1 < state.tabs.len() && pos >= total_width.saturating_sub(3) {
        return Some(state.tabs[active_index + 1].position as u32 + 1);
    }
    None
}
fn locator_target_at(
    format: &[FormattedPart],
    index: &str,
    left_target: Option<u32>,
    right_target: Option<u32>,
    pos: usize,
) -> Option<u32> {
    let mut offset = 0;
    for part in format {
        let mut remaining = part.content.as_str();
        loop {
            let next = [
                ("{index}", index, None),
                ("{left_arrow}", "<- ", left_target),
                ("{right_arrow}", " ->", right_target),
            ]
            .into_iter()
            .filter_map(|(token, replacement, target)| {
                remaining
                    .find(token)
                    .map(|start| (start, token, replacement, target))
            })
            .min_by_key(|(start, _, _, _)| *start);
            let Some((start, token, replacement, target)) = next else {
                offset += console::measure_text_width(remaining);
                break;
            };
            offset += console::measure_text_width(&remaining[..start]);
            let replacement = if target.is_some() || token == "{index}" {
                replacement
            } else {
                ""
            };
            let width = console::measure_text_width(replacement);
            if target.is_some() && pos >= offset && pos < offset + width {
                return target;
            }
            offset += width;
            remaining = &remaining[start + token.len()..];
        }
    }
    None
}

pub fn get_tab_window(
    tabs: &Vec<TabInfo>,
    max_count: Option<usize>,
) -> (usize, usize, Vec<TabInfo>) {
    let max_count = match max_count {
        Some(count) => count,
        None => return (0, 0, tabs.to_vec()),
    };

    if tabs.len() <= max_count {
        return (0, 0, tabs.to_vec());
    }

    let active_index = tabs.iter().position(|t| t.active).expect("no active tab");

    // active tab is in the last #max_count tabs, so return the last #max_count
    if active_index > tabs.len().saturating_sub(max_count) {
        return (
            tabs.len().saturating_sub(max_count),
            0,
            tabs.iter()
                .cloned()
                .rev()
                .take(max_count)
                .rev()
                .collect::<Vec<TabInfo>>(),
        );
    }

    // tabs must be truncated
    let first_index = active_index.saturating_sub(1);
    let last_index = cmp::min(first_index + max_count, tabs.len());

    (
        first_index,
        tabs.len().saturating_sub(last_index),
        tabs.as_slice()[first_index..last_index].to_vec(),
    )
}

fn active_tab_window(tabs: &[TabInfo]) -> (usize, usize, Vec<TabInfo>) {
    let Some(active_index) = tabs.iter().position(|tab| tab.active) else {
        return (0, 0, Vec::new());
    };
    (0, 0, vec![tabs[active_index].clone()])
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use zellij_tile::prelude::TabInfo;

    use super::{TabsWidget, get_tab_window, navigation_target};
    use crate::{config::ZellijState, widgets::widget::Widget};
    use rstest::rstest;

    #[rstest]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (1, 1, vec![
                TabInfo {
                    active: false,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: true,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 2, vec![
                TabInfo {
                    active: true,
                    name: "1".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 2, vec![
                TabInfo {
                    active: false,
                    name: "1".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (2, 0, vec![
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "5".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (2, 0, vec![
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "5".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        None,
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    pub fn test_get_tab_window(
        #[case] tabs: Vec<TabInfo>,
        #[case] max_count: Option<usize>,
        #[case] expected: (usize, usize, Vec<TabInfo>),
    ) {
        let res = get_tab_window(&tabs, max_count);

        assert_eq!(res, expected);
    }

    #[test]
    fn responsive_level_reduces_tab_window_before_active_name() {
        let config = BTreeMap::from([
            ("tab_display_count".to_owned(), "3".to_owned()),
            ("tab_active".to_owned(), "{name}".to_owned()),
            ("tab_normal".to_owned(), "{name}".to_owned()),
        ]);
        let widget = TabsWidget::new(&config);
        let state = ZellijState {
            tabs: vec![
                TabInfo {
                    position: 0,
                    ..Default::default()
                },
                TabInfo {
                    position: 1,
                    active: true,
                    name: "two".to_owned(),
                    ..Default::default()
                },
                TabInfo {
                    position: 2,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let (_, _, tabs) = widget.tab_window_at_level(&state, 1);
        assert_eq!(tabs.len(), 2);
        assert!(tabs.iter().any(|tab| tab.active));
    }

    #[test]
    fn narrow_levels_keep_the_active_tab_position() {
        let config = BTreeMap::from([
            (
                "tab_truncate_start_format".to_owned(),
                "<{count}>".to_owned(),
            ),
            ("tab_truncate_end_format".to_owned(), "<{count}>".to_owned()),
            ("tab_active".to_owned(), "{name}".to_owned()),
        ]);
        let widget = TabsWidget::new(&config);
        let state = ZellijState {
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
            ..Default::default()
        };

        assert_eq!(widget.process_at_level("tabs", &state, 2), "<- two ->");
        assert_eq!(widget.process_at_level("tabs", &state, 3), "<- 2 ->");
        assert_eq!(widget.process_at_level("tabs", &state, 4), "2");
    }

    #[test]
    fn navigation_clicks_follow_rendered_arrow_geometry() {
        let state = ZellijState {
            tabs: vec![
                TabInfo {
                    position: 0,
                    ..Default::default()
                },
                TabInfo {
                    position: 1,
                    active: true,
                    ..Default::default()
                },
                TabInfo {
                    position: 2,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert_eq!(navigation_target(&state, 0, 9), Some(1));
        assert_eq!(navigation_target(&state, 4, 9), None);
        assert_eq!(navigation_target(&state, 8, 9), Some(3));

        let reordered = TabsWidget::new(&BTreeMap::from([(
            "tab_locator_format".to_owned(),
            "{right_arrow}{index}{left_arrow}".to_owned(),
        )]));
        assert_eq!(
            reordered.locator_navigation_target(&state, 0, false),
            Some(3)
        );
        assert_eq!(
            reordered.locator_navigation_target(&state, 6, false),
            Some(1)
        );

        let compact = TabsWidget::new(&BTreeMap::from([(
            "tab_locator_compact_format".to_owned(),
            "{right_arrow}{index}".to_owned(),
        )]));
        assert_eq!(compact.locator_navigation_target(&state, 0, true), Some(3));

        let omitted = TabsWidget::new(&BTreeMap::from([(
            "tab_locator_format".to_owned(),
            "{index}".to_owned(),
        )]));
        assert_eq!(omitted.locator_navigation_target(&state, 0, false), None);
    }
}
