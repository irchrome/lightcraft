//! Auto tone and auto white balance, backed by `craft_develop::auto`.

use lightcraft_develop::DevelopSettings;
use lightcraft_raster::Rgb32f;

use crate::SourceInfo;

pub use craft_develop::auto::AutoTone;

/// The scene's log-average luminance under the current white balance, in EV relative to middle grey.
pub fn scene_log_mean(src: &Rgb32f, info: &SourceInfo, s: &DevelopSettings) -> Option<f64> {
    craft_develop::auto::scene_log_mean(
        &crate::bridge::to_craft_rgb32f(src),
        &crate::bridge::to_craft_wb_source(info),
        &crate::bridge::to_craft_settings(s),
    )
}

/// Auto tone settings for `src`.
pub fn auto_tone(src: &Rgb32f, info: &SourceInfo, s: &DevelopSettings) -> AutoTone {
    craft_develop::auto::auto_tone(
        &crate::bridge::to_craft_rgb32f(src),
        &crate::bridge::to_craft_wb_source(info),
        &crate::bridge::to_craft_settings(s),
    )
}

/// Auto B&W mix for `src`.
pub fn auto_bw_mix(src: &Rgb32f, info: &SourceInfo, s: &DevelopSettings) -> [f64; 8] {
    craft_develop::auto::auto_bw_mix(
        &crate::bridge::to_craft_rgb32f(src),
        &crate::bridge::to_craft_wb_source(info),
        &crate::bridge::to_craft_settings(s),
    )
}

/// Auto white balance `(temp, tint)` for `src`.
///
/// Mirrors `craft_develop::auto::auto_wb_with`, clamping to [`lightcraft_develop::controls::WB_MIN_TEMP`]
/// (1300 K) rather than 2000 K so infrared RAW white-balance picks do not hit a 2000 K floor.
pub fn auto_wb(src: &Rgb32f, info: &SourceInfo) -> (f64, f64) {
    let img = lightcraft_raster::resample::fit(src, 256, 256, lightcraft_raster::resample::Filter::Box);
    let (mut acc, mut wsum) = ([0.0f64; 3], 0.0f64);
    for c in &img.data {
        let y = lightcraft_color::luminance_2020(*c);
        if !(0.01..=2.0).contains(&y) {
            continue;
        }
        let mx = c[0].max(c[1]).max(c[2]);
        let mn = c[0].min(c[1]).min(c[2]);
        let chroma = (mx - mn) / (mx + 1e-6);
        let w = (1.0 - chroma).powi(2) as f64 * (1.0 - ((y.log2() + 2.5) / 4.0).abs().min(1.0)) as f64;
        for i in 0..3 {
            acc[i] += c[i] as f64 * w;
        }
        wsum += w;
    }
    if wsum <= 0.0 {
        return (info.as_shot_temp, info.as_shot_tint);
    }
    let avg = acc.map(|v| v / wsum);
    let seen = match info
        .camera_color
        .as_deref()
        .filter(|_| info.raw && !info.relative_wb)
        .and_then(|cc| lightcraft_raw::color::neutral_white(&cc.tags, cc.developed_for, avg))
    {
        Some(white) => white,
        None => {
            let shot = lightcraft_color::cct::temp_tint_to_xy(info.as_shot_temp, info.as_shot_tint);
            let xyz = lightcraft_color::bradford(lightcraft_color::REC2020.white, shot).apply(lightcraft_color::REC2020.to_xyz().apply(avg));
            if xyz[0] + xyz[1] + xyz[2] <= 0.0 {
                return (info.as_shot_temp, info.as_shot_tint);
            }
            lightcraft_color::Xy::from_xyz(xyz)
        }
    };
    let (t, tint) = lightcraft_color::cct::xy_to_temp_tint(seen);
    (t.clamp(lightcraft_develop::controls::WB_MIN_TEMP, 50000.0).round(), tint.clamp(-150.0, 150.0).round())
}

/// Current white balance `(temp, tint)` for `info` and `s`.
pub fn current_wb(info: &SourceInfo, s: &DevelopSettings) -> (f64, f64) {
    craft_develop::auto::current_wb(&crate::bridge::to_craft_wb_source(info), &crate::bridge::to_craft_settings(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_wb_allows_infrared_kelvin_below_2000() {
        let warm_ir = Rgb32f::filled(16, 16, [0.35, 0.06, 0.015]);
        let info = SourceInfo { raw: true, as_shot_temp: 5500.0, as_shot_tint: 0.0, ..Default::default() };
        let (temp, _) = auto_wb(&warm_ir, &info);
        assert!((lightcraft_develop::controls::WB_MIN_TEMP..2000.0).contains(&temp), "expected < 2000 K for warm IR sample, got {temp}");
    }
}
