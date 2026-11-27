//! Fork (irchrome/lightcraft, infrared): dedicated infrared sections inside the `Edit` panel:
//! - `IR-FALSE-COLOR`: neutral pickers, WB, sRGB false-color mixes (built-in + hub catalog), vibrance/saturation
//! - `IR-MONO`: neutral pickers, infrared monochrome mixes, tone & contrast controls
//! - `AEROCHROME`: camera-bound Chrome/Aerochrome DCP profiles, WB, foliage/sky HSL & primary calibration

use egui::{CornerRadius, Sense, Stroke, StrokeKind, pos2, vec2};
use lightcraft_catalog::{MediaKind, PhotoId};
use lightcraft_develop::DevelopSettings;
use lightcraft_engine::cmd::dcp_profiles::is_dcp_id;
use lightcraft_pipeline::ir_swap::active_hub_ir_mix;
use lightcraft_pipeline::ir_swap::catalog::{HUB_IR_MIXES, HubIrMixSpec};
use serde_json::json;

use super::edit::{REL_TEMP, REL_TINT, apply_slider_out, control, k_to_rel, rel_to_k, sub_title};
use super::ir_mix::{render_group, reset_swap_preview, run};
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{divider, flyout_row, icon_button, one_line, register, section_header, text_button};
use crate::{HoverPreview, LightcraftApp};

const FALSE_COLOR_FAMILIES: &[(&str, &str)] = &[
    ("all", "All"),
    ("builtin", "Built-in"),
    ("swap", "Swaps"),
    ("swap-variant", "Swap Variants"),
    ("creative", "Creative"),
    ("fit", "Fitted Looks"),
];

fn is_chrome_profile(pid: &str, name: &str) -> bool {
    let p = pid.to_ascii_lowercase();
    let n = name.to_ascii_lowercase();
    p.contains("chrome") || p.contains("aero") || n.contains("chrome") || n.contains("aero")
}

fn ir_section(app: &mut LightcraftApp, ui: &mut egui::Ui, id: &str, title: &str, body: impl FnOnce(&mut LightcraftApp, &mut egui::Ui)) {
    let open = app.ui.section_open(id);
    let (resp, _) = section_header(ui, id, title, open, None);
    if resp.clicked() {
        let solo = ui.input(|i| i.modifiers.alt);
        if let Err(e) = app.run(&format!("section.{id}"), json!({"solo": solo})) {
            app.toast(ui.ctx(), e);
        }
    }
    crate::widgets::context_menu(&resp, |ui| {
        let mut solo = app.ui.single_panel;
        let item = ui.checkbox(&mut solo, crate::i18n::tr("Solo Mode"));
        register(ui, "sectionMenu:solo", item.rect);
        if item.clicked() {
            if let Err(e) = app.run("view.soloSections", json!({"on": solo})) {
                app.toast(ui.ctx(), e);
            }
            ui.close();
        }
    });
    if open {
        ui.push_id(("irSection", id), |ui| body(app, ui));
    }
    divider(ui);
}

fn wb_controls(app: &mut LightcraftApp, ui: &mut egui::Ui, d: &DevelopSettings, raw: bool) {
    if raw {
        control(app, ui, d, "wb.temp", true);
        control(app, ui, d, "wb.tint", true);
    } else {
        let out = crate::widgets::slider(ui, &REL_TEMP, k_to_rel(d.wb.temp), true, None);
        apply_slider_out(app, &REL_TEMP, out, |app, v| app.run("develop.set", json!({"control": "wb.temp", "value": rel_to_k(v)})));
        let out = crate::widgets::slider(ui, &REL_TINT, d.wb.tint.clamp(-100.0, 100.0), true, None);
        apply_slider_out(app, &REL_TINT, out, |app, v| app.run("develop.set", json!({"control": "wb.tint", "value": v})));
    }
}

