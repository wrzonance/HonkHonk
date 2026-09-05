use super::{HonkHonk, ImportMessage as M, Message};
use crate::state::import::ImportRow;
use crate::ui::theme::{Hh, Theme};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Element, Length};

fn msg(message: M) -> Message {
    Message::Import(message)
}

impl HonkHonk {
    pub(in crate::app) fn view_import(&self) -> Element<'_, Message> {
        let state = &self.import;
        let theme = self.config.theme;
        let selected = state
            .report
            .rows
            .iter()
            .filter(|r| r.selected && r.error.is_none())
            .count();
        let header = column![
            text("IMPORT REVIEW").size(13),
            text("Bring your sounds together").size(28),
            text(&state.status)
        ]
        .spacing(8);
        let mut content = column![header].spacing(16);
        if state.busy {
            content = content.push(text(
                "Writing imported copies. This screen will update when they are ready.",
            ));
        } else {
            content = content.push(self.import_path_controls());
            if !state.scanning {
                content = content.push(self.import_batch_controls());
                content = content.push(self.import_rows());
            }
            let confirm = button(text(format!("Import {selected} sounds")))
                .on_press_maybe((selected > 0 && !state.scanning).then(|| msg(M::Confirm)));
            content = content.push(
                row![
                    button("Close / Cancel").on_press(msg(M::Cancel)),
                    button("Stop preview").on_press(Message::StopAll),
                    confirm
                ]
                .spacing(12),
            );
        }
        for error in &state.report.errors {
            content = content.push(text(error.to_string()).size(13));
        }
        container(content)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(theme.bg().into()),
                text_color: Some(theme.ink()),
                ..Default::default()
            })
            .into()
    }

    fn import_rows(&self) -> Element<'_, Message> {
        let query = self.import.filter.to_lowercase();
        let rows = self
            .import
            .report
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                row.name.to_lowercase().contains(&query)
                    || row.category.to_lowercase().contains(&query)
            })
            .map(|(index, row)| review_row(index, row, self.config.theme));
        scrollable(column(rows).spacing(8))
            .height(Length::Fill)
            .into()
    }

    fn import_path_controls(&self) -> Element<'_, Message> {
        let path = text_input(
            "Folder path — or drop folders into this window",
            &self.import.path,
        )
        .on_input(|path| msg(M::Path(path)))
        .on_submit(msg(M::Scan));
        row![
            path,
            button("Scan folder").on_press(msg(M::Scan)),
            button("Browse…").on_press(msg(M::Pick))
        ]
        .spacing(8)
        .into()
    }

    fn import_batch_controls(&self) -> Element<'_, Message> {
        let rows = &self.import.report.rows;
        let selected: Vec<_> = rows
            .iter()
            .filter(|r| r.selected && r.error.is_none())
            .collect();
        let all = !rows.is_empty() && rows.iter().all(|r| r.selected || r.error.is_some());
        let normalize = !selected.is_empty() && selected.iter().all(|r| r.normalize);
        let trim = !selected.is_empty() && selected.iter().all(|r| r.trim);
        let category = text_input("Batch category", &self.import.batch_category)
            .on_input(|v| msg(M::BatchCategory(v)));
        let filter =
            text_input("Filter rows…", &self.import.filter).on_input(|v| msg(M::Filter(v)));
        let colors = (0..8).fold(row![text("Color")].spacing(4), |r, color| {
            r.push(
                button(
                    text("●").color(
                        crate::ui::sound_tile::tone_from_seed(color)
                            .sticker(self.config.theme.is_dark()),
                    ),
                )
                .on_press(msg(M::BatchColor(color as u8))),
            )
        });
        column![
            row![
                checkbox(all)
                    .label(format!("{} selected", selected.len()))
                    .on_toggle(|v| msg(M::SelectAll(v))),
                filter
            ]
            .spacing(12),
            row![
                category,
                button("Set category").on_press(msg(M::ApplyCategory)),
                colors
            ]
            .spacing(8),
            row![
                checkbox(normalize)
                    .label("Normalize peak to 90%")
                    .on_toggle(|v| msg(M::Normalize(v))),
                checkbox(trim)
                    .label("Auto-trim silence")
                    .on_toggle(|v| msg(M::Trim(v)))
            ]
            .spacing(16),
        ]
        .spacing(8)
        .into()
    }
}

fn review_row(index: usize, row: &ImportRow, theme: Theme) -> Element<'_, Message> {
    let select = checkbox(row.selected).on_toggle(move |value| msg(M::Select(index, value)));
    let name =
        text_input("Sound name", &row.name).on_input(move |value| msg(M::Name(index, value)));
    let category =
        text_input("Category", &row.category).on_input(move |value| msg(M::Category(index, value)));
    let color = button(text("●").color(
        crate::ui::sound_tile::tone_from_seed(u64::from(row.color)).sticker(theme.is_dark()),
    ))
    .on_press(msg(M::Color(index, (row.color + 1) % 8)));
    let preview = button("▶").on_press_maybe(row.error.is_none().then(|| msg(M::Preview(index))));
    let metadata = text(format!(
        "{} · {:.1} KiB",
        crate::ui::fmt_duration(Some(row.analysis.duration_ms)),
        row.analysis.bytes as f64 / 1024.0
    ))
    .size(12);
    let status = if row.error.is_some() {
        "FAILED"
    } else if !row.selected {
        "SKIP"
    } else {
        "READY"
    };
    let controls = row![
        select,
        name.width(Length::FillPortion(3)),
        category.width(Length::FillPortion(2)),
        color,
        preview,
        text(status).size(12)
    ]
    .spacing(8);
    let content = column![
        controls,
        row![text(row.source.to_string_lossy()).size(12), metadata].spacing(12),
        text(warnings(row)).size(12)
    ]
    .spacing(4);
    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(theme.panel().into()),
            border: crate::ui::theme::tile_border(theme.hairline(), 1.0),
            ..Default::default()
        })
        .into()
}

fn warnings(row: &ImportRow) -> String {
    if let Some(error) = &row.error {
        return error.to_string();
    }
    let mut warnings = Vec::new();
    if row.analysis.peak >= 0.98 {
        warnings.push("Hot file: near full-scale peak".into());
    }
    if row.analysis.unnamed {
        warnings.push("Generic filename: choose a name".into());
    }
    if row.analysis.leading_ms >= 100 {
        warnings.push(format!("Leading silence: {} ms", row.analysis.leading_ms));
    }
    if row.normalize {
        warnings.push("Normalize imported copy".into());
    }
    if row.trim {
        warnings.push("Trim imported copy".into());
    }
    warnings.join(" · ")
}
