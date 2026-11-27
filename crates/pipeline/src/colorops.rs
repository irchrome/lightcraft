//! Colour tools in OkLCh on display-linear Rec.2020 values, backed by `craft_develop::colorops`.

use std::ops::Deref;

use lightcraft_color::Mat3;
use lightcraft_develop::{Calibration, ChannelMixer, DevelopSettings, MixRow, Process};

pub use craft_develop::colorops::{
    CALIB_HUE, CALIB_SAT, POINT_WORDS, PointK, SHADOW_TINT, WheelK, band_hues, band_weights, calibrate, oklch_hue_of_srgb_hue,
};

/// The colour tools' parameters, resolved once per render (fields are read by the GPU kernel).
#[derive(Clone, Debug)]
pub struct ColorOps(pub craft_develop::colorops::ColorOps);

impl Deref for ColorOps {
    type Target = craft_develop::colorops::ColorOps;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ColorOps {
    pub fn new(s: &DevelopSettings) -> ColorOps {
        let mut ops = craft_develop::colorops::ColorOps::new(&crate::bridge::to_craft_settings(s));
        if let Some((wheels, _, _)) = ops.grading.as_mut() {
            wheels[3] = global_wheel(wheels[3], s.process.process());
        }
        ColorOps(ops)
    }
}

/// How much of V6's tint the global colour-grading wheel keeps under V13.
pub const GLOBAL_WHEEL_V13: f32 = 0.45;

/// The global wheel `k`: from V13 its tint is [`GLOBAL_WHEEL_V13`] of the other wheels' at the same saturation. On
/// Lightroom exports of real edits with Global saturation 10 (three photos) and 16 (one), V6 left the photos about
/// +2 b* and up to +2.8 a* warmer than the exports, and 0.4 to 0.6 of the tint cancels it; the shadows and
/// highlights wheels measured right as they are (Highlights 23 got further from the exports when halved). Its
/// luminance is unchanged.
fn global_wheel(k: WheelK, process: Process) -> WheelK {
    if process >= Process::V13 { WheelK { a: k.a * GLOBAL_WHEEL_V13, b: k.b * GLOBAL_WHEEL_V13, ..k } } else { k }
}

/// The calibration panel's primaries as a white-preserving 3×3 matrix on linear Rec.2020.
pub fn calibration_matrix(c: &Calibration) -> Option<[[f32; 3]; 3]> {
    craft_develop::colorops::calibration_matrix(&crate::bridge::to_craft_calibration(c))
}

/// The RGB channel mixer as a 3×3 matrix on linear Rec.2020. Returns `None` when the mixer is the
/// identity (nothing to do), or when any coefficient is non-finite (don't darken the photo on a bad input).
pub fn channel_mixer_matrix(m: &ChannelMixer) -> Option<[[f32; 3]; 3]> {
    let rows = m.rows();
    if m.is_identity() || rows.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    Some(rows.map(|r| r.map(|v| (v.clamp(-200.0, 200.0) / 100.0) as f32)))
}

/// The matrix the scene-linear colour step applies: the channel mixer first, then the calibration primaries, each
/// only when its section is on. `None` when both are neutral or off, so an untouched photo renders as before.
pub fn scene_colour_matrix(s: &DevelopSettings) -> Option<[[f32; 3]; 3]> {
    let calib = if s.section_enabled("calibration") { calibration_matrix(&s.calibration) } else { None };
    let mixer = if s.section_enabled("channelMixer") { channel_mixer_matrix(&s.channel_mixer) } else { None };
    match (calib, mixer) {
        (Some(c), Some(m)) => Some(std::array::from_fn(|r| std::array::from_fn(|col| (0..3).map(|k| c[r][k] * m[k][col]).sum()))),
        (c, m) => c.or(m),
    }
}

/// What the spot-dependent refusals of [`mixer_neutral`] say: one message the UI can translate and
/// the user act on.
pub const MIXER_NEUTRAL_ERROR: &str = "This spot can't be made neutral: pick a brighter, less saturated area";

/// What a mixer that no spot can fix refuses with: the rows can't be inverted, or the gains'
/// direction `M⁻¹ · [1,1,1]` has a non-positive part (so every spot's gains come out non-positive).
/// Picking elsewhere can't help — the mixer has to change or be reset (issue #2158).
pub const MIXER_IMPOSSIBLE_ERROR: &str =
    "The Channel Mixer's current mix can't be made neutral at any spot: change or reset the mixer, then pick again";

/// The Rec.2020 luminance weights, as the tone map reads them.
fn luminance(c: [f64; 3]) -> f64 {
    c[0] * 0.2627 + c[1] * 0.6780 + c[2] * 0.0593
}

/// The channel mixer that makes a sampled colour come out neutral — white balance at the mixer's
/// input (issue #2158, for infrared foliage). With `m` the mixer as it renders and `c` the colour at
/// the mixer's input (linear Rec.2020: the sample with the photo's current white balance already
/// applied), the new mixer is `M · diag(g)` where `g` are input gains, so a mix already set (a
/// red/blue swap) is kept: `M · diag(g) · c` is neutral when `g ⊙ c` is proportional to
/// `M⁻¹ · [1,1,1]`. `calib` is the calibration matrix the render will apply after the mixer (row-
/// major, `None` when its section is off or neutral).
///
/// The brightness the point comes out at is the spot's own with no mix: the Rec.2020 luminance of
/// `max(C · c, 0)` (the colour step clamps each channel at zero ahead of the tone map,
/// [`calibrate`], CPU and GPU alike) — not of what the current mix made of it. Testers of the PR
/// found that aiming for the mixed brightness made the result depend on the mix the pick started
/// from: on an infrared raw at 2000 K a mix pouring the strong red channel into green made the spot
/// several times brighter, and the pick kept that, raising Exposure by up to 3.5 stops and blowing
/// the photo out. With the spot's own brightness, the spot keeps the brightness it has with no mix,
/// whichever mix the pick composes over.
///
/// Calibration matrices are white-preserving (`C · [1,1,1] = [1,1,1]`, how [`calibration_matrix`]
/// normalizes them), so `C⁻¹ · [1,1,1] = [1,1,1]`: the calibration moves only the brightness
/// target, and `C · M' · c` comes out neutral at it. The calibration's shadow tint comes after the
/// colour matrix and touches only dark tones, and is not modelled: a dark spot with a shadow tint
/// set may keep a little of it. Returns the new nine values in per cent and how much the whole
/// matrix was scaled down to fit the ±200 % sliders (1.0 when it already fitted; the point stays
/// neutral at that lower luminance, and Exposure can compensate). `Err` ([`MIXER_IMPOSSIBLE_ERROR`])
/// when no spot can fix the mixer, or ([`MIXER_NEUTRAL_ERROR`]) when the sample has no usable signal (a
/// component ≤ 1e-6, or not a number) or the gains come out non-finite or non-positive; nothing is
/// changed then.
pub fn mixer_neutral(m: &ChannelMixer, c: [f64; 3], calib: Option<[[f64; 3]; 3]>) -> Result<(ChannelMixer, f64), &'static str> {
    let rows = m.rows().map(|r| r.map(|v| v.clamp(-200.0, 200.0) / 100.0));
    let mat = Mat3(rows);
    let u = mat.inverse().ok_or(MIXER_IMPOSSIBLE_ERROR)?.apply([1.0, 1.0, 1.0]);
    if u.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
        return Err(MIXER_IMPOSSIBLE_ERROR);
    }
    if c.iter().any(|v| !(v.is_finite() && *v > 1e-6)) {
        return Err(MIXER_NEUTRAL_ERROR);
    }
    let own = calib.map_or(c, |cm| Mat3(cm).apply(c));
    let q = luminance(own.map(|v| v.max(0.0)));
    let g = [q * u[0] / c[0], q * u[1] / c[1], q * u[2] / c[2]];
    if !(q.is_finite() && q > 0.0) || g.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
        return Err(MIXER_NEUTRAL_ERROR);
    }
    let mut new = rows;
    for r in &mut new {
        for (j, v) in r.iter_mut().enumerate() {
            *v *= g[j];
        }
    }
    let done = Mat3(new).apply(c);
    if done.iter().any(|v| !v.is_finite()) || (done[0] - done[1]).abs() > 1e-6 * q || (done[1] - done[2]).abs() > 1e-6 * q {
        return Err(MIXER_NEUTRAL_ERROR);
    }
    let max = new.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
    let scaled = if max > 2.0 { 2.0 / max } else { 1.0 };
    let row = |r: usize| MixRow { red: new[r][0] * scaled * 100.0, green: new[r][1] * scaled * 100.0, blue: new[r][2] * scaled * 100.0 };
    Ok((ChannelMixer { red: row(0), green: row(1), blue: row(2) }, scaled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v13_global_wheel_tints_less_and_earlier_processes_keep_it() {
        let tint = |p: Process, global: bool| {
            let mut s = DevelopSettings { process: p.version(), ..Default::default() };
            let w = lightcraft_develop::Wheel { hue: 35.0, sat: 16.0, lum: 10.0 };
            if global {
                s.grading.global = w
            } else {
                s.grading.shadows = w
            }
            let ops = ColorOps::new(&s);
            let (wheels, _, _) = ops.grading.expect("grading");
            let k = wheels[if global { 3 } else { 0 }];
            (k.a, k.b, k.lum)
        };
        let v10 = tint(Process::V10, true);
        for p in Process::ALL.into_iter().filter(|p| *p < Process::V13) {
            assert_eq!(tint(p, true), v10, "{p:?} tints as before");
        }
        let v13 = tint(Process::V13, true);
        assert!(v10.0 != 0.0 && v10.1 != 0.0);
        assert!(v13.0.abs() < v10.0.abs() && v13.1.abs() < v10.1.abs(), "V13's Global wheel tints less: {v13:?} {v10:?}");
        assert_eq!((v13.0.signum(), v13.1.signum(), v13.2), (v10.0.signum(), v10.1.signum(), v10.2), "same hue, same luminance");
        assert_eq!(tint(Process::V10, false), tint(Process::V13, false), "the shadows wheel is unchanged");
    }

    #[test]
    fn channel_mixer_swaps_channels_and_composes_before_calibration() {
        use lightcraft_develop::{ChannelMixer, MixRow};
        let mut s = DevelopSettings::default();
        assert!(channel_mixer_matrix(&s.channel_mixer).is_none());
        assert!(scene_colour_matrix(&s).is_none(), "an untouched photo keeps the step off");

        s.channel_mixer = ChannelMixer {
            red: MixRow { blue: 100.0, ..MixRow::default() },
            green: MixRow { green: 100.0, ..MixRow::default() },
            blue: MixRow { red: 100.0, ..MixRow::default() },
        };
        let m = scene_colour_matrix(&s).unwrap();
        assert_eq!(calibrate([0.4, 0.1, 0.02], Some(&m), 0.0), [0.02, 0.1, 0.4]);

        s.channel_mixer.green = MixRow { green: 100.0, red: -200.0, ..MixRow::default() };
        let out = calibrate([0.4, 0.1, 0.02], Some(&scene_colour_matrix(&s).unwrap()), 0.0);
        assert_eq!(out[1], 0.0);

        s.channel_mixer.green = MixRow { green: 100.0, ..MixRow::default() };
        s.calibration = Calibration { red_hue: 60.0, ..Default::default() };
        let both = scene_colour_matrix(&s).unwrap();
        let (c, x) = (calibration_matrix(&s.calibration).unwrap(), channel_mixer_matrix(&s.channel_mixer).unwrap());
        let input = [0.3, 0.2, 0.1];
        let seq = calibrate(calibrate(input, Some(&x), 0.0), Some(&c), 0.0);
        let one = calibrate(input, Some(&both), 0.0);
        assert!(seq.iter().zip(one).all(|(a, b)| (a - b).abs() < 1e-5), "{seq:?} vs {one:?}");

        s.set_section_enabled("channelMixer", false);
        assert_eq!(scene_colour_matrix(&s), Some(c));
        s.set_section_enabled("calibration", false);
        assert!(scene_colour_matrix(&s).is_none());

        s.set_section_enabled("channelMixer", true);
        s.channel_mixer.red.red = f64::NAN;
        assert!(channel_mixer_matrix(&s.channel_mixer).is_none());
    }

    #[test]
    fn mixer_neutral_picks_input_gains() {
        let apply = |m: &ChannelMixer, c: [f64; 3]| Mat3(m.rows().map(|r| r.map(|v| v / 100.0))).apply(c);
        let red_cast = [0.4, 0.2, 0.15];
        let (m, scaled) = mixer_neutral(&ChannelMixer::default(), red_cast, None).unwrap();
        assert_eq!(scaled, 1.0);
        let out = apply(&m, red_cast);
        assert!((out[0] - out[1]).abs() < 1e-4 && (out[1] - out[2]).abs() < 1e-4, "{out:?}");
        assert!((luminance(out) - luminance(red_cast)).abs() < 1e-9);

        let swap = ChannelMixer {
            red: MixRow { blue: 100.0, ..MixRow::default() },
            green: MixRow { green: 100.0, ..MixRow::default() },
            blue: MixRow { red: 100.0, ..MixRow::default() },
        };
        let (m, scaled) = mixer_neutral(&swap, red_cast, None).unwrap();
        assert_eq!(scaled, 1.0);
        let out = apply(&m, red_cast);
        assert!((out[0] - out[1]).abs() < 1e-4 && (out[1] - out[2]).abs() < 1e-4, "{out:?}");
        assert!((luminance(out) - luminance(red_cast)).abs() < 1e-9);

        let extreme = [1.0, 0.01, 0.02];
        let (m, scaled) = mixer_neutral(&ChannelMixer::default(), extreme, None).unwrap();
        assert!(scaled < 1.0);
        for v in m.rows().iter().flatten() {
            assert!(*v >= -200.0 && *v <= 200.0 && v.is_finite());
        }
        let out = apply(&m, extreme);
        assert!((out[0] - out[1]).abs() < 1e-4 && (out[1] - out[2]).abs() < 1e-4, "{out:?}");
        assert!((luminance(out) - scaled * luminance(extreme)).abs() < 1e-9);
    }

    #[test]
    fn mixer_neutral_refuses_the_mixer_before_the_spot() {
        let same = MixRow { red: 100.0, green: 0.0, blue: 0.0 };
        let singular = ChannelMixer { red: same, green: same, blue: same };
        let negative = ChannelMixer {
            red: MixRow { red: 100.0, green: 0.0, blue: 0.0 },
            green: MixRow { red: 0.0, green: 100.0, blue: 0.0 },
            blue: MixRow { red: 100.0, green: 100.0, blue: 100.0 },
        };
        for (name, m) in [("singular", singular), ("negative gain", negative)] {
            for spot in [[0.0, 0.0, 0.0], [1e-9, 0.2, 0.2], [f64::NAN, 0.2, 0.2], [0.3, 0.3, 0.3]] {
                assert_eq!(mixer_neutral(&m, spot, None), Err(MIXER_IMPOSSIBLE_ERROR), "{name} at {spot:?}");
            }
        }
        assert_eq!(mixer_neutral(&ChannelMixer::default(), [0.0, 0.0, 0.0], None), Err(MIXER_NEUTRAL_ERROR));
    }

    #[test]
    fn mixer_neutral_targets_the_calibrated_luminance() {
        let apply = |m: &ChannelMixer, c: [f64; 3]| Mat3(m.rows().map(|r| r.map(|v| v / 100.0))).apply(c);
        let calib = calibration_matrix(&Calibration { red_hue: 40.0, red_sat: 30.0, ..Default::default() })
            .map(|m| m.map(|r| r.map(f64::from)))
            .expect("an invertible calibration");
        let spot = [0.4, 0.2, 0.15];
        let shown = Mat3(calib).apply(spot).map(|v| v.max(0.0));
        let (picked, scaled) = mixer_neutral(&ChannelMixer::default(), spot, Some(calib)).unwrap();
        let out = apply(&picked, spot);
        assert!((out[0] - out[1]).abs() < 1e-4 && (out[1] - out[2]).abs() < 1e-4);
        assert!((luminance(out) - scaled * luminance(shown)).abs() < 1e-9);
    }
}
