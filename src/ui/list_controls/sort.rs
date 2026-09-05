use std::cmp::Ordering;

use iced::widget::{Column, button, container, mouse_area, row, text};
use iced::{Element, Length, Padding, Point};

use crate::ui::theme::{self, Hh, Theme};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Ascending,
    Descending,
}

impl Direction {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    fn apply(self, ordering: Ordering) -> Ordering {
        match self {
            Self::Ascending => ordering,
            Self::Descending => ordering.reverse(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortState<K> {
    key: K,
    direction: Direction,
}

impl<K> SortState<K> {
    pub const fn new(key: K, direction: Direction) -> Self {
        Self { key, direction }
    }

    pub const fn key(&self) -> K
    where
        K: Copy,
    {
        self.key
    }

    pub const fn direction(&self) -> Direction {
        self.direction
    }
}

pub trait SortLabel {
    fn label(self) -> &'static str;
}

pub trait SortKey<T>: SortLabel + Sized {
    fn compare(self, left: &T, right: &T) -> Ordering;

    fn value_unknown(self, _item: &T) -> bool {
        false
    }

    /// Breaks a tie left by `compare()` (i.e. `compare()` returned
    /// `Ordering::Equal`). Unlike `compare()`, this is **never** reversed by
    /// `Direction` — `sorted()` applies it after direction has already been
    /// applied to the primary comparison, so a tie-break stays in the same
    /// order regardless of ascending/descending. Defaults to `Equal` (no
    /// tie-break) for keys where `compare()` is already total.
    fn tie_break(self, _left: &T, _right: &T) -> Ordering {
        Ordering::Equal
    }
}

impl<K: Copy> SortState<K> {
    pub fn select(&mut self, key: K) {
        self.key = key;
    }

    pub fn toggle_direction(&mut self) {
        self.direction = self.direction.toggled();
    }

    pub fn sorted<T>(&self, items: impl IntoIterator<Item = T>) -> Vec<T>
    where
        K: SortKey<T>,
    {
        let mut sorted = items.into_iter().collect::<Vec<_>>();
        sorted.sort_by(|left, right| {
            match (self.key.value_unknown(left), self.key.value_unknown(right)) {
                (false, true) => Ordering::Less,
                (true, false) => Ordering::Greater,
                _ => self
                    .direction
                    .apply(self.key.compare(left, right))
                    .then_with(|| self.key.tie_break(left, right)),
            }
        });
        sorted
    }
}

const MENU_WIDTH: f32 = 180.0;
const OPTION_HEIGHT: f32 = 36.0;

pub fn view_sort_chip<'a, K, Message>(
    state: SortState<K>,
    on_open: Message,
    on_toggle: Message,
    theme: Theme,
) -> Element<'a, Message>
where
    K: Copy + SortLabel,
    Message: Clone + 'a,
{
    let label = button(
        text(format!("Sort: {}", state.key().label()))
            .size(theme::font::BODY)
            .color(theme.ink()),
    )
    .on_press(on_open)
    .padding([theme::space::XS, theme::space::SM])
    .style(move |_iced_theme, _status| transparent_button(theme));
    let chevron = match state.direction() {
        Direction::Ascending => "\u{2191}",
        Direction::Descending => "\u{2193}",
    };
    let direction = button(text(chevron).size(theme::font::BODY).color(theme.ink()))
        .on_press(on_toggle)
        .padding([theme::space::XS, theme::space::SM])
        .style(move |_iced_theme, _status| transparent_button(theme));

    container(row![label, direction].spacing(0))
        .style(move |_iced_theme| container::Style {
            background: Some(theme::bg_color(theme.panel())),
            border: theme::tile_border(theme.hairline(), 1.0),
            ..Default::default()
        })
        .into()
}

pub struct SortMenu<'a, K> {
    pub state: SortState<K>,
    pub options: &'a [K],
    pub theme: Theme,
    pub anchor: Point,
    pub window_size: (f32, f32),
}

pub fn view_sort_menu_overlay<'a, K, Message, F>(
    menu: SortMenu<'a, K>,
    on_select: F,
    on_dismiss: Message,
) -> Element<'a, Message>
where
    K: Copy + Eq + SortLabel + 'a,
    Message: Clone + 'a,
    F: Fn(K) -> Message + Copy + 'a,
{
    view_sort_menu_with_grouping(menu, on_select, on_dismiss, None)
}

pub fn view_sort_menu_with_grouping<'a, K, Message, F>(
    menu: SortMenu<'a, K>,
    on_select: F,
    on_dismiss: Message,
    grouping: Option<(bool, Message)>,
) -> Element<'a, Message>
where
    K: Copy + Eq + SortLabel + 'a,
    Message: Clone + 'a,
    F: Fn(K) -> Message + Copy + 'a,
{
    let SortMenu {
        state,
        options,
        theme,
        anchor,
        window_size,
    } = menu;
    let mut options = view_sort_options(state, options, on_select, theme);
    if let Some((selected, message)) = grouping {
        options.push(grouping_option(selected, message, theme));
    }
    let menu_height = options.len() as f32 * OPTION_HEIGHT;
    let menu = container(Column::with_children(options).spacing(2))
        .width(MENU_WIDTH)
        .padding(theme::space::XS)
        .style(move |_iced_theme| container::Style {
            background: Some(theme::bg_color(theme.panel())),
            border: theme::tile_border(theme.hairline(), 1.0),
            ..Default::default()
        });
    let (left, top) = menu_position(anchor, window_size, menu_height);
    let dismiss = mouse_area(
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(on_dismiss.clone())
    .on_right_press(on_dismiss);

    container(iced::widget::stack![
        dismiss,
        container(menu)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding {
                top,
                left,
                right: 0.0,
                bottom: 0.0,
            }),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn grouping_option<'a, Message: Clone + 'a>(
    selected: bool,
    message: Message,
    theme: Theme,
) -> Element<'a, Message> {
    let label = if selected {
        "✓ Group by tag"
    } else {
        "  Group by tag"
    };
    button(text(label).size(theme::font::BODY).color(theme.ink()))
        .on_press(message)
        .width(Length::Fill)
        .padding([theme::space::SM, theme::space::MD])
        .style(move |_, status| option_style(theme, status, selected))
        .into()
}

fn view_sort_options<'a, K, Message, F>(
    state: SortState<K>,
    options: &'a [K],
    on_select: F,
    theme: Theme,
) -> Vec<Element<'a, Message>>
where
    K: Copy + Eq + SortLabel + 'a,
    Message: Clone + 'a,
    F: Fn(K) -> Message + Copy + 'a,
{
    options
        .iter()
        .copied()
        .map(|key| {
            let selected = key == state.key();
            let label = if selected {
                format!("\u{2713} {}", key.label())
            } else {
                format!("  {}", key.label())
            };
            button(text(label).size(theme::font::BODY).color(theme.ink()))
                .on_press(on_select(key))
                .width(Length::Fill)
                .padding([theme::space::SM, theme::space::MD])
                .style(move |_iced_theme, status| option_style(theme, status, selected))
                .into()
        })
        .collect()
}

fn transparent_button(theme: Theme) -> button::Style {
    button::Style {
        background: None,
        text_color: theme.ink(),
        ..Default::default()
    }
}

fn option_style(theme: Theme, status: button::Status, selected: bool) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => theme.accent(),
        _ if selected => theme.hairline2(),
        _ => theme.panel(),
    };
    button::Style {
        background: Some(theme::bg_color(background)),
        text_color: theme.ink(),
        ..Default::default()
    }
}

