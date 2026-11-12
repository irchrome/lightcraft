//! Fork (irchrome/lightcraft, infrared): channel mixes between sRGB primaries.
//!
//! The Channel Mixer works on linear Rec.2020. Swapping its red and blue output rows there turns the
//! gold sky of a white-balanced infrared frame teal (h ≈ 216°) and pushes dark foliage towards
//! salmon, where sRGB-based editors give the classic azure sky (h ≈ 251°). Every [`IrMix`] here is
//! defined between sRGB primaries, as those editors do it, and applied to a base mixer:
//! `P · S · P⁻¹ · B`, with `P` linear sRGB → linear Rec.2020, `S` the mix and `B` the mixer before
//! the first IR mix of the current series (so a white balance or a neutral pick made before stays
//! in). Each `S` keeps white white (its rows sum to one), hence so does the result. A result past
//! the ±200 % sliders is scaled down as a whole, like the neutral picker does: colours stay, the
//! photo comes out darker by `darker` EV.
//!
//! The buttons choose one mix ([`choose_ir_mix`]): another mix replaces the one that is on (never
//! stacks on it: after "Mono from Red" every row is the same and a further mix stacked on it did
//! nothing), and the mix that is on, chosen again, returns to the base. The settings keep the base
//! and the mix ([`IrMixState`]); a mixer changed since by anything else becomes the next base.

use lightcraft_color::{Mat3, REC2020, SRGB};
use lightcraft_develop::{ChannelMixer, DevelopSettings, IrMixState, MixRow};

/// What a mix refuses with: a value that is not a number (a damaged mixer).
pub const IR_MIX_ERROR: &str = "This Channel Mixer can't be mixed further: reset it, then try again";

/// The infrared channel mixes the fork offers, all between sRGB primaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrMix {
    /// Red and blue swapped: the classic false-colour swap (blue sky, cream or gold foliage).
    SwapRB,
    /// Red and green swapped.
    SwapRG,
    /// Green and blue swapped.
    SwapGB,
    /// Red input goes to green, green to blue, blue to red.
    RotateRGB,
    /// Red input goes to blue, blue to green, green to red.
    RotateRBG,
    /// Red and blue 70 % swapped: softer sky, pastel pink or gold foliage.
    SwapRB70,
    /// Red and blue 30 % swapped: mostly false colour, warm red to rust foliage.
    SwapRB30,
    /// Every output takes the red input: monochrome from the channel that carries most infrared.
    MonoRed,
}

impl IrMix {
    /// Every mix with its command id and English label (translated in the UI).
    pub const ALL: [(IrMix, &'static str, &'static str); 8] = [
        (IrMix::SwapRB, "swapRB", "Red ↔ Blue"),
        (IrMix::SwapRG, "swapRG", "Red ↔ Green"),
        (IrMix::SwapGB, "swapGB", "Green ↔ Blue"),
        (IrMix::RotateRGB, "rotateRGB", "Rotate R → G → B"),
        (IrMix::RotateRBG, "rotateRBG", "Rotate R → B → G"),
        (IrMix::SwapRB70, "swapRB70", "Red ↔ Blue 70 %"),
        (IrMix::SwapRB30, "swapRB30", "Red ↔ Blue 30 %"),
        (IrMix::MonoRed, "monoRed", "Mono from Red (IR)"),
    ];

    /// The mix named `id` (see [`IrMix::ALL`]).
    pub fn from_id(id: &str) -> Option<IrMix> {
        IrMix::ALL.iter().find(|(_, i, _)| *i == id).map(|(m, _, _)| *m)
    }

