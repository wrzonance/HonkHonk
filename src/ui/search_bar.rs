use iced::widget::{button, container, row, text, text_input};
use iced::{Alignment, Border, Element, Length, Padding};

use crate::ui::theme::{self, Hh, Theme};

const INPUT_ID: &str = "honkhonk-shared-filter";
const SETTINGS_INPUT_ID: &str = "honkhonk-settings-filter";
const HOTKEYS_INPUT_ID: &str = "honkhonk-hotkeys-filter";
const SLOTS_INPUT_ID: &str = "honkhonk-slots-filter";

pub(crate) fn macros_input_id() -> iced::widget::Id {
    iced::widget::Id::new("honkhonk-macros-filter")
}

pub(crate) fn view_macros_search_bar<'a, Message: Clone + 'a>(
    query: &'a str,
    theme: Theme,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    view_search_input(
        query,
        SearchInputConfig {
            placeholder: "Search macros…",
            id: macros_input_id(),
            width: Length::Fill,
            theme,
        },
        on_input,
    )
}

#[derive(Clone)]
struct SearchInputConfig<'a> {
    placeholder: &'a str,
    id: iced::widget::Id,
    width: Length,
    theme: Theme,
}

/// Returns the stable widget identifier used for programmatic filter focus.
pub fn input_id() -> iced::widget::Id {
    iced::widget::Id::new(INPUT_ID)
}

/// Returns the stable widget identifier used for settings focus and selection.
pub(crate) fn settings_input_id() -> iced::widget::Id {
    iced::widget::Id::new(SETTINGS_INPUT_ID)
}

/// Returns the stable widget identifier used for hotkeys-section focus and selection.
pub(crate) fn hotkeys_input_id() -> iced::widget::Id {
    iced::widget::Id::new(HOTKEYS_INPUT_ID)
}

/// Returns the stable widget identifier used for slot-manager focus and selection.
pub(crate) fn slots_input_id() -> iced::widget::Id {
    iced::widget::Id::new(SLOTS_INPUT_ID)
}

#[allow(
    clippy::too_many_lines,
    reason = "stable stack layout avoids Iced text-input focus reset across query states"
)]
/// Builds the shared search input using the caller's message mapper.
pub fn view_search_bar<'a, Message>(
    query: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    view_search_input(
        query,
        SearchInputConfig {
            placeholder: "Find a sound to honk\u{2026}",
            id: input_id(),
            width: Length::Fixed(300.0),
            theme: Theme::Dark,
        },
        on_input,
    )
}

/// Builds the click-only settings search using the same stable input stack.
pub fn view_settings_search_bar<'a, Message>(
    query: &'a str,
    t: Theme,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    view_search_input(
        query,
        SearchInputConfig {
            placeholder: "Search settings\u{2026}",
            id: settings_input_id(),
            width: Length::Fill,
            theme: t,
        },
        on_input,
    )
}

/// Builds the hotkeys-section search (type-to-filter activated) using the same
/// stable input stack.
pub fn view_hotkeys_search_bar<'a, Message>(
    query: &'a str,
    t: Theme,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    view_search_input(
        query,
        SearchInputConfig {
            placeholder: "Search shortcuts\u{2026}",
            id: hotkeys_input_id(),
            width: Length::Fill,
            theme: t,
        },
        on_input,
    )
}

/// Builds the slot-manager search (type-to-filter activated) using the same
/// stable input stack.
pub fn view_slots_search_bar<'a, Message>(
    query: &'a str,
    t: Theme,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    view_search_input(
        query,
        SearchInputConfig {
            placeholder: "Search slots\u{2026}",
            id: slots_input_id(),
            width: Length::Fill,
            theme: t,
        },
        on_input,
    )
}

fn view_search_input<'a, Message>(
    query: &'a str,
    config: SearchInputConfig<'a>,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let clear_message = on_input(String::new());
    let input = search_input(query, config.clone(), on_input);
    let overlay = clear_overlay(query, clear_message, config.width, config.theme);
    iced::widget::stack![input, overlay].into()
}