fn picker_bar(app: &mut LightcraftApp, ui: &mut egui::Ui, wb_id: &str, mixer_id: &str, reset_id: &str, reset_section: &str) {
    egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 4 }).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            let wb_active = app.ui.tool == "wbPicker";
            if icon_button(
                ui,
                wb_id,
                Icon::Picker,
                vec2(28.0, 28.0),
                wb_active,
                true,
                "White Balance Selector (W) — pick foliage or grey after selecting an IR DCP",
            )
            .clicked()
            {
                if wb_active {
                    app.ui.tool.clear();
                } else if let Err(e) = app.run("tool.wbPicker", json!({})) {
                    app.toast(ui.ctx(), e);
                }
            }
            let mix_active = app.ui.tool == "mixerPicker";
            if icon_button(
                ui,
                mixer_id,
                Icon::Target,
                vec2(28.0, 28.0),
                mix_active,
                true,
                "Channel Mixer Neutral — forces picked spot to exact white",
            )
            .clicked()
            {
                if mix_active {
                    app.ui.tool.clear();
                } else if let Err(e) = app.run("tool.mixerPicker", json!({})) {
                    app.toast(ui.ctx(), e);
                }
            }
            if text_button(ui, reset_id, "Reset", false).clicked() {
                let _ = app.run("develop.resetSection", json!({"section": reset_section}));
            }
        });
    });
}

fn false_color_section(app: &mut LightcraftApp, ui: &mut egui::Ui, d: &DevelopSettings, raw: bool) {
    picker_bar(app, ui, "irFcWbPicker", "irFcMixerPicker", "irFcResetMixer", "channelMixer");
    wb_controls(app, ui, d, raw);

    let active_spec = active_hub_ir_mix(d);
    let active_id = active_spec.map(|s| s.id);
    let fam_id = egui::Id::unique("irFcFamily");
    let selected: &'static str = ui.data(|m| m.get_temp(fam_id)).unwrap_or("all");

    sub_title(ui, "False-Color Mixes");
    egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 6 }).show(ui, |ui| {
        if text_button(ui, "irFcSwapRB", "Swap Red and Blue (sRGB)", active_spec.is_some_and(|s| s.ir_mix_id == Some("swapRB"))).clicked() {
            run(app, ui, "develop.mixerSwapRBsRGB", json!({}));
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (key, label) in FALSE_COLOR_FAMILIES {
                if text_button(ui, &format!("irFcFilter:{key}"), label, selected == *key).clicked() {
                    ui.data_mut(|m| m.insert_temp(fam_id, *key));
                }
            }
        });
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 3.0;
        let is_fc_builtin = |s: &&HubIrMixSpec| s.ir_mix_id.is_some() && s.ir_mix_id != Some("swapRB") && s.family != "mono";
        if selected == "all" {
            let builtins: Vec<&HubIrMixSpec> = HUB_IR_MIXES.iter().filter(is_fc_builtin).collect();
            render_group(app, ui, d, active_id, Some("Built-in"), "irFcMix", &builtins);
            for (key, label) in &FALSE_COLOR_FAMILIES[2..] {
                let group: Vec<&HubIrMixSpec> =
                    HUB_IR_MIXES.iter().filter(|s| s.ir_mix_id.is_none() && s.family != "mono" && s.family == *key).collect();
                render_group(app, ui, d, active_id, Some(label), "irFcMix", &group);
            }
            let other: Vec<&HubIrMixSpec> = HUB_IR_MIXES
                .iter()
                .filter(|s| s.ir_mix_id.is_none() && s.family != "mono" && !FALSE_COLOR_FAMILIES[2..].iter().any(|(k, _)| *k == s.family))
                .collect();
            render_group(app, ui, d, active_id, Some("Other"), "irFcMix", &other);
        } else if selected == "builtin" {
            let builtins: Vec<&HubIrMixSpec> = HUB_IR_MIXES.iter().filter(is_fc_builtin).collect();
            render_group(app, ui, d, active_id, None, "irFcMix", &builtins);
        } else {
            let filtered: Vec<&HubIrMixSpec> =
                HUB_IR_MIXES.iter().filter(|s| s.ir_mix_id.is_none() && s.family != "mono" && s.family == selected).collect();
            render_group(app, ui, d, active_id, None, "irFcMix", &filtered);
        }
    });

    control(app, ui, d, "color.vibrance", true);
    control(app, ui, d, "color.saturation", true);
    ui.add_space(8.0);
}

