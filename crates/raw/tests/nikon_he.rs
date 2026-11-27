//! IR fork: with `--features nikon-he`, a Z f High Efficiency NEF decodes to sensor data instead of preview only.
//! The sample is the owner's own file (never in git): `LC_NIKON_HE_SAMPLE`, else the Z f HE frame of the IR corpus.
#![cfg(feature = "nikon-he")]

use lightcraft_raw::{RawData, decode, probe_info};

fn sample() -> Option<Vec<u8>> {
    let path = std::env::var_os("LC_NIKON_HE_SAMPLE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Documents/_IR-raw-corpus/Zf/he-sample.nef"));
    std::fs::read(path).ok()
}

#[test]
fn zf_high_efficiency_decodes_sensor_data() {
    let Some(bytes) = sample() else {
        eprintln!("skip: no Z f HE sample");
        return;
    };
    let img = decode(&bytes).expect("HE NEF decodes with nikon-he");
    assert_eq!((img.width, img.height), (6064, 4040));
    let RawData::U16(s) = &img.data else { panic!("u16 samples") };
    assert_eq!(s.len(), img.width * img.height);
    let mut sorted = s.clone();
    sorted.sort_unstable();
    let pct = |p: f64| sorted[((sorted.len() - 1) as f64 * p) as usize];
    eprintln!(
        "bits {} white {:?} black {:?} wb {:?} crop {:?} p0.1 {} p50 {} p99 {} p99.99 {}",
        img.bits,
        img.white,
        img.black,
        img.wb_multipliers,
        img.crop,
        pct(0.001),
        pct(0.5),
        pct(0.99),
        pct(0.9999)
    );
    let max = *s.iter().max().unwrap();
    let min = *s.iter().min().unwrap();
    assert!(max > 2000 && max < 16384, "14-bit range, max {max}");
    assert!(min < 1200, "near black level, min {min}");
    // not a flat or noise image: adjacent same-colour rows correlate
    let w = img.width;
    let row = |y: usize| &s[y * w..(y + 1) * w];
    let d: f64 = row(2000).iter().zip(row(2002)).map(|(a, b)| (*a as f64 - *b as f64).abs()).sum::<f64>() / w as f64;
    let spread: f64 = row(2000).iter().zip(row(100)).map(|(a, b)| (*a as f64 - *b as f64).abs()).sum::<f64>() / w as f64;
    assert!(d < spread, "neighbour rows differ by {d}, far rows by {spread}");
    probe_info(&bytes).expect("probe");
}

/// Cross-check against Adobe DNG Converter's mosaic DNG of the same file (`-u`, not linear), when
/// `LC_NIKON_HE_DNG` names one: same samples after aligning the frames and the black levels.
#[test]
fn zf_high_efficiency_matches_adobe_dng() {
    let (Some(bytes), Some(dng)) = (sample(), std::env::var_os("LC_NIKON_HE_DNG").and_then(|p| std::fs::read(p).ok())) else {
        eprintln!("skip: no sample or no Adobe DNG");
        return;
    };
    let a = decode(&bytes).unwrap();
    let b = decode(&dng).unwrap();
    let (RawData::U16(sa), RawData::U16(sb)) = (&a.data, &b.data) else { panic!("u16") };
    eprintln!(
        "NEF {}x{} black {:?}; DNG {}x{} black {:?} white {:?} active {:?}",
        a.width, a.height, a.black.values, b.width, b.height, b.black.values, b.white, b.active_area
    );
    let (ba, bb) = (a.black.values[0] as f64, b.black.values[0] as f64);
    let mut best = (f64::MAX, 0i64, 0i64, 0f64);
    for dy in -8i64..=8 {
        for dx in -8i64..=8 {
            let (mut sum, mut n, mut ratio_num, mut ratio_den) = (0f64, 0f64, 0f64, 0f64);
            for y in (1000..3000).step_by(7) {
                for x in (1000..5000).step_by(5) {
                    let (ya, xa) = (y as i64, x as i64);
                    let (yb, xb) = (ya + dy, xa + dx);
                    if yb < 0 || xb < 0 || yb as usize >= b.height || xb as usize >= b.width {
                        continue;
                    }
                    let va = sa[ya as usize * a.width + xa as usize] as f64 - ba;
                    let vb = sb[yb as usize * b.width + xb as usize] as f64 - bb;
                    sum += (va - vb).abs();
                    ratio_num += va * vb;
                    ratio_den += va * va;
                    n += 1.0;
                }
            }
            if sum / n < best.0 {
                best = (sum / n, dx, dy, ratio_num / ratio_den);
            }
        }
    }
    eprintln!("best offset dx {} dy {}: mean |diff| {:.2}, scale DNG/NEF {:.4}", best.1, best.2, best.0, best.3);
    assert!(best.0 < 2.0, "mean |diff| {}", best.0);
}