fn menu_position(anchor: Point, window_size: (f32, f32), menu_height: f32) -> (f32, f32) {
    let left = anchor.x.min((window_size.0 - MENU_WIDTH).max(0.0)).max(0.0);
    let below = anchor.y + theme::space::SM;
    let top = below.min((window_size.1 - menu_height).max(0.0)).max(0.0);
    (left, top)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct Item(Option<u8>);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct ValueKey;

    impl SortLabel for ValueKey {
        fn label(self) -> &'static str {
            "Value"
        }
    }

    impl SortKey<Item> for ValueKey {
        fn compare(self, left: &Item, right: &Item) -> Ordering {
            left.0.cmp(&right.0)
        }

        fn value_unknown(self, item: &Item) -> bool {
            item.0.is_none()
        }
    }

    fn values(items: &[Item]) -> Vec<Option<u8>> {
        items.iter().map(|item| item.0).collect()
    }

    #[test]
    fn ascending_places_unknown_values_last() {
        let items = [Item(None), Item(Some(3)), Item(Some(1)), Item(Some(2))];
        let sorted = SortState::new(ValueKey, Direction::Ascending).sorted(items);

        assert_eq!(values(&sorted), vec![Some(1), Some(2), Some(3), None]);
    }

    #[test]
    fn descending_reverses_known_values_but_keeps_unknown_last() {
        let items = [Item(None), Item(Some(1)), Item(Some(3)), Item(Some(2))];
        let sorted = SortState::new(ValueKey, Direction::Descending).sorted(items);

        assert_eq!(values(&sorted), vec![Some(3), Some(2), Some(1), None]);
    }
}