fn mono_section(app: &mut LightcraftApp, ui: &mut egui::Ui, d: &DevelopSettings) {
    picker_bar(app, ui, "irMonoWbPicker", "irMonoMixerPicker", "irMonoResetMixer", "channelMixer");

    let active_id = active_hub_ir_mix(d).map(|s| s.id);
    let monos: Vec<&HubIrMixSpec> = HUB_IR_MIXES.iter().filter(|s| s.family == "mono").collect();
    egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 6 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        render_group(app, ui, d, active_id, None, "irMonoMix", &monos);
    });

    sub_title(ui, "Tone & Contrast");
    for c in ["light.exposure", "light.contrast", "light.whites", "light.blacks", "effects.clarity", "effects.dehaze"] {
        control(app, ui, d, c, true);
    }
    ui.add_space(8.0);
}

fn chrome_profile_row(app: &mut LightcraftApp, ui: &mut egui::Ui, d: &DevelopSettings, pid: &str, label: &str) {
    let t = Tokens::get(ui.ctx());
    let active = d.profile.id == pid;
    let row_w = ui.available_width().max(40.0);
    let fav = app.session.profile_favorites.iter().any(|f| f == pid);
    let shown = if fav { format!("★ {label}") } else { label.to_string() };
    let (r, resp) = ui.allocate_exact_size(vec2(row_w, 24.0), Sense::click());
    crate::widgets::announce_selected(&resp, shown.clone(), active);
    register(ui, format!("irAeroProfile:{pid}"), r);
    let fill = if active {
        t.tool_active
    } else if resp.hovered() {
        t.hover
    } else {
        t.inset
    };
    let p = ui.painter();
    p.rect(r, CornerRadius::same(4), fill, Stroke::new(1.0, if active { t.accent } else { t.button_border }), StrokeKind::Inside);
    let text_c = if active { t.text } else { t.text_label };
    let galley = one_line(p, &shown, t.font(12.0), text_c, (row_w - 14.0).max(16.0));
    let elided = galley.elided;
    p.galley(pos2(r.left() + 7.0, r.center().y - galley.size().y / 2.0), galley, text_c);
    if resp.hovered() && !active {
        let mut s = d.clone();
        s.profile.id = pid.to_string();
        s.profile.amount = 100.0;
        reset_swap_preview(&mut s);
        app.hover_preview = Some(HoverPreview { label: crate::i18n::tr_format!("Profile: {}", crate::i18n::profile_label(pid, label)), settings: s });
    }
    let resp = if elided { resp.on_hover_text(shown) } else { resp };
    if resp.clicked() && !active {
        let _ = app.run("develop.profile", json!({"id": pid}));
    }
}

fn aerochrome_section(app: &mut LightcraftApp, ui: &mut egui::Ui, d: &DevelopSettings, id: PhotoId, raw: bool) {
    let t = Tokens::get(ui.ctx());
    let chrome_dcps: Vec<(String, String)> = app
        .session
        .imported_profiles_for(Some(id))
        .into_iter()
        .filter(|(pid, name, _)| is_dcp_id(pid) && is_chrome_profile(pid, name))
        .map(|(pid, name, _)| (pid, name))
        .collect();
    let is_raw = app.session.catalog.photo(id).is_some_and(|p| p.kind == MediaKind::Raw);

    sub_title(ui, "Chrome Profiles");
    egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 6 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        let color_label = crate::i18n::tr("Color").to_string();
        chrome_profile_row(app, ui, d, "lc.color", &color_label);
        for (pid, name) in &chrome_dcps {
            chrome_profile_row(app, ui, d, pid, name);
        }
        if is_raw && chrome_dcps.is_empty() {
            ui.add_space(2.0);
            ui.label(egui::RichText::new(crate::i18n::tr("No Chrome camera profiles for this camera yet.")).size(11.5).color(t.text_dim));
        }
        ui.add_space(2.0);
        if text_button(ui, "irAeroImportDcp", "Import DCP…", false).clicked() {
            let _ = app.run("file.importPresets", json!({}));
        }
    });
    if d.profile.id != "lc.color" {
        control(app, ui, d, "profile.amount", true);
        ui.add_space(4.0);
    }

    sub_title(ui, "White Balance");
    picker_bar(app, ui, "irAeroWbPicker", "irAeroMixerPicker", "irAeroResetColor", "color");
    wb_controls(app, ui, d, raw);

    sub_title(ui, "Color Grading");
    control(app, ui, d, "color.vibrance", true);
    control(app, ui, d, "color.saturation", true);
    ui.add_space(4.0);

    let hsl_id = egui::Id::unique("irAeroHslOpen");
    let hsl_open: bool = ui.data(|m| m.get_temp(hsl_id)).unwrap_or(false);
    if flyout_row(ui, "irAeroHsl", "Foliage & Sky HSL", Icon::Radial, hsl_open).clicked() {
        ui.data_mut(|m| m.insert_temp(hsl_id, !hsl_open));
    }
    if hsl_open {
        for (title, band) in [("Red", "red"), ("Orange", "orange"), ("Yellow", "yellow"), ("Blue", "blue")] {
            sub_title(ui, title);
            for attr in ["hue", "sat", "lum"] {
                control(app, ui, d, &format!("mixer.{band}.{attr}"), true);
            }
        }
    }

    let calib_id = egui::Id::unique("irAeroCalibOpen");
    let calib_open: bool = ui.data(|m| m.get_temp(calib_id)).unwrap_or(false);
    if flyout_row(ui, "irAeroCalib", "Primary Calibration", Icon::Sliders, calib_open).clicked() {
        ui.data_mut(|m| m.insert_temp(calib_id, !calib_open));
    }
    if calib_open {
        for (title, k) in [("Red Primary", "red"), ("Green Primary", "green"), ("Blue Primary", "blue")] {
            sub_title(ui, title);
            control(app, ui, d, &format!("calibration.{k}Hue"), true);
            control(app, ui, d, &format!("calibration.{k}Sat"), true);
        }
    }
    ui.add_space(8.0);
}