fn search_input<'a, Message>(
    query: &'a str,
    config: SearchInputConfig<'a>,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let t = config.theme;
    // Reserve right space for the clear button so typed text doesn't run under it.
    let padding = if query.is_empty() {
        Padding::from(5.0)
    } else {
        Padding {
            top: 5.0,
            right: 30.0,
            bottom: 5.0,
            left: 10.0,
        }
    };

    text_input(config.placeholder, query)
        .id(config.id)
        .on_input(on_input)
        .size(theme::font::BODY)
        .width(config.width)
        .padding(padding)
        .style(move |_theme, status| {
            let border_color = match status {
                text_input::Status::Focused { .. } => t.accent(),
                _ => t.hairline(),
            };
            text_input::Style {
                background: theme::bg_color(t.panel()),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: theme::radius::PILL,
                },
                icon: t.ink_dim(),
                placeholder: t.ink_faint(),
                value: t.ink(),
                selection: t.accent(),
            }
        })
        .into()
}

fn clear_overlay<'a, Message>(
    query: &str,
    clear_message: Message,
    width: Length,
    t: Theme,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    // Always use stack so the widget tree shape is stable across all query states.
    // Changing from container → stack on first keystroke caused Iced to reset
    // text_input focus. An empty row as the second layer has no hit area or cost.
    if query.is_empty() {
        row![].into()
    } else {
        // Clear button — floats over the right edge of the input via stack.
        let clear_btn = button(text("\u{2715}").size(theme::font::BODY).color(t.ink_dim()))
            .on_press(clear_message)
            .padding(Padding {
                top: 4.0,
                right: 10.0,
                bottom: 4.0,
                left: 4.0,
            })
            .style(move |_t, status| button::Style {
                text_color: match status {
                    button::Status::Hovered | button::Status::Pressed => t.ink(),
                    _ => t.ink_dim(),
                },
                background: None,
                ..Default::default()
            });

        container(clear_btn)
            .width(width)
            .align_x(Alignment::End)
            .align_y(Alignment::Center)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum TestMessage {
        Input(String),
    }

    #[test]
    fn hotkeys_input_id_is_stable_across_calls() {
        assert_eq!(hotkeys_input_id(), hotkeys_input_id());
    }

    #[test]
    fn hotkeys_input_id_is_distinct_from_other_search_inputs() {
        let hotkeys = hotkeys_input_id();

        assert_ne!(hotkeys, input_id());
        assert_ne!(hotkeys, settings_input_id());
    }

    #[test]
    fn hotkeys_input_id_uses_its_dedicated_dom_key() {
        let debug = format!("{:?}", hotkeys_input_id());

        assert!(
            debug.contains(HOTKEYS_INPUT_ID),
            "expected hotkeys input id to carry {HOTKEYS_INPUT_ID:?}, got {debug}"
        );
    }

    #[test]
    fn slots_input_id_is_stable_across_calls() {
        assert_eq!(slots_input_id(), slots_input_id());
    }

    #[test]
    fn slots_input_id_is_distinct_from_other_search_inputs() {
        let slots = slots_input_id();

        assert_ne!(slots, input_id());
        assert_ne!(slots, settings_input_id());
        assert_ne!(slots, hotkeys_input_id());
    }

    #[test]
    fn slots_input_id_uses_its_dedicated_dom_key() {
        let debug = format!("{:?}", slots_input_id());

        assert!(
            debug.contains(SLOTS_INPUT_ID),
            "expected slots input id to carry {SLOTS_INPUT_ID:?}, got {debug}"
        );
    }

    #[test]
    fn view_hotkeys_search_bar_builds_for_empty_query() {
        let _element: Element<'_, TestMessage> =
            view_hotkeys_search_bar("", Theme::Dark, TestMessage::Input);
    }

    #[test]
    fn view_hotkeys_search_bar_builds_for_populated_query() {
        let _element: Element<'_, TestMessage> =
            view_hotkeys_search_bar("mute", Theme::Dark, TestMessage::Input);
    }

    #[test]
    fn view_slots_search_bar_builds_for_empty_query() {
        let _element: Element<'_, TestMessage> =
            view_slots_search_bar("", Theme::Dark, TestMessage::Input);
    }

    #[test]
    fn view_slots_search_bar_builds_for_populated_query() {
        let _element: Element<'_, TestMessage> =
            view_slots_search_bar("airhorn", Theme::Dark, TestMessage::Input);
    }
}
