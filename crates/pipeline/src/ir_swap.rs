//! Fork (irchrome/lightcraft, infrared): the red/blue channel swap between sRGB primaries.
//!
//! The Channel Mixer works on linear Rec.2020. Swapping its red and blue output rows there turns the
//! gold sky of a white-balanced infrared frame teal and pushes dark foliage towards salmon, where
//! sRGB-based editors give the classic blue sky. [`mixer_swap_rb_srgb`] swaps red and blue between
//! sRGB primaries instead: the new mix is `P · S · P⁻¹ · M`, with `P` linear sRGB → linear Rec.2020,
//! `S` the red/blue swap and `M` the current mix. White stays white (each row of `P·S·P⁻¹` sums to
//! one) and doing it twice gives the mix back. Measured on an X-S20 + 720 nm frame white-balanced on
//! foliage: sky hue 232° with the Rec.2020 swap, 266° with this one (owner's target 270-290°).

use lightcraft_color::{Mat3, REC2020, SRGB};
use lightcraft_develop::{ChannelMixer, MixRow};

/// What [`mixer_swap_rb_srgb`] refuses with: the swapped mix needs a slider past ±200 %.
pub const MIXER_SWAP_SRGB_RANGE_ERROR: &str =
    "The sRGB swap of this mix needs more than ±200 %: reset the Channel Mixer or pick a brighter spot, then swap again";

/// The red/blue swap expressed on linear Rec.2020: `P · S · P⁻¹`.
pub fn swap_rb_srgb_matrix() -> Mat3 {
    let p = SRGB.to_space(&REC2020);
    let s = Mat3([[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]);
    // P is built from fixed, non-collinear primaries: its inverse exists
    let p_inv = p.inverse().unwrap_or(Mat3::IDENTITY);
    p.mul(&s).mul(&p_inv)
}

/// The mix after swapping red and blue between sRGB primaries (see the module docs). A value within
/// 1e-6 % of a whole per cent snaps to it, so slider values stay whole and swapping the default mix
/// twice gives back the identity, which the stored settings leave out.
pub fn mixer_swap_rb_srgb(m: &ChannelMixer) -> Result<ChannelMixer, &'static str> {
    let new = swap_rb_srgb_matrix().mul(&Mat3(m.rows())).0;
    let round = |v: f64| if (v - v.round()).abs() < 1e-6 { v.round() + 0.0 } else { v };
    if new.iter().flatten().any(|v| !v.is_finite() || round(*v).abs() > 200.0) {
        return Err(MIXER_SWAP_SRGB_RANGE_ERROR);
    }
    let row = |r: usize| MixRow { red: round(new[r][0]), green: round(new[r][1]), blue: round(new[r][2]) };
    Ok(ChannelMixer { red: row(0), green: row(1), blue: row(2) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// The identity mix becomes `P·S·P⁻¹` in per cent; each row sums to 100 (white stays white).
    #[test]
    fn identity_becomes_the_srgb_swap_and_keeps_white() {
        let m = mixer_swap_rb_srgb(&ChannelMixer::default()).unwrap();
        let rows = m.rows();
        for r in rows {
            assert!(close(r.iter().sum::<f64>(), 100.0, 1e-4), "{rows:?}");
        }
        // published BT.2087 primaries give red-from-blue ≈ 69.6 % and blue-from-red ≈ 147.6 %
        assert!(close(rows[0][2], 69.6, 0.2) && close(rows[2][0], 147.6, 0.2), "{rows:?}");
    }

    /// Twice gives the mix back: exactly for whole per cents (the default mix is left out of the
    /// settings again), to rounding for a picked mix.
    #[test]
    fn twice_gives_the_mix_back() {
        let once = mixer_swap_rb_srgb(&ChannelMixer::default()).unwrap();
        assert!(mixer_swap_rb_srgb(&once).unwrap().is_identity());
        let picked = ChannelMixer {
            red: MixRow { red: 57.9, ..MixRow::default() },
            green: MixRow { green: 163.6, ..MixRow::default() },
            blue: MixRow { blue: 45.0, ..MixRow::default() },
        };
        let back = mixer_swap_rb_srgb(&mixer_swap_rb_srgb(&picked).unwrap()).unwrap();
        for (b, p) in back.rows().iter().flatten().zip(picked.rows().iter().flatten()) {
            assert!(close(*b, *p, 1e-9), "{back:?}");
        }
    }

    /// The defining property: an sRGB colour comes out with its sRGB red and blue swapped.
    #[test]
    fn an_srgb_gold_comes_out_srgb_blue() {
        let to2020 = SRGB.to_space(&REC2020);
        let gold = [0.9, 0.6, 0.2];
        let mixed = swap_rb_srgb_matrix().apply(to2020.apply(gold));
        let back = to2020.inverse().unwrap().apply(mixed);
        for (got, want) in back.iter().zip([0.2, 0.6, 0.9]) {
            assert!(close(*got, want, 1e-9), "{back:?}");
        }
    }

    /// A mix whose swap needs a slider past ±200 % is refused rather than clipped.
    #[test]
    fn a_swap_past_the_sliders_is_refused() {
        let strong = ChannelMixer { red: MixRow { red: 200.0, ..MixRow::default() }, ..ChannelMixer::default() };
        assert_eq!(mixer_swap_rb_srgb(&strong), Err(MIXER_SWAP_SRGB_RANGE_ERROR));
    }
}