/// Dedicated infrared workflow sections appended inside the `Edit` panel (`RightPanel::Edit`).
pub fn sections(app: &mut LightcraftApp, ui: &mut egui::Ui, d: &DevelopSettings, id: PhotoId, raw: bool) {
    ir_section(app, ui, "irFalseColor", "IR-FALSE-COLOR", |app, ui| false_color_section(app, ui, d, raw));
    ir_section(app, ui, "irMono", "IR-MONO", |app, ui| mono_section(app, ui, d));
    ir_section(app, ui, "irAerochrome", "AEROCHROME", |app, ui| aerochrome_section(app, ui, d, id, raw));
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lightcraft_catalog::{MediaKind, Op, Photo, Source};
    use lightcraft_engine::Session;
    use serde_json::json;

    use crate::headless::Headless;
    use crate::state::{RIGHT_WIDTH, RightPanel, ViewMode};
    use crate::{LightcraftApp, Services};

    const T: Duration = Duration::from_secs(20);
    const SETTLE: Duration = Duration::from_secs(60);

    fn minimal_dcp(name: &str, camera: &str) -> Vec<u8> {
        let mut cam = camera.as_bytes().to_vec();
        cam.push(0);
        let mut pname = name.as_bytes().to_vec();
        pname.push(0);

        let ifd_offset = 8usize;
        let entries = 4usize;
        let data_start = ifd_offset + 2 + entries * 12 + 4;
        let mut out = vec![0u8; data_start];
        out[0..4].copy_from_slice(b"IIRC");
        out[4..8].copy_from_slice(&(ifd_offset as u32).to_le_bytes());
        out[ifd_offset..ifd_offset + 2].copy_from_slice(&(entries as u16).to_le_bytes());

        fn push_entry(out: &mut [u8], ifd_offset: usize, idx: usize, tag: u16, typ: u16, count: u32, val: [u8; 4]) {
            let p = ifd_offset + 2 + idx * 12;
            out[p..p + 2].copy_from_slice(&tag.to_le_bytes());
            out[p + 2..p + 4].copy_from_slice(&typ.to_le_bytes());
            out[p + 4..p + 8].copy_from_slice(&count.to_le_bytes());
            out[p + 8..p + 12].copy_from_slice(&val);
        }

        let cam_off = out.len() as u32;
        out.extend_from_slice(&cam);
        if cam.len() <= 4 {
            let mut inline = [0u8; 4];
            inline[..cam.len()].copy_from_slice(&cam);
            push_entry(&mut out, ifd_offset, 0, 0xC614, 2, cam.len() as u32, inline);
        } else {
            push_entry(&mut out, ifd_offset, 0, 0xC614, 2, cam.len() as u32, cam_off.to_le_bytes());
        }

        let mat_off = out.len() as u32;
        let identity = [1000i32, 1000, 0, 1000, 0, 1000, 0, 1000, 1000, 1000, 0, 1000, 0, 1000, 0, 1000, 1000, 1000];
        for v in identity {
            out.extend_from_slice(&v.to_le_bytes());
        }
        push_entry(&mut out, ifd_offset, 1, 0xC621, 10, 9, mat_off.to_le_bytes());
        push_entry(&mut out, ifd_offset, 2, 0xC65A, 3, 1, [21, 0, 0, 0]);

        let name_off = out.len() as u32;
        out.extend_from_slice(&pname);
        if pname.len() <= 4 {
            let mut inline = [0u8; 4];
            inline[..pname.len()].copy_from_slice(&pname);
            push_entry(&mut out, ifd_offset, 3, 0xC6F8, 2, pname.len() as u32, inline);
        } else {
            push_entry(&mut out, ifd_offset, 3, 0xC6F8, 2, pname.len() as u32, name_off.to_le_bytes());
        }
        out
    }

    fn app_with_nikon_raw_and_dcps(tag: &str) -> (Headless, String, String, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("lc-ir-edit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let ir_dcp_path = dir.join("zf-ir100.dcp");
        let chrome_dcp_path = dir.join("zf-chrome.dcp");
        std::fs::write(&ir_dcp_path, minimal_dcp("Nikon Z f Infrared -100", "Nikon Z f")).unwrap();
        std::fs::write(&chrome_dcp_path, minimal_dcp("Nikon Z f Chrome", "Nikon Z f")).unwrap();

        let mut s = Session::new().with_fs();
        s.open_library(dir.join("lib"), false).unwrap();
        let pid = s.catalog.alloc_photo_id();
        let mut ph =
            Photo::new(pid, Source::File { path: dir.join("DSC_0001.NEF").to_string_lossy().into() }, "DSC_0001.NEF", "NEF", 600, 400, "2026-11-27");
        ph.kind = MediaKind::Raw;
        ph.meta.make = "NIKON CORPORATION".into();
        ph.meta.camera = "NIKON CORPORATION NIKON Z f".into();
        s.catalog.apply(Op::AddPhoto { photo: Box::new(ph) }).unwrap();

        let ir_r = s.execute("profile.import", &json!({"paths": [ir_dcp_path.to_string_lossy()]})).unwrap();
        let ir_id = ir_r["imported"][0]["id"].as_str().unwrap().to_string();
        let chrome_r = s.execute("profile.import", &json!({"paths": [chrome_dcp_path.to_string_lossy()]})).unwrap();
        let chrome_id = chrome_r["imported"][0]["id"].as_str().unwrap().to_string();
        s.execute("library.select", &json!({"ids": [pid.0]})).unwrap();

        let mut app = LightcraftApp::new(s, Services { png: None, ..Default::default() });
        app.ui.view = ViewMode::Detail;
        app.ui.right = RightPanel::Edit;
        app.ui.ir_tab = false;
        app.ui.histogram = false;
        app.ui.open_sections.clear();
        let mut h = Headless::new(app, [1200.0, 1800.0], 1.0);
        h.settle(SETTLE);
        (h, ir_id, chrome_id, dir)
    }

    fn widget(h: &Headless, id: &str) -> Option<egui::Rect> {
        h.app.widgets.iter().rev().find(|(w, _)| w == id).map(|(_, r)| *r)
    }

    fn assert_fits(h: &Headless, panel: egui::Rect, ids: &[&str]) {
        for id in ids {
            let r = widget(h, id).unwrap_or_else(|| panic!("missing {id}"));
            assert!(r.left() >= panel.left() - 0.5 && r.right() <= panel.right() + 0.5, "{id} {r:?} overflows {panel:?}");
        }
    }

    #[test]
    fn ir_false_color_and_ir_mono_sections_fit_and_apply_mixes() {
        let (mut h, _, _, dir) = app_with_nikon_raw_and_dcps("fc-mono");
        assert!(widget(&h, "section:irFalseColor").is_some());
        assert!(widget(&h, "section:irMono").is_some());
        assert!(widget(&h, "section:irAerochrome").is_some());

        h.request("ui.set", json!({"rightWidth": RIGHT_WIDTH.min}), T);
        h.settle(SETTLE);
        let panel = widget(&h, "panel:right_panel").expect("right panel");
        assert!((panel.width() - RIGHT_WIDTH.min).abs() < 1.0, "{panel:?}");

        h.app.run("section.irFalseColor", json!({"solo": true})).unwrap();
        h.settle(SETTLE);
        assert!(widget(&h, "irFcMix:own-swap-amber").is_some());
        assert!(widget(&h, "irFcMix:course-mono-swap").is_none());
        assert_fits(
            &h,
            panel,
            &[
                "icon:irFcWbPicker",
                "icon:irFcMixerPicker",
                "button:irFcResetMixer",
                "button:irFcSwapRB",
                "button:irFcFilter:all",
                "button:irFcFilter:builtin",
                "irFcMix:own-swap-amber",
            ],
        );
        assert_eq!(h.request("ui.clickWidget", json!({"id": "button:irFcFilter:swap"}), T)["ok"], true);
        h.settle(SETTLE);
        assert_eq!(h.request("ui.clickWidget", json!({"id": "irFcMix:own-swap-amber"}), T)["ok"], true);
        h.settle(SETTLE);
        let pid = h.app.session.active().unwrap();
        assert_eq!(h.app.session.develop_of(pid).unwrap().ir_mix.as_ref().map(|m| m.mix.as_str()), Some("own-swap-amber"));

        h.app.run("section.irMono", json!({"solo": true})).unwrap();
        h.settle(SETTLE);
        assert!(widget(&h, "irMonoMix:monoRed").is_some());
        assert!(widget(&h, "irMonoMix:course-mono-swap").is_some());
        assert!(widget(&h, "irMonoMix:own-swap-amber").is_none());
        assert_fits(
            &h,
            panel,
            &["icon:irMonoWbPicker", "icon:irMonoMixerPicker", "button:irMonoResetMixer", "irMonoMix:course-mono-swap", "slider:light.exposure"],
        );
        assert_eq!(h.request("ui.clickWidget", json!({"id": "irMonoMix:course-mono-swap"}), T)["ok"], true);
        h.settle(SETTLE);
        assert_eq!(h.app.session.develop_of(pid).unwrap().ir_mix.as_ref().map(|m| m.mix.as_str()), Some("course-mono-swap"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn aerochrome_section_filters_chrome_dcps_and_resets_swap_on_hover_and_click() {
        let (mut h, ir_id, chrome_id, dir) = app_with_nikon_raw_and_dcps("aero");
        h.request("ui.set", json!({"rightWidth": RIGHT_WIDTH.min}), T);
        h.settle(SETTLE);
        let panel = widget(&h, "panel:right_panel").expect("right panel");

        h.app.run("develop.mixerSwapRBsRGB", json!({})).unwrap();
        h.app.run("section.irAerochrome", json!({"solo": true})).unwrap();
        h.settle(SETTLE);
        let chrome_widget = format!("irAeroProfile:{chrome_id}");
        let non_chrome_widget = format!("irAeroProfile:{ir_id}");
        assert!(widget(&h, &chrome_widget).is_some(), "missing Chrome DCP");
        assert!(widget(&h, &non_chrome_widget).is_none(), "non-Chrome DCP must not appear in AEROCHROME");

        assert_eq!(h.request("ui.hoverWidget", json!({"id": chrome_widget}), T)["ok"], true);
        let preview = h.app.hover_preview.as_ref().expect("hover preview");
        assert_eq!(preview.settings.profile.id, chrome_id);
        assert!(preview.settings.ir_mix.is_none());
        assert!(preview.settings.channel_mixer.is_identity());

        assert_eq!(h.request("ui.clickWidget", json!({"id": chrome_widget}), T)["ok"], true);
        h.settle(SETTLE);
        let pid = h.app.session.active().unwrap();
        let d = h.app.session.develop_of(pid).unwrap();
        assert_eq!(d.profile.id, chrome_id);
        assert!(d.ir_mix.is_none(), "swap must auto-reset on profile change");
        assert!(d.channel_mixer.is_identity(), "channel mixer must auto-reset on profile change");

        assert_eq!(h.request("ui.clickWidget", json!({"id": "flyout:irAeroCalib"}), T)["ok"], true);
        assert_eq!(h.request("ui.clickWidget", json!({"id": "flyout:irAeroHsl"}), T)["ok"], true);
        h.settle(SETTLE);
        assert_fits(
            &h,
            panel,
            &["irAeroProfile:lc.color", "button:irAeroImportDcp", "slider:profile.amount", "flyout:irAeroHsl", "flyout:irAeroCalib"],
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
