//! Fork (irchrome/lightcraft, infrared): channel mixes between sRGB primaries.
//!
//! The Channel Mixer works on linear Rec.2020. Swapping its red and blue output rows there turns the
//! gold sky of a white-balanced infrared frame teal (h ≈ 216°) and pushes dark foliage towards
//! salmon, where sRGB-based editors give the classic azure sky (h ≈ 251°). Every [`IrMix`] here is
//! defined between sRGB primaries, as those editors do it, and applied on top of the current mix:
//! `P · S · P⁻¹ · M`, with `P` linear sRGB → linear Rec.2020, `S` the mix and `M` the current mixer
//! (so a white balance or a neutral pick made before stays in). Each `S` keeps white white (its
//! rows sum to one), hence so does the result. A result past the ±200 % sliders is scaled down as a
//! whole, like the neutral picker does: colours stay, the photo comes out darker by `darker` EV.

use lightcraft_color::{Mat3, REC2020, SRGB};
use lightcraft_develop::{ChannelMixer, MixRow};

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
        }
        assert_eq!(IrMix::from_id("nope"), None);
    }
}
