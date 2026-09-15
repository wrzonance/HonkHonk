use super::*;
use crate::ui::list_controls::sort;
use crate::ui::{effects_panel_view, search_bar};
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length};

fn msg(message: EditorMessage) -> Message {
    Message::MacroEditor(message)
}

impl HonkHonk {
    pub(in crate::app) fn view_macros(&self) -> Element<'_, Message> {
        let transport = row![
            button("Back").on_press(Message::ShowMain),
            text("Macros").size(24),
            button(if self.is_recording() {
                "Finish recording"
            } else {
                "Record"
            })
            .on_press(if self.is_recording() {
                Message::StopRecording
            } else {
                Message::StartRecording
            }),
            button("Play").on_press(msg(EditorMessage::Play)),
            button("Stop").on_press(msg(EditorMessage::Stop)),
            iced::widget::checkbox(self.macro_editor.snap)
                .label("Snap 50 ms")
                .on_toggle(|v| msg(EditorMessage::Snap(v))),
        ]
        .spacing(10);
        let editor = column![
            self.view_macro_name(),
            crate::ui::macros::view_timeline(&self.macro_editor),
            self.view_macro_palette()
        ]
        .spacing(10)
        .width(Length::Fill);
        let content = container(
            column![
                transport,
                row![self.view_macro_list(), editor]
                    .spacing(16)
                    .height(Length::Fill)
            ]
            .spacing(12),
        )
        .padding(16);
        if let Some(menu) = self
            .view_macro_step_menu()
            .or_else(|| self.view_macro_sort_menu())
        {
            iced::widget::stack![content, menu].into()
        } else {
            content.into()
        }
    }

    fn view_macro_name(&self) -> Element<'_, Message> {
        let Some(value) = self
            .macro_editor
            .active
            .as_deref()
            .and_then(|id| self.macros.get(id))
        else {
            return text("Create or select a macro to begin").into();
        };
        if self.macro_editor.text_entry_active {
            row![
                text_input("Macro name", &value.name)
                    .on_input(|name| msg(EditorMessage::Rename(name)))
                    .on_submit(msg(EditorMessage::EndRename)),
                button("Done").on_press(msg(EditorMessage::EndRename))
            ]
            .spacing(8)
            .into()
        } else {
            row![
                text(&value.name).size(20),
                button("Rename").on_press(msg(EditorMessage::BeginRename)),
                button("Delete").on_press(msg(EditorMessage::Delete))
            ]
            .spacing(8)
            .into()
        }
    }

    fn view_macro_list(&self) -> Element<'_, Message> {
        let state = &self.macro_editor;
        let filter =
            search_bar::view_macros_search_bar(state.filter.query(), self.config.theme, |query| {
                msg(EditorMessage::Filter(query))
            });
        let chip = sort::view_sort_chip(
            state.sort,
            msg(EditorMessage::ToggleSort),
            msg(EditorMessage::ToggleDirection),
            self.config.theme,
        );
        let list = column![
            button("New macro").on_press(msg(EditorMessage::New)),
            filter,
            chip
        ]
        .spacing(8);
        let rows = self
            .macro_rows()
            .into_iter()
            .fold(column![].spacing(4), |rows, row| {
                let selected = state.active.as_deref() == Some(row.value.id.as_str());
                rows.push(
                    button(text(format!(
                        "{}{}",
                        if selected { "▸ " } else { "" },
                        row.value.name
                    )))
                    .width(Length::Fill)
                    .on_press(msg(EditorMessage::Select(row.value.id.clone()))),
                )
            });
        list.push(scrollable(rows).height(Length::Fill))
            .width(220)
            .into()
    }

    fn view_macro_palette(&self) -> Element<'_, Message> {
        let items = self.sounds.iter().fold(
            column![text("Sounds — drag to timeline; ▶ auditions / records")].spacing(5),
            |list, sound| {
                let label = container(text(&sound.name)).padding(8).width(Length::Fill);
                let drag = iced::widget::mouse_area(label)
                    .on_press(msg(EditorMessage::PaletteDrag(sound.path.clone())));
                list.push(
                    row![
                        drag,
                        button("▶").on_press(Message::PlaySound(sound.id.clone()))
                    ]
                    .spacing(5),
                )
            },
        );
        scrollable(items).height(140).into()
    }

    fn view_macro_step_menu(&self) -> Option<Element<'_, Message>> {
        let index = self.macro_editor.menu?;
        let step = self
            .macro_editor
            .active
            .as_deref()
            .and_then(|id| self.macros.get(id))?
            .steps
            .get(index)?;
        let effects =
            effects_panel_view::view_effects_panel(&self.macro_editor.effects, self.config.theme)
                .map(|message| msg(EditorMessage::Effects(Box::new(message))));
        let controls = column![
            row![
                text("Step"),
                button("Close").on_press(msg(EditorMessage::CloseMenu)),
                button("Duplicate").on_press(msg(EditorMessage::Edit(Edit::Duplicate(index)))),
                button("Remove").on_press(msg(EditorMessage::Edit(Edit::Remove(index))))
            ]
            .spacing(8),
            text(format!("Gain: {:.0}%", step.gain * 100.0)),
            iced::widget::slider(0.0..=2.0, step.gain, move |gain| msg(EditorMessage::Edit(
                Edit::Gain(index, gain)
            )))
            .step(0.01_f32),
            effects,
        ]
        .spacing(10);
        Some(step_overlay(controls.into()))
    }

    fn view_macro_sort_menu(&self) -> Option<Element<'_, Message>> {
        if !self.macro_editor.sort_open {
            return None;
        }
        Some(sort::view_sort_menu_overlay(
            sort::SortMenu {
                state: self.macro_editor.sort,
                options: &MacroSortKey::ALL,
                theme: self.config.theme,
                anchor: self.sort_menu_anchor?,
                window_size: self.window_size,
            },
            |key| msg(EditorMessage::Sort(key)),
            msg(EditorMessage::ToggleSort),
        ))
    }
}

fn step_overlay(controls: Element<'_, Message>) -> Element<'_, Message> {
    let dismiss = iced::widget::mouse_area(
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(msg(EditorMessage::CloseMenu))
    .on_right_press(msg(EditorMessage::CloseMenu));
    let panel = container(scrollable(controls))
        .padding(16)
        .width(460)
        .height(Length::Fill)
        .style(container::bordered_box);
    iced::widget::stack![
        dismiss,
        container(panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::Alignment::End)
    ]
    .into()
}