    /// The mix's command id (see [`IrMix::ALL`]).
    pub fn id(self) -> &'static str {
        IrMix::ALL.iter().find(|(m, _, _)| *m == self).map_or("", |(_, id, _)| id)
    }

    /// The mix between sRGB primaries (rows: red, green, blue outputs; columns: inputs).
    pub fn srgb(self) -> Mat3 {
        let partial = |s: f64| Mat3([[1.0 - s, 0.0, s], [0.0, 1.0, 0.0], [s, 0.0, 1.0 - s]]);
        match self {
            IrMix::SwapRB => Mat3([[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]),
            IrMix::SwapRG => Mat3([[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]),
            IrMix::SwapGB => Mat3([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]]),
            IrMix::RotateRGB => Mat3([[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            IrMix::RotateRBG => Mat3([[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]),
            IrMix::SwapRB70 => partial(0.7),
            IrMix::SwapRB30 => partial(0.3),
            IrMix::MonoRed => Mat3([[1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]),
        }
    }

    /// The same mix expressed on the mixer's linear Rec.2020: `P · S · P⁻¹`.
    pub fn rec2020(self) -> Mat3 {
        let p = SRGB.to_space(&REC2020);
        // P is built from fixed, non-collinear primaries: its inverse exists
        let p_inv = p.inverse().unwrap_or(Mat3::IDENTITY);
        p.mul(&self.srgb()).mul(&p_inv)
    }
}

/// The swap between sRGB primaries on Rec.2020 (kept for callers of the first fork version).
pub fn swap_rb_srgb_matrix() -> Mat3 {
    IrMix::SwapRB.rec2020()
}

/// The mixer after `mix` (see the module docs) and how much darker the photo comes out (EV, 0 when
/// it fits the sliders). A value within 1e-6 % of a whole per cent snaps to it, so slider values
/// stay whole and swapping the default mixer twice gives back the identity.
pub fn apply_ir_mix(mix: IrMix, m: &ChannelMixer) -> Result<(ChannelMixer, f64), &'static str> {
    let mut new = mix.rec2020().mul(&Mat3(m.rows())).0;
    if new.iter().flatten().any(|v| !v.is_finite()) {
        return Err(IR_MIX_ERROR);
    }
    let max = new.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
    let scaled = if max > 200.0 { 200.0 / max } else { 1.0 };
    let snap = |v: f64| if (v - v.round()).abs() < 1e-6 { v.round() + 0.0 } else { v };
    for v in new.iter_mut().flatten() {
        *v = snap(*v * scaled);
    }
    let row = |r: usize| MixRow { red: new[r][0], green: new[r][1], blue: new[r][2] };
    Ok((ChannelMixer { red: row(0), green: row(1), blue: row(2) }, -scaled.log2() + 0.0))
}

/// The red/blue swap between sRGB primaries (`develop.mixerSwapRBsRGB`).
pub fn mixer_swap_rb_srgb(m: &ChannelMixer) -> Result<(ChannelMixer, f64), &'static str> {
    apply_ir_mix(IrMix::SwapRB, m)
}

/// How far a mixer coefficient (per cent) may be from the mix worked out again and still count as it.
const SAME: f64 = 1e-6;

/// The IR mix the Channel Mixer of `d` shows: the one its [`IrMixState`] names, while the mixer is still that mix
/// applied to the state's base. `None` once anything else changed the mixer.
pub fn active_ir_mix(d: &DevelopSettings) -> Option<IrMix> {
    let state = d.ir_mix.as_ref()?;
    let mix = IrMix::from_id(&state.mix)?;
    let (want, _) = apply_ir_mix(mix, &state.base).ok()?;
    let same = want.rows().iter().flatten().zip(d.channel_mixer.rows().iter().flatten()).all(|(a, b)| (a - b).abs() <= SAME);
    same.then_some(mix)
}

/// Choose IR mix `mix` for settings `d` (an IR mix button): `mix` applied to the base mixer, replacing the IR mix
/// that is on; `mix` itself when it is on: back to the base, with no IR mix on. The base is the state's while
/// [`active_ir_mix`] holds, else the mixer as it is now. Returns how much darker the photo comes out (EV, see
/// [`apply_ir_mix`]); the Channel Mixer section's eye is left to the caller.
pub fn choose_ir_mix(d: &mut DevelopSettings, mix: IrMix) -> Result<f64, &'static str> {
    let on = active_ir_mix(d);
    let base = match (&d.ir_mix, on) {
        (Some(state), Some(_)) => state.base,
        _ => d.channel_mixer,
    };
    if on == Some(mix) {
        d.channel_mixer = base;
        d.ir_mix = None;
        return Ok(0.0);
    }
    let (mixer, darker) = apply_ir_mix(mix, &base)?;
    d.channel_mixer = mixer;
    d.ir_mix = Some(IrMixState { base, mix: mix.id().to_string() });
    Ok(darker)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// Every mix keeps white white: each row of the result sums to 100 % (times the scale).
    #[test]
    fn every_mix_keeps_white() {
        for (mix, id, _) in IrMix::ALL {
            let (m, darker) = apply_ir_mix(mix, &ChannelMixer::default()).unwrap();
            let k = 2f64.powf(-darker) * 100.0;
            for r in m.rows() {
                assert!(close(r.iter().sum::<f64>(), k, 1e-4), "{id}: {:?}", m.rows());
            }
        }
    }

    /// The identity becomes `P·S·P⁻¹`: published BT.2087 primaries give red-from-blue ≈ 69.6 % and
    /// blue-from-red ≈ 147.6 % for the swap.
    #[test]
    fn the_swap_matches_the_published_primaries() {
        let (m, darker) = mixer_swap_rb_srgb(&ChannelMixer::default()).unwrap();
        let rows = m.rows();
        assert_eq!(darker, 0.0);
        assert!(close(rows[0][2], 69.6, 0.2) && close(rows[2][0], 147.6, 0.2), "{rows:?}");
    }

    /// Swaps twice give the mix back (exactly for whole per cents); rotations three times.
    #[test]
    fn swaps_twice_and_rotations_thrice_give_the_mix_back() {
        for (mix, n) in [(IrMix::SwapRB, 2), (IrMix::SwapRG, 2), (IrMix::SwapGB, 2), (IrMix::RotateRGB, 3), (IrMix::RotateRBG, 3)] {
            let mut m = ChannelMixer::default();
            for _ in 0..n {
                m = apply_ir_mix(mix, &m).unwrap().0;
            }
            assert!(m.is_identity(), "{mix:?}: {:?}", m.rows());
        }
        let picked = ChannelMixer {
            red: MixRow { red: 57.9, ..MixRow::default() },
            green: MixRow { green: 163.6, ..MixRow::default() },
            blue: MixRow { blue: 45.0, ..MixRow::default() },
        };
        let back = mixer_swap_rb_srgb(&mixer_swap_rb_srgb(&picked).unwrap().0).unwrap().0;
        for (b, p) in back.rows().iter().flatten().zip(picked.rows().iter().flatten()) {
            assert!(close(*b, *p, 1e-9), "{back:?}");
        }
    }

    /// The defining property: an sRGB colour comes out with its sRGB channels mixed by `S`.
    #[test]
    fn mixes_act_on_srgb_colours() {
        let to2020 = SRGB.to_space(&REC2020);
        let back = to2020.inverse().unwrap();
        let gold = [0.9, 0.6, 0.2];
        for (mix, want) in [
            (IrMix::SwapRB, [0.2, 0.6, 0.9]),
            (IrMix::SwapRG, [0.6, 0.9, 0.2]),
            (IrMix::RotateRGB, [0.2, 0.9, 0.6]),
            (IrMix::SwapRB70, [0.41, 0.6, 0.69]),
            (IrMix::MonoRed, [0.9, 0.9, 0.9]),
        ] {
            let got = back.apply(mix.rec2020().apply(to2020.apply(gold)));
            for (g, w) in got.iter().zip(want) {
                assert!(close(*g, w, 1e-9), "{mix:?}: {got:?}");
            }
        }
    }

    /// A result past ±200 % is scaled down as a whole and says how much darker it is.
    #[test]
    fn a_mix_past_the_sliders_is_scaled_and_says_by_how_much() {
        let strong = ChannelMixer { red: MixRow { red: 200.0, ..MixRow::default() }, ..ChannelMixer::default() };
        let (m, darker) = mixer_swap_rb_srgb(&strong).unwrap();
        assert!(darker > 0.0, "{darker}");
        assert!(m.rows().iter().flatten().all(|v| v.abs() <= 200.0 + 1e-9), "{:?}", m.rows());
    }

    #[test]
    fn ids_round_trip() {
        for (mix, id, _) in IrMix::ALL {
            assert_eq!(IrMix::from_id(id), Some(mix));
            assert_eq!(mix.id(), id);
        }
        assert_eq!(IrMix::from_id("nope"), None);
    }

    /// A mixer with a white balance in it (as a neutral pick leaves it).
    fn picked() -> ChannelMixer {
        ChannelMixer {
            red: MixRow { red: 57.9, ..MixRow::default() },
            green: MixRow { green: 163.6, ..MixRow::default() },
            blue: MixRow { blue: 45.0, ..MixRow::default() },
        }
    }

    fn with_mixer(m: ChannelMixer) -> DevelopSettings {
        DevelopSettings { channel_mixer: m, ..DevelopSettings::default() }
    }

    /// The owner's bug: after "Mono from Red" every row was the same and further mixes, stacked on it, did nothing.
    /// Another mix now replaces it: exactly that mix on the mixer from before the first one.
    #[test]
    fn a_mix_after_mono_from_red_replaces_it() {
        let mut d = with_mixer(picked());
        choose_ir_mix(&mut d, IrMix::MonoRed).unwrap();
        let mono = d.channel_mixer.rows();
        assert!(mono.iter().all(|r| r.iter().zip(mono[0]).all(|(a, b)| close(*a, b, 1e-9))), "mono: {mono:?}");
        assert_eq!(active_ir_mix(&d), Some(IrMix::MonoRed));
        let darker = choose_ir_mix(&mut d, IrMix::SwapRB).unwrap();
        let (want, want_darker) = apply_ir_mix(IrMix::SwapRB, &picked()).unwrap();
        assert_eq!((d.channel_mixer, darker), (want, want_darker), "the swap on the picked mixer, not on mono");
        assert_eq!(active_ir_mix(&d), Some(IrMix::SwapRB));
        assert_eq!(d.ir_mix.as_ref().map(|s| s.base), Some(picked()));
        // every mix in turn: never stacked, never scaled further
        for (mix, _, _) in IrMix::ALL.into_iter().chain(IrMix::ALL) {
            if active_ir_mix(&d) == Some(mix) {
                continue;
            }
            choose_ir_mix(&mut d, mix).unwrap();
            assert_eq!(d.channel_mixer, apply_ir_mix(mix, &picked()).unwrap().0, "{mix:?}");
        }
    }

    /// The mix that is on, chosen again, returns to the base: no IR mix is on then.
    #[test]
    fn the_mix_that_is_on_chosen_again_returns_to_the_base() {
        for (mix, _, _) in IrMix::ALL {
            let mut d = with_mixer(picked());
            choose_ir_mix(&mut d, mix).unwrap();
            assert_ne!(d.channel_mixer, picked(), "{mix:?}");
            assert_eq!(choose_ir_mix(&mut d, mix).unwrap(), 0.0);
            assert_eq!((d.channel_mixer, d.ir_mix.clone(), active_ir_mix(&d)), (picked(), None, None), "{mix:?}");
        }
    }

    /// A mixer changed by anything else after a mix (here a slider) is the next mix's base; the stale state says
    /// no mix is on.
    #[test]
    fn a_mixer_changed_since_is_the_next_base() {
        let mut d = with_mixer(picked());
        choose_ir_mix(&mut d, IrMix::SwapRB).unwrap();
        d.channel_mixer.green.green = 120.0;
        assert_eq!(active_ir_mix(&d), None);
        let changed = d.channel_mixer;
        choose_ir_mix(&mut d, IrMix::SwapRB).unwrap();
        assert_eq!(d.channel_mixer, apply_ir_mix(IrMix::SwapRB, &changed).unwrap().0, "a swap of the changed mixer, not a toggle");
        assert_eq!(d.ir_mix.map(|s| s.base), Some(changed));
    }

    /// A state that names no known mix, or whose base can't be mixed, is not a mix that is on.
    #[test]
    fn a_damaged_state_is_no_mix() {
        let mut d = with_mixer(picked());
        d.ir_mix = Some(IrMixState { base: picked(), mix: "nope".into() });
        assert_eq!(active_ir_mix(&d), None);
        choose_ir_mix(&mut d, IrMix::SwapGB).unwrap();
        assert_eq!(d.channel_mixer, apply_ir_mix(IrMix::SwapGB, &picked()).unwrap().0);
    }
}
