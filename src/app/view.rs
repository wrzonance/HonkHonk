//! Root layouts and stable overlay ordering.

use super::*;

impl HonkHonk {
    fn view_category_chips(&self, t: theme::Theme) -> Element<'_, Message> {
        use std::collections::BTreeSet;

        let categories: BTreeSet<&str> = self.sounds.iter().map(|s| s.category.as_str()).collect();

        let all_chip = self.category_chip("All", self.active_category.is_none(), None, t);

        let has_favorites = self
            .sounds
            .iter()
            .any(|s| self.sound_meta.is_favorite(&s.id));
        let fav_active = self.active_category.as_deref() == Some(FAVORITES_TAB);

        let chips: Vec<Element<'_, Message>> = std::iter::once(all_chip)
            .chain(has_favorites.then(|| {
                self.category_chip(FAVORITES_TAB, fav_active, Some(FAVORITES_TAB.to_owned()), t)
            }))
            .chain(categories.into_iter().map(|cat| {
                let is_active = self.active_category.as_deref() == Some(cat);
                self.category_chip(cat, is_active, Some(cat.to_owned()), t)
            }))
            .collect();

        let chip_row = chips
            .into_iter()
            .fold(row![].spacing(theme::space::SM), |r, chip| r.push(chip));

        scrollable(chip_row)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new(),
            ))
            .into()
    }

    fn category_chip(
        &self,
        label: &str,
        active: bool,
        value: Option<String>,
        t: theme::Theme,
    ) -> Element<'_, Message> {
        let bg = if active { t.accent() } else { t.panel() };
        let text_color = if active {
            iced::Color::from_rgb(0.1, 0.07, 0.03)
        } else {
            t.ink()
        };

        button(text(label.to_owned()).size(13).color(text_color))
            .on_press(Message::SelectCategory(value))
            .padding([theme::space::XS, theme::space::MD])
            .style(move |_theme, _status| button::Style {
                background: Some(theme::bg_color(bg)),
                text_color,
                border: iced::Border {
                    color: iced::Color::TRANSPARENT,
                    width: 0.0,
                    radius: theme::radius::PILL,
                },
                ..Default::default()
            })
            .into()
    }

    pub fn theme(&self) -> Theme {
        match self.config.theme {
            theme::Theme::Light => Theme::Light,
            theme::Theme::Dark | theme::Theme::System => Theme::Dark,
        }
    }

    fn view_shortcuts_banner(&self, t: theme::Theme) -> Option<Element<'_, Message>> {
        let ShortcutsStatus::Unavailable(ref reason) = self.shortcuts_status else {
            return None;
        };
        if self.shortcuts_warning_dismissed {
            return None;
        }
        let banner = container(
            row![
                text(format!(
                    "Global shortcuts unavailable: {reason}. Check xdg-desktop-portal is running."
                ))
                .size(13)
                .color(iced::Color::from_rgb(0.6, 0.4, 0.0)),
                space::horizontal(),
                button(text("×").size(14))
                    .on_press(Message::DismissShortcutsWarning)
                    .style(move |_t, _s| button::Style {
                        background: None,
                        text_color: t.ink(),
                        ..Default::default()
                    }),
            ]
            .spacing(theme::space::MD)
            .align_y(iced::Alignment::Center),
        )
        .padding([theme::space::SM, theme::space::LG])
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(iced::Color::from_rgb(0.98, 0.92, 0.75))),
            border: theme::tile_border(iced::Color::from_rgb(0.9, 0.75, 0.3), 1.0),
            ..Default::default()
        });
        Some(banner.into())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "legacy root layout preserves Iced widget-state ordering while the app split proceeds under #142"
    )]
    fn view_main(&self) -> Element<'_, Message> {
        let t = self.config.theme;
        let header = self.view_header(t);
        let chips = self.view_category_chips(t);
        let grid_ctx = sound_grid::GridCtx {
            slots: &self.slots,
            triggers: &self.slot_triggers,
            shortcuts_active: matches!(self.shortcuts_status, ShortcutsStatus::Active),
            columns: self.config.density.columns(),
            sound_meta: &self.sound_meta,
        };
        let grid = if self.sound_tags_grouped() {
            sound_grid::view_groups(
                &self.sounds,
                self.sound_tag_groups(),
                self.playing.as_deref(),
                grid_ctx,
            )
        } else {
            sound_grid::view_grid(
                &self.sounds,
                &self.filtered_sound_indices,
                self.playing.as_deref(),
                grid_ctx,
            )
        };

        let playing_sound = self
            .playing
            .as_deref()
            .and_then(|id| self.sounds.iter().find(|s| s.id == id));
        let envelope = self
            .playing
            .as_deref()
            .and_then(|id| self.now_playing.envelope(id));
        let now_playing = now_playing::view_now_playing(
            &self.now_playing,
            playing_sound,
            self.now_playing.display_progress(),
            self.config.volume,
            envelope.as_deref(),
        );

        // The banner shares one stable column slot with the header: inserting
        // it as its own top-level slot would shift every later sibling during
        // tree diffing when it appears/dismisses, wiping the grid scrollable's
        // offset (#112).
        let mut top = iced::widget::Column::new().spacing(theme::space::MD);
        if let Some(banner) = self.view_shortcuts_banner(t) {
            top = top.push(banner);
        }
        let top = top.push(header);

        // Inset the grid from the overlay scrollbar (10px, drawn over content) so
        // the last tile column is never clipped by it.
        let grid_scroll = scrollable(container(grid).width(Length::Fill).padding(iced::Padding {
            top: 0.0,
            right: theme::space::LG,
            bottom: 0.0,
            left: 0.0,
        }))
        .width(Length::Fill)
        .height(Length::Fill);

        let items: Vec<Element<'_, Message>> =
            vec![top.into(), chips, grid_scroll.into(), now_playing];

        let content = iced::widget::Column::with_children(items)
            .spacing(theme::space::MD)
            .width(Length::Fill)
            .height(Length::Fill);

        let base = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            // Larger right padding reserves a clean gutter for the closed side-panel
            // handle (#143, ~28px) so it never overlaps the grid or its scrollbar.
            .padding(iced::Padding {
                top: theme::space::XL,
                right: theme::space::XL + theme::space::SM,
                bottom: theme::space::XL,
                left: theme::space::XL,
            })
            .style(move |_theme| container::Style {
                background: Some(theme::bg_color(t.bg())),
                ..Default::default()
            });

        // iced reconciles widget state positionally: flipping the root between
        // `container` (no overlay) and `stack![...]` (overlay open) discards
        // every descendant's state — including the grid scrollable's offset,
        // which made the list snap to the top on right-click (#112). Keep the
        // root a Stack with the base layout always at child 0; overlays only
        // append/remove child 1, so the base subtree (and its scroll position)
        // survives the diff.
        let mut layers: Vec<Element<'_, Message>> = vec![base.into()];

        // Effects side panel (#143): pull tab always visible; scrim + body slide
        // in when open. Pushed below the context-menu/editor modals so those stack
        // on top. All drawer assembly + logic lives in `ui` modules, not here.
        layers.push(effects_panel_view::effects_side_panel_layer(
            &self.effects_ui,
            self.panel_progress,
            t,
        ));
        // Decorative flourish keeps a stable layer below interactive overlays.
        layers.push(crate::ui::side_panel::view_panel_flourish(
            &self.panel_flourish,
        ));

        // Overlay context menu at window level so cursor coords map exactly.
        if let Some(sort_menu) = self.view_sound_sort_overlay(t) {
            layers.push(sort_menu);
        } else if let (Some(sound_id), Some(pos)) = (&self.context_menu, self.context_menu_pos) {
            let found = self.sounds.iter().find(|s| s.id == *sound_id);
            layers.push(sound_grid::context_menu_overlay(
                found,
                sound_grid::SlotCtx {
                    slots: &self.slots,
                    triggers: &self.slot_triggers,
                },
                t,
                pos,
                self.window_size,
            ));
        } else if let Some(ref sound_id) = self.editor_sound_id {
            // Per-sound editor overlay
            if let Some(sound) = self.sounds.iter().find(|s| s.id == *sound_id) {
                layers.push(crate::ui::sound_editor::view_editor_overlay(
                    crate::ui::sound_editor::EditorCtx {
                        sound,
                        meta: self.sound_meta.get(sound_id),
                        draft_name: &self.editor_draft_name,
                        draft_tags: &self.editor_draft_tags,
                        draft_volume: self.editor_draft_volume,
                    },
                    t,
                ));
            }
        }

        iced::widget::Stack::with_children(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let base = if self.import.open {
            self.view_import()
        } else {
            match self.view_mode {
                ViewMode::Main => self.view_main(),
                ViewMode::Macros => self.view_macros(),
                ViewMode::SlotManager => {
                    let t = self.config.theme;
                    slot_manager::view_slot_manager(
                        self,
                        slot_manager::SlotManagerCtx {
                            slots: &self.slots,
                            slot_triggers: &self.slot_triggers,
                            sounds: &self.sounds,
                            macros: &self.macros,
                            selected_slot: self.selected_slot,
                            configure_available: self.shortcut_config.can_open(),
                        },
                        t,
                    )
                }
                ViewMode::Settings => crate::ui::settings::view_settings(self, self.config.theme),
            }
        };

        let mut layers = vec![base];
        if let Some(notice_layer) =
            crate::ui::notice::view_notice_layer(self.notices(), self.config.theme)
        {
            layers.push(notice_layer);
        }

        iced::widget::Stack::with_children(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
