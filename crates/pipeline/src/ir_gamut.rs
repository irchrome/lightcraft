//! IR fork: soft gamut compression for infrared photos (owner request 2026-11-13).
//!
//! An infrared photo white-balanced on foliage puts the sky far outside any RGB gamut: a channel
//! goes strongly negative. Upstream brings such colours in per pixel, by hard clipping (the colour
//! step clamps channels at zero) and by desaturating each pixel just onto the gamut boundary
//! ([`crate::finish::gamut_map_to`]). Every colour past the boundary then lands on the same surface
//! (one channel exactly 0), and a sky becomes a flat yellow or cyan fill with hard edges.
//!
//! Here colours are compressed towards the achromatic axis first, as the ACES Reference Gamut
//! Compression does (ACES 1.3, a published algorithm): per channel, the distance
//! `d = (max − c) / max` (0 neutral, 1 on the boundary, >1 outside) is kept below [`THRESHOLD`] and
//! compressed smoothly above it so that [`LIMIT`] lands on the boundary. More saturated stays more
//! saturated, so the gradient survives; the hard clip after it only catches what is still beyond.
//! Applied only to infrared edits ([`active`]): a user DCP camera profile or a channel mixer that
//! isn't neutral. Mirrored in `lightcraft-gpu`'s `finish.wgsl` (`ir_compress`).

use lightcraft_develop::DevelopSettings;

/// Distances below this are left alone.
pub const THRESHOLD: f32 = 0.7;
/// The distance that lands on the gamut boundary (d = 1).
pub const LIMIT: f32 = 2.2;
/// The compression curve's exponent (ACES: 1.2).
pub const POWER: f32 = 1.2;

/// Whether a photo's edit is an infrared one: a user camera profile (`dcp:` ids, the imported .dcp
/// profiles) or a channel mixer that is on and not neutral.
pub fn active(s: &DevelopSettings) -> bool {
    s.profile.id.starts_with("dcp:") || (s.section_enabled("channelMixer") && !s.channel_mixer.is_identity())
}

/// The scale `s` of the ACES compression curve for [`THRESHOLD`], [`LIMIT`], [`POWER`].
pub fn scale() -> f32 {
    (LIMIT - THRESHOLD) / (((1.0 - THRESHOLD) / (LIMIT - THRESHOLD)).powf(-POWER) - 1.0).powf(1.0 / POWER)
}

#[inline]
fn compress_distance(d: f32, s: f32) -> f32 {
    if d < THRESHOLD {
        return d;
    }
    let n = (d - THRESHOLD) / s;
    THRESHOLD + s * n / (1.0 + n.powf(POWER)).powf(1.0 / POWER)
}

/// `c` compressed towards its achromatic value (the largest channel); unchanged when in gamut far
/// enough, or when it has no positive channel.
#[inline]
pub fn compress(c: [f32; 3]) -> [f32; 3] {
    let ach = c[0].max(c[1]).max(c[2]);
    if ach.is_nan() || ach <= 0.0 {
        return c;
    }
    let s = scale();
    c.map(|v| {
        let d = (ach - v) / ach;
        if d < THRESHOLD { v } else { ach - compress_distance(d, s) * ach }
    })
}

/// The output-space colour before the hard gamut map: compressed when `ir`.
#[inline]
pub fn output(r: [f32; 3], ir: bool) -> [f32; 3] {
    if ir { compress(r) } else { r }
}

/// `colorops::calibrate` for the per-pixel stage: with `ir` and a mixer/calibration matrix, the mixed
/// colour is compressed before the step's clamp at zero, so a negative channel isn't cut flat.
#[inline]
pub fn calibrate(c: [f32; 3], calib: Option<&[[f32; 3]; 3]>, shadow_tint: f32, ir: bool) -> [f32; 3] {
    match (ir, calib) {
        (true, Some(m)) => {
            let mixed = std::array::from_fn(|r| m[r][0] * c[0] + m[r][1] * c[1] + m[r][2] * c[2]);
            crate::colorops::calibrate(compress(mixed).map(|v| v.max(0.0)), None, shadow_tint)
        }
        _ => crate::colorops::calibrate(c, calib, shadow_tint),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_lands_on_the_boundary_and_inside_stays() {
        let s = scale();
        assert!((compress_distance(LIMIT, s) - 1.0).abs() < 1e-4);
        assert_eq!(compress_distance(0.5, s), 0.5);
        assert_eq!(compress([0.2, 0.5, 0.4]), [0.2, 0.5, 0.4]);
    }

    #[test]
    fn monotonic_so_a_gradient_survives() {
        // a sky running further and further out of gamut keeps distinct values after compression
        let s = scale();
        let mut last = -1.0;
        for i in 0..400 {
            let d = i as f32 * 0.01;
            let cd = compress_distance(d, s);
            assert!(cd > last, "{d}: {cd} <= {last}");
            last = cd;
        }
        let a = compress([0.9, 0.8, -0.3]);
        let b = compress([0.9, 0.8, -0.6]);
        assert!(a[2] > b[2] && b[2] > -0.6, "{a:?} {b:?}");
    }

    #[test]
    fn only_infrared_edits() {
        let mut s = DevelopSettings::default();
        assert!(!active(&s));
        s.profile.id = "dcp:nikon-z-f-x".into();
        assert!(active(&s));
        s.profile.id = "lc.color".into();
        s.channel_mixer.red.blue = 100.0;
        s.channel_mixer.red.red = 0.0;
        assert!(active(&s));
        s.disabled_sections.push("channelMixer".into());
        assert!(!active(&s));
    }

    #[test]
    fn the_gpu_kernel_uses_the_same_constants() {
        let wgsl = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../gpu/src/wgsl/finish.wgsl")).unwrap();
        assert!(wgsl.contains(&format!("const IR_THRESHOLD: f32 = {THRESHOLD:?};")), "IR_THRESHOLD");
        assert!(wgsl.contains(&format!("const IR_POWER: f32 = {POWER:?};")), "IR_POWER");
    }
}
