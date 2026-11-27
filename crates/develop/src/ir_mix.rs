//! Fork (irchrome/lightcraft, infrared): which infrared channel mix the Channel Mixer's IR buttons put on, and the
//! mixer it went on.
//!
//! An IR mix is applied to a base mixer, not stacked on the mix before it: choosing another IR mix replaces the one
//! that is on, and choosing the one that is on again returns to the base. The pipeline works the mixes out
//! (`lightcraft_pipeline::ir_swap`); this is only what the settings keep so it survives saving, reopening and undo.

use serde::{Deserialize, Serialize};

use crate::ChannelMixer;

/// The infrared mix that is on and the mixer it was applied to ([`crate::DevelopSettings::ir_mix`]). It describes the
/// Channel Mixer only while the mixer still shows `mix` applied to `base`: once anything else changes the mixer
/// (a slider, the neutral picker, a reset, a paste), the next IR mix starts from the mixer as it is then.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IrMixState {
    /// The mixer before the first IR mix of the current series (a white balance made in the mixer stays in it).
    pub base: ChannelMixer,
    /// The mix's id (`lightcraft_pipeline::ir_swap::IrMix::ALL`, e.g. `swapRB`).
    pub mix: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{DevelopSettings, IrMixState, MixRow, Section};

    /// Settings without an IR mix keep their JSON (and hash); one with it reads back the same; resetting the Channel
    /// Mixer section, or the whole photo, forgets it.
    #[test]
    fn stored_only_when_set_and_reset_with_the_channel_mixer() {
        let plain = DevelopSettings::default();
        assert!(plain.to_json().get("ir_mix").is_none());
        let mut d = plain.clone();
        d.channel_mixer.red = MixRow { red: 0.0, green: 0.0, blue: 100.0 };
        d.ir_mix = Some(IrMixState { base: plain.channel_mixer, mix: "swapRB".into() });
        let back = DevelopSettings::from_json(&d.to_json()).unwrap();
        assert_eq!(back, d);
        assert_eq!(d.to_json()["ir_mix"]["mix"], json!("swapRB"));
        // a partial state (an Auto Sync delta naming only the mix) still reads
        let partial = plain.merged(&json!({"ir_mix": {"mix": "monoRed"}})).unwrap();
        assert_eq!(partial.ir_mix.map(|s| s.mix), Some("monoRed".into()));
        d.reset_section(Section::ChannelMixer);
        assert!(d.ir_mix.is_none() && d.channel_mixer.is_identity());
    }
}
