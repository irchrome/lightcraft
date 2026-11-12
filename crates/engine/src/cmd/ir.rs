//! Fork (irchrome/lightcraft, infrared) commands. Kept out of upstream's command files so that a
//! sync touches two lines of `cmd.rs` and nothing else.

use lightcraft_pipeline::ir_swap::{IrMix, active_ir_mix, choose_ir_mix};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_active, str_param};
use crate::{Result, Session};

fn active(s: &Session, c: &str) -> Result<lightcraft_catalog::PhotoId> {
    s.active().ok_or_else(|| bad(c, "no active photo"))
}

/// Choose `mix` for the active photo's Channel Mixer in one undo step ([`choose_ir_mix`]: it replaces the IR mix
/// that is on, or turns it off when it is that one), switching the section on when its eye is off (a mix the render
/// ignores would look like a dead button).
fn run_mix(s: &mut Session, c: &str, mix: IrMix, label: &str) -> Result<Value> {
    let id = active(s, c)?;
    let mut d = (*s.develop_of(id).unwrap_or_default()).clone();
    let darker = choose_ir_mix(&mut d, mix).map_err(|e| bad(c, e))?;
    if !d.section_enabled("channelMixer") {
        d.set_section_enabled("channelMixer", true);
    }
    let (mixer, on) = (d.channel_mixer, active_ir_mix(&d).map(IrMix::id));
    s.set_develop(id, d, label)?;
    Ok(json!({"mixer": mixer, "darker": darker, "irMix": on}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "develop.mixerSwapRBsRGB",
            "Swap Red and Blue (sRGB)",
            [],
            None,
            "{} (no parameters) → {mixer, darker: EV the photo came out darker when the mix had to be scaled into ±200 %, irMix: the IR mix now on or null} — the infrared swap between sRGB primaries, as sRGB-based editors do it: a gold sky turns azure where the plain swap (develop.mixerSwapRB, on Rec.2020) turns it teal; the same as develop.mixerIrMix {mix: swapRB}: it replaces the IR mix that is on, and twice gives the mix back; with the section's eye off it switches the section on in the same step",
            has_active,
            |s, _| run_mix(s, "develop.mixerSwapRBsRGB", IrMix::SwapRB, "Swap Red and Blue (sRGB)")
        ),
        cmd!(
            "develop.mixerIrMix",
            "Infrared Channel Mix",
            [],
            None,
            "{mix: swapRB | swapRG | swapGB | rotateRGB | rotateRBG | swapRB70 | swapRB30 | monoRed} → {mixer, darker, irMix: the IR mix now on or null} — an infrared channel mix between sRGB primaries applied to the mixer from before the first IR mix (a white balance or neutral pick made before stays in): it replaces the IR mix that is on, and the mix that is on, chosen again, returns to that mixer; a mixer changed since by anything else is the new starting point; one undo step; scaled into ±200 % when needed (`darker` says by how many EV)",
            has_active,
            |s, p| {
                const C: &str = "develop.mixerIrMix";
                let name = str_param(p, "mix").ok_or_else(|| bad(C, "missing `mix`"))?;
                let (mix, _, label) =
                    IrMix::ALL.iter().find(|(_, id, _)| *id == name).copied().ok_or_else(|| bad(C, format!("unknown mix `{name}`")))?;
                run_mix(s, C, mix, label)
            }
        ),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    fn mixer(s: &Session) -> lightcraft_develop::ChannelMixer {
        s.develop_of(s.active().unwrap()).unwrap().channel_mixer
    }

    /// The sRGB swap switches the section on, keeps white (rows sum to 100), and one undo restores
    /// the mix and the section's off state; twice gives the default mix back.
    #[test]
    fn srgb_swap_is_one_undo_step_and_twice_is_the_identity() {
        let mut s = Session::with_demo();
        s.execute("develop.sectionEnabled", &json!({"section": "channelMixer", "enabled": false})).unwrap();
        let out = s.execute("develop.mixerSwapRBsRGB", &json!({})).unwrap();
        assert!(out.get("mixer").is_some() && out["darker"] == 0.0, "{out}");
        let d = s.develop_of(s.active().unwrap()).unwrap();
        assert!(d.section_enabled("channelMixer"), "the swap switched the section on");
        for r in d.channel_mixer.rows() {
            assert!((r.iter().sum::<f64>() - 100.0).abs() < 1e-4, "{r:?}");
        }
        assert!(d.channel_mixer.blue.red > 140.0, "blue takes mostly from red: {:?}", d.channel_mixer);

        s.execute("edit.undo", &json!({})).unwrap();
        let d = s.develop_of(s.active().unwrap()).unwrap();
        assert!(d.channel_mixer.is_identity() && !d.section_enabled("channelMixer"), "undo restores both");

        s.execute("develop.mixerSwapRBsRGB", &json!({})).unwrap();
        s.execute("develop.mixerSwapRBsRGB", &json!({})).unwrap();
        assert!(mixer(&s).is_identity(), "{:?}", mixer(&s));
    }

    /// Every named mix runs, keeps white, and is one undo step; an unknown name is refused unchanged.
    #[test]
    fn every_ir_mix_runs_and_undoes() {
        let mut s = Session::with_demo();
        for (_, id, _) in lightcraft_pipeline::ir_swap::IrMix::ALL {
            let out = s.execute("develop.mixerIrMix", &json!({"mix": id})).unwrap();
            let k = 100.0 * 2f64.powf(-out["darker"].as_f64().unwrap());
            for r in mixer(&s).rows() {
                assert!((r.iter().sum::<f64>() - k).abs() < 1e-4, "{id}: {r:?}");
            }
            s.execute("edit.undo", &json!({})).unwrap();
            assert!(mixer(&s).is_identity(), "{id}: undo");
        }
        assert!(s.execute("develop.mixerIrMix", &json!({"mix": "nope"})).is_err());
        assert!(s.execute("develop.mixerIrMix", &json!({})).is_err());
        assert!(mixer(&s).is_identity());
    }

    fn set_mixer(s: &mut Session, m: &lightcraft_develop::ChannelMixer) {
        for (row, out) in m.rows().iter().zip(["red", "green", "blue"]) {
            for (v, inp) in row.iter().zip(["red", "green", "blue"]) {
                s.execute("develop.set", &json!({"control": format!("channelMixer.{out}.{inp}"), "value": v})).unwrap();
            }
        }
    }

    fn base() -> lightcraft_develop::ChannelMixer {
        use lightcraft_develop::MixRow;
        lightcraft_develop::ChannelMixer {
            red: MixRow { red: 58.0, ..MixRow::default() },
            green: MixRow { green: 164.0, ..MixRow::default() },
            blue: MixRow { blue: 45.0, ..MixRow::default() },
        }
    }

    /// The owner's bug (2026-11-12): after "Mono from Red" further mixes did nothing. Each click now replaces the IR
    /// mix that is on (applied to the mixer from before the first one), the mix that is on clicked again returns to
    /// that mixer, and every click is one undo step that restores the mixer and the stored IR mix.
    #[test]
    fn mixes_replace_each_other_toggle_off_and_undo() {
        use lightcraft_pipeline::ir_swap::{IrMix, apply_ir_mix};
        let mut s = Session::with_demo();
        set_mixer(&mut s, &base());
        assert_eq!(mixer(&s), base());
        let mut states = vec![(*s.develop_of(s.active().unwrap()).unwrap()).clone()];
        let mono = s.execute("develop.mixerIrMix", &json!({"mix": "monoRed"})).unwrap();
        assert_eq!(mono["irMix"], "monoRed");
        states.push((*s.develop_of(s.active().unwrap()).unwrap()).clone());
        let swap = s.execute("develop.mixerIrMix", &json!({"mix": "swapRB"})).unwrap();
        assert_eq!(mixer(&s), apply_ir_mix(IrMix::SwapRB, &base()).unwrap().0, "the swap of the base, not of mono");
        assert_eq!(swap["irMix"], "swapRB");
        states.push((*s.develop_of(s.active().unwrap()).unwrap()).clone());
        // the sRGB swap button is the same mix: it turns it off
        let off = s.execute("develop.mixerSwapRBsRGB", &json!({})).unwrap();
        assert_eq!((mixer(&s), off["irMix"].clone()), (base(), json!(null)));
        assert!(s.develop_of(s.active().unwrap()).unwrap().ir_mix.is_none());
        while let Some(want) = states.pop() {
            s.execute("edit.undo", &json!({})).unwrap();
            let d = s.develop_of(s.active().unwrap()).unwrap();
            assert_eq!((d.channel_mixer, d.ir_mix.clone()), (want.channel_mixer, want.ir_mix.clone()), "undo to {:?}", want.ir_mix);
        }
    }

    /// A slider moved after a mix makes the moved mixer the next mix's starting point.
    #[test]
    fn a_slider_after_a_mix_is_the_next_base() {
        use lightcraft_pipeline::ir_swap::{IrMix, apply_ir_mix};
        let mut s = Session::with_demo();
        s.execute("develop.mixerIrMix", &json!({"mix": "swapRG"})).unwrap();
        s.execute("develop.set", &json!({"control": "channelMixer.green.green", "value": 150.0})).unwrap();
        let moved = mixer(&s);
        let out = s.execute("develop.mixerIrMix", &json!({"mix": "swapRG"})).unwrap();
        assert_eq!(out["irMix"], "swapRG", "a new mix, not the old one turned off");
        assert_eq!(mixer(&s), apply_ir_mix(IrMix::SwapRG, &moved).unwrap().0);
    }

    /// A mix past ±200 % is scaled down, not refused, and says how much darker it came out.
    #[test]
    fn a_mix_past_the_sliders_is_scaled() {
        let mut s = Session::with_demo();
        s.execute("develop.set", &json!({"control": "channelMixer.red.red", "value": 200.0})).unwrap();
        let out = s.execute("develop.mixerSwapRBsRGB", &json!({})).unwrap();
        assert!(out["darker"].as_f64().unwrap() > 0.0, "{out}");
        assert!(mixer(&s).rows().iter().flatten().all(|v| v.abs() <= 200.0 + 1e-9));
    }
}
