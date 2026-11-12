//! Fork (irchrome/lightcraft, infrared) commands. Kept out of upstream's command files so that a
//! sync touches two lines of `cmd.rs` and nothing else.

use lightcraft_pipeline::ir_swap::{IrMix, apply_ir_mix};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_active, str_param};
use crate::{Result, Session};

fn active(s: &Session, c: &str) -> Result<lightcraft_catalog::PhotoId> {
    s.active().ok_or_else(|| bad(c, "no active photo"))
}

/// Apply `mix` to the active photo's Channel Mixer in one undo step, switching the section on when its eye is
/// off (a mix the render ignores would look like a dead button).
fn run_mix(s: &mut Session, c: &str, mix: IrMix, label: &str) -> Result<Value> {
    let id = active(s, c)?;
    let mut d = (*s.develop_of(id).unwrap_or_default()).clone();
    let (mixer, darker) = apply_ir_mix(mix, &d.channel_mixer).map_err(|e| bad(c, e))?;
    d.channel_mixer = mixer;
    if !d.section_enabled("channelMixer") {
        d.set_section_enabled("channelMixer", true);
    }
    s.set_develop(id, d, label)?;
    Ok(json!({"mixer": mixer, "darker": darker}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "develop.mixerSwapRBsRGB",
            "Swap Red and Blue (sRGB)",
            [],
            None,
            "{} (no parameters) → {mixer, darker: EV the photo came out darker when the mix had to be scaled into ±200 %} — the infrared swap between sRGB primaries, as sRGB-based editors do it: a gold sky turns azure where the plain swap (develop.mixerSwapRB, on Rec.2020) turns it teal; twice gives the mix back; with the section's eye off it switches the section on in the same step",
            has_active,
            |s, _| run_mix(s, "develop.mixerSwapRBsRGB", IrMix::SwapRB, "Swap Red and Blue (sRGB)")
        ),
        cmd!(
            "develop.mixerIrMix",
            "Infrared Channel Mix",
            [],
            None,
            "{mix: swapRB | swapRG | swapGB | rotateRGB | rotateRBG | swapRB70 | swapRB30 | monoRed} → {mixer, darker} — an infrared channel mix between sRGB primaries applied on top of the current mix (a white balance or neutral pick made before stays in); one undo step; scaled into ±200 % when needed (`darker` says by how many EV)",
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
