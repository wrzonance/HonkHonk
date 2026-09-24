use super::super::registry::RegistryGuard;
use super::*;

pub(super) fn handle(
    ctx: &EngineCtx,
    registry: &RegistryGuard,
    mainloop: &pipewire::main_loop::MainLoopRc,
    cmd: AudioCommand,
) {
    match cmd {
        cmd @ (AudioCommand::Play { .. }
        | AudioCommand::StopVoice(_)
        | AudioCommand::Stop
        | AudioCommand::SetDynamics(_)
        | AudioCommand::SetVolume(_)) => playback_command(ctx, cmd),
        AudioCommand::SetMicPassthrough(v) => registry.apply_passthrough(v),
        AudioCommand::SetMicPassthroughLevel(_) => {}
        AudioCommand::SetMonitorDevice(target) => {
            *ctx.monitor_target.borrow_mut() = target;
            rebuild_monitor_stream(ctx);
        }
        AudioCommand::SetInputDevice(target) => {
            registry.set_input_device(target.or_else(query_default_source_name))
        }
        AudioCommand::Router(cmd) => router_command(ctx, cmd),
        AudioCommand::Shutdown => {
            let _ = ctx.voices.borrow_mut().stop_all();
            mainloop.quit();
        }
        cmd @ (AudioCommand::SetEffectBypass { .. }
        | AudioCommand::SetEffectParam { .. }
        | AudioCommand::SetEffectWetDry(_)
        | AudioCommand::SetEffectChainBypass(_)) => effect_command(ctx, cmd),
    }
}

fn playback_command(ctx: &EngineCtx, cmd: AudioCommand) {
    match cmd {
        AudioCommand::Play {
            processing,
            voice_id,
            sound_id,
            samples,
            sample_rate,
            channels,
            generation,
            gain,
            effects,
            mode,
        } => handle_play(
            ctx,
            PlayRequest {
                processing,
                voice_id,
                sound_id,
                samples,
                sample_rate,
                channels,
                generation,
                gain,
                effects,
                mode,
            },
        ),
        AudioCommand::StopVoice(id) => {
            send_finished_events(&ctx.evt_tx, ctx.voices.borrow_mut().stop_voice(id))
        }
        AudioCommand::Stop => send_finished_events(&ctx.evt_tx, ctx.voices.borrow_mut().stop_all()),
        AudioCommand::SetDynamics(settings) => ctx.voices.borrow_mut().set_dynamics(settings),
        AudioCommand::SetVolume(v) => {
            let volume = v.clamp(0.0, 1.0);
            ctx.engine_volume.set(volume);
            ctx.voices.borrow_mut().set_master_volume(volume);
        }
        _ => {} // The exhaustive dispatcher above assigns these variants.
    }
}

fn router_command(ctx: &EngineCtx, cmd: super::super::router::RouterCommand) {
    use super::super::router::RouterCommand;
    let mut router = ctx.router.borrow_mut();
    match cmd {
        RouterCommand::RouteSource { source_node_id } => {
            router.route_source(source_node_id, &ctx.core)
        }
        RouterCommand::UnrouteSource { source_node_id } => {
            router.handle_command_unroute_source(source_node_id)
        }
        RouterCommand::SetSafeMode(enabled) => router.set_safe_mode(enabled),
        RouterCommand::UndoRoutingChange => router.restore_last_change(),
        RouterCommand::UnrouteAll => router.handle_command_unroute_all(),
    }
}

fn effect_command(ctx: &EngineCtx, cmd: AudioCommand) {
    let error = match cmd {
        AudioCommand::SetEffectBypass { index, bypass } => ctx
            .mixer
            .borrow_mut()
            .chain_mut()
            .set_bypass(index, bypass)
            .err()
            .map(|e| EngineErrorEvent::EffectBypass {
                index,
                detail: e.to_string(),
            }),
        AudioCommand::SetEffectParam {
            index,
            param,
            value,
        } => ctx
            .mixer
            .borrow_mut()
            .chain_mut()
            .set_param(index, &param, value)
            .err()
            .map(|e| EngineErrorEvent::EffectParam {
                index,
                param,
                detail: e.to_string(),
            }),
        AudioCommand::SetEffectWetDry(wet_dry) => {
            ctx.mixer.borrow_mut().chain_mut().set_wet_dry(wet_dry);
            None
        }
        AudioCommand::SetEffectChainBypass(bypass) => {
            ctx.mixer.borrow_mut().chain_mut().set_chain_bypass(bypass);
            None
        }
        _ => None,
    };
    if let Some(error) = error {
        let _ = ctx.evt_tx.send(AudioEvent::Error(error));
    }
}
