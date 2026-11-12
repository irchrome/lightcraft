//! Fork (irchrome/lightcraft, infrared) commands. Kept out of upstream's command files so that a
//! sync touches two lines of `cmd.rs` and nothing else.

use lightcraft_pipeline::ir_swap::mixer_swap_rb_srgb;
use serde_json::json;

use super::{CommandSpec, bad, cmd, has_active};
use crate::{Result, Session};

fn active(s: &Session, c: &str) -> Result<lightcraft_catalog::PhotoId> {
    s.active().ok_or_else(|| bad(c, "no active photo"))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "develop.mixerSwapRBsRGB",
        "Swap Red and Blue (sRGB)",
        [],
        None,
        "{} (no parameters) → {mixer: the new mixer as {red, green, blue} rows of {red, green, blue} percentages} — the infrared swap between sRGB primaries, as sRGB-based editors do it: a gold sky turns blue where the plain swap (develop.mixerSwapRB, on Rec.2020) turns it teal; twice gives the mix back; refused when a slider would pass ±200 %; with the section's eye off it switches the section on in the same step",
        has_active,
        |s, _| {
            const C: &str = "develop.mixerSwapRBsRGB";
            let id = active(s, C)?;
            let mut d = (*s.develop_of(id).unwrap_or_default()).clone();
            d.channel_mixer = mixer_swap_rb_srgb(&d.channel_mixer).map_err(|e| bad(C, e))?;
            // like the plain swap: a swap the render ignores would look like a dead button
            if !d.section_enabled("channelMixer") {
                d.set_section_enabled("channelMixer", true);
            }
            let mixer = d.channel_mixer;
            s.set_develop(id, d, "Swap Red and Blue (sRGB)")?;
            Ok(json!({"mixer": mixer}))
        }
    )]
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
        assert!(out.get("mixer").is_some(), "{out}");
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

    /// A mix whose sRGB swap passes ±200 % is refused and left unchanged.
    #[test]
    fn srgb_swap_past_the_sliders_is_refused_unchanged() {
        let mut s = Session::with_demo();
        s.execute("develop.set", &json!({"control": "channelMixer.red.red", "value": 200.0})).unwrap();
        let before = mixer(&s);
        assert!(s.execute("develop.mixerSwapRBsRGB", &json!({})).is_err());
        assert_eq!(mixer(&s), before);
    }
}
