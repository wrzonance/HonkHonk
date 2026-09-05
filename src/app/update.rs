//! Exhaustive Iced message routing; feature transitions live in sibling modules.

use super::*;

impl HonkHonk {
    #[allow(
        clippy::too_many_lines,
        reason = "exhaustive message routing delegates cohesive transitions to app modules"
    )]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NoOp => Task::none(),
            Message::ToggleVisibility => self.toggle_visibility(),
            Message::Quit => self.quit(),
            Message::TrayEvent(event) => {
                let msg = Message::from_tray_event(event);
                self.update(msg)
            }
            Message::TrayPoll => self.poll_tray(),
            Message::AudioEvent(event) => self.handle_audio_event(event),
            Message::RaiseNotice(notice) => {
                self.notices.push(notice, Instant::now());
                Task::none()
            }
            Message::DismissNotice(id) => {
                self.notices.dismiss(id);
                Task::none()
            }
            Message::NoticeTick(now) => {
                self.notices.expire(now);
                Task::none()
            }
            Message::PlaySound(sound_id) => {
                if let Some(sound) = self.sounds.iter().find(|s| s.id == sound_id).cloned() {
                    self.request_play(&sound, false)
                } else {
                    Task::none()
                }
            }
            Message::StopAll => self.stop_all(),
            Message::StartRecording => {
                self.start_recording_at(Instant::now());
                Task::none()
            }
            Message::StopRecording => {
                self.stop_recording();
                Task::none()
            }
            Message::PlayMacro(id) => self.play_macro(&id),
            Message::MacroStepDue { run_id, step } => self.on_macro_step_due(run_id, step),
            Message::MacroStepDecoded {
                run_id,
                voice_id,
                sound_id,
                gain,
                effects,
                result,
            } => self.on_macro_step_decoded(
                run_id,
                macros::MacroVoice {
                    voice_id,
                    sound_id,
                    gain,
                    effects,
                },
                result,
            ),
            Message::SelectCategory(cat) => {
                self.select_sound_category(cat);
                Task::none()
            }
            Message::EscapePressed => self.handle_escape(false),
            Message::CapturedEscapePressed => self.handle_escape(true),
            Message::SearchChanged(query) => {
                self.replace_filter_query(query);
                Task::none()
            }
            Message::TypeToFilter(text) => self.handle_type_to_filter(&text),
            Message::ToggleSoundSortMenu => {
                self.toggle_sound_sort_menu();
                Task::none()
            }
            Message::ToggleSoundSortDirection => {
                self.toggle_sound_sort_direction();
                Task::none()
            }
            Message::SelectSoundSort(key) => {
                self.select_sound_sort(key);
                Task::none()
            }
            Message::ToggleSoundTagGrouping => {
                self.toggle_sound_tag_grouping();
                Task::none()
            }
            Message::DismissSoundSortMenu => {
                self.dismiss_sound_sort_menu();
                Task::none()
            }
            Message::HotkeySearchChanged(query) => {
                self.replace_hotkey_filter_query(query);
                Task::none()
            }
            Message::ToggleHotkeySortMenu => {
                self.toggle_hotkey_sort_menu();
                Task::none()
            }
            Message::ToggleHotkeySortDirection => {
                self.toggle_hotkey_sort_direction();
                Task::none()
            }
            Message::SelectHotkeySort(key) => {
                self.select_hotkey_sort(key);
                Task::none()
            }
            Message::DismissHotkeySortMenu => {
                self.dismiss_hotkey_sort_menu();
                Task::none()
            }
            Message::SlotSearchChanged(query) => {
                self.replace_slot_filter_query(query);
                Task::none()
            }
            Message::ToggleSlotSortMenu => {
                self.toggle_slot_sort_menu();
                Task::none()
            }
            Message::ToggleSlotSortDirection => {
                self.toggle_slot_sort_direction();
                Task::none()
            }
            Message::SelectSlotSort(key) => {
                self.select_slot_sort(key);
                Task::none()
            }
            Message::DismissSlotSortMenu => {
                self.dismiss_slot_sort_menu();
                Task::none()
            }
            Message::VolumeChanged(v) => {
                self.config.volume = v.clamp(0.0, 1.0);
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::SetVolume(self.config.volume));
                }
                Task::none()
            }
            Message::VolumeSaveRequested => {
                self.persist_config();
                Task::none()
            }
            Message::ShortcutsReady => {
                self.shortcuts_status = ShortcutsStatus::Active;
                Task::none()
            }
            Message::ShortcutsUnavailable(reason) => {
                self.shortcuts_status = ShortcutsStatus::Unavailable(reason);
                Task::none()
            }
            Message::DismissShortcutsWarning => {
                self.shortcuts_warning_dismissed = true;
                Task::none()
            }
            Message::ShortcutActivated(idx) => self.activate_slot(idx),
            Message::ShortcutBindingsUpdated(bindings) => {
                for (idx, trigger) in bindings {
                    if let Some(slot) = self.slot_triggers.get_mut(idx as usize) {
                        *slot = Some(trigger);
                    }
                }
                Task::none()
            }
            Message::DurationsLoaded(map) => {
                self.apply_loaded_durations(&map);
                Task::none()
            }
            Message::AssignSlot(idx, path) => {
                self.slots.set(idx, path);
                self.persist_slots();
                Task::none()
            }
            Message::AssignMacroSlot(idx, macro_id) => self.assign_macro_slot(idx, macro_id),
            Message::ClearSlot(idx) => {
                self.slots.clear(idx);
                self.persist_slots();
                Task::none()
            }
            Message::OpenContextMenu(sound_id) => {
                self.context_menu = Some(sound_id);
                self.context_menu_pos = Some(self.cursor_pos);
                Task::none()
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                self.context_menu_pos = None;
                Task::none()
            }
            Message::CursorMoved(pos) => {
                self.cursor_pos = pos;
                Task::none()
            }
            Message::WindowResized(w, h) => self.resize_window(w, h),
            Message::Frame(now) => {
                self.tick_frame(now);
                Task::none()
            }
            Message::ShowSlots => {
                self.dismiss_sound_sort_menu();
                self.view_mode = ViewMode::SlotManager;
                self.selected_slot = None;
                Task::none()
            }
            Message::ShowMain => {
                self.dismiss_sound_sort_menu();
                self.view_mode = ViewMode::Main;
                self.selected_slot = None;
                Task::none()
            }
            Message::Settings(message) => self.update_settings(message),
            Message::SelectSlot(idx) => {
                self.selected_slot = Some(idx);
                Task::none()
            }
            Message::RescanLibrary => self.rescan_library(),
            Message::AddSoundDirectory => Task::perform(
                async {
                    match pick_directory().await {
                        Ok(opt) => opt,
                        Err(e) => {
                            tracing::warn!(error = ?e, "directory picker error");
                            None
                        }
                    }
                },
                Message::SoundDirectoryPickResult,
            ),
            Message::SoundDirectoryPickResult(Some(path)) => self.add_sound_directory(path),
            Message::SoundDirectoryPickResult(None) => Task::none(),
            Message::RemoveSoundDirectory(path) => self.remove_sound_directory(path),
            Message::ThemeChanged(t) => self.change_theme(t),
            Message::DensityChanged(d) => self.change_density(d),
            Message::PanelAnimationsChanged(enabled) => self.set_panel_animations(enabled),
            Message::RendererChanged(r) => self.change_renderer(r),
            Message::MicPassthroughChanged(v) => self.change_mic_passthrough(v),
            Message::MicPassthroughLevelChanged(v) => self.change_mic_passthrough_level(v),
            Message::OverlapModeChanged(overlap_mode) => self.change_overlap_mode(overlap_mode),
            Message::MonitorDeviceChanged(target) => self.change_monitor_device(target),
            Message::InputDeviceChanged(target) => self.change_input_device(target),
            Message::SelectEffectPreset(preset) => {
                let cmds = effects_panel::select_preset(&mut self.effects_ui, preset);
                self.send_audio_commands(cmds);
                Task::none()
            }
            Message::SetEffectBypassUi(bypass) => {
                let cmd = effects_panel::set_chain_bypass(&mut self.effects_ui, bypass);
                self.send_audio_commands([cmd]);
                Task::none()
            }
            Message::SetWetDryMix(mix) => {
                let cmd = effects_panel::set_wet_dry(&mut self.effects_ui, mix);
                self.send_audio_commands([cmd]);
                Task::none()
            }
            Message::SetEffectParamUi { slot, param, value } => {
                let cmds = effects_panel::edit_param(&mut self.effects_ui, slot, param, value);
                self.send_audio_commands(cmds);
                Task::none()
            }
            Message::ToggleEffectsPanel => self.toggle_effects_panel(),
            Message::CloseEffectsPanel => self.close_effects_panel(),
            Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender)) => {
                self.shortcut_config.set_portal_sender(sender);
                Task::none()
            }
            Message::ShortcutsConfigureAvailable(available) => {
                self.shortcut_config.set_portal_v2_available(available);
                Task::none()
            }
            Message::OpenShortcutConfig => {
                self.shortcut_config.open();
                Task::none()
            }
            Message::ToggleFavorite(sound_id) => {
                self.toggle_sound_favorite(&sound_id);
                Task::none()
            }
            Message::OpenSoundEditor(sound_id) => self.open_sound_editor(sound_id),
            Message::CloseSoundEditor => {
                self.editor_sound_id = None;
                self.editor_draft_name = String::new();
                self.editor_draft_tags.clear();
                self.editor_draft_volume = 1.0;
                Task::none()
            }
            Message::SoundEditorNameChanged(name) => {
                self.editor_draft_name = name;
                Task::none()
            }
            Message::SoundEditorTagsChanged(tags) => {
                self.editor_draft_tags = tags;
                Task::none()
            }
            Message::SoundEditorVolumeChanged(_sound_id, v) => {
                self.editor_draft_volume = v.clamp(0.0, 2.0);
                Task::none()
            }
            Message::SaveSoundMeta(sound_id) => {
                self.save_sound_metadata(sound_id);
                Task::none()
            }
            Message::Decoded {
                generation,
                voice_id,
                id,
                result,
                gain,
                effects,
                mode,
            } => self.handle_decoded(
                id,
                result,
                playback::PlaybackDispatch {
                    generation,
                    voice_id,
                    gain,
                    effects,
                    mode,
                },
            ),
        }
    }
}
