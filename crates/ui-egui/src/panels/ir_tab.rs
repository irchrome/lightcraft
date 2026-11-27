//! Fork (irchrome/lightcraft, infrared): dedicated "Infrared" tab in the right-hand Tools panel,
//! layered over `RightPanel::Edit` via `UiState::ir_tab` (0 lines in `docking.rs` and `detail.rs`).
//!
//! Order strictly follows the infrared processing workflow:
//! 1. Camera Profile (camera-bound DCP quick list with live `HoverPreview` + `Amount`)
//! 2. White Balance & Color (`Temp`/`Tint`, `W` picker, compact `Exposure` / `Vibrance` / `Saturation`)
//! 3. Infrared Channel Mixer (neutral picker, `Swap Red and Blue (sRGB)`, `More IR mixes`,
//!    extended `IR Mix Catalog`, Reset, 3×3 sliders)

use egui::{CornerRadius, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use lightcraft_catalog::{MediaKind, PhotoId};
use lightcraft_develop::WbMode;
use lightcraft_engine::cmd::dcp_profiles::is_dcp_id;
use serde_json::json;

use super::edit::{
    REL_TEMP, REL_TINT, apply_slider_out, control, control_override, k_to_rel, profile_menu, rel_to_k, reload_preview_only, sub_title,
};
use super::right::fold_header;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{divider, dropdown, dropdown_within, flyout_row, icon_button, one_line, register, text_button};
use crate::{HoverPreview, LightcraftApp};

/// Segmented `[ Edit | Infrared ]` tab bar drawn at the top of `Tools` when `RightPanel::Edit` is active.
pub fn tab_bar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 8, bottom: 8 }).show(ui, |ui| {
        const GAP: f32 = 4.0;
        let w = ui.available_width();
        let tab_w = ((w - GAP) / 2.0).max(40.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for (widget_id, label, is_ir) in [("tab:edit", "Edit", false), ("tab:infrared", "Infrared", true)] {
                let active = app.ui.ir_tab == is_ir;
                let translated = crate::i18n::tr(label);
                let (r, resp) = ui.allocate_exact_size(vec2(tab_w, 26.0), Sense::click());
                crate::widgets::announce_selected(&resp, translated.to_string(), active);
                register(ui, widget_id, r);
                let fill = if active {
                    t.pressed
                } else if resp.hovered() {
                    t.hover
                } else {
                    t.button
                };
                let p = ui.painter();
                p.rect(r, CornerRadius::same(4), fill, Stroke::new(1.0, t.button_border), StrokeKind::Inside);
                if active {
                    let bar = Rect::from_min_max(pos2(r.left() + 6.0, r.bottom() - 2.0), pos2(r.right() - 6.0, r.bottom()));
                    p.rect_filled(bar, 1.0, t.accent);
                }
                let text_c = if active { t.text } else { t.text_label };
                let galley = one_line(p, translated, t.semibold(12.0), text_c, (tab_w - 12.0).max(16.0));
                p.galley(r.center() - galley.size() / 2.0, galley, text_c);
                if resp.clicked() {
                    app.ui.ir_tab = is_ir;
                }
            }
        });
    });
    divider(ui);
}

/// Dedicated infrared workflow controls inside `RightPanel::Edit` when `app.ui.ir_tab` is true.
pub fn show(app: &mut LightcraftApp, ui: &mut egui::Ui, id: PhotoId) {
    let t = Tokens::get(ui.ctx());
    let (d, raw) = super::edit::prepared_develop(app, id);
    let preview_only = app.session.catalog.photo(id).and_then(|p| p.preview_only.clone());

    if let Some(why) = &preview_only {
        let reload = egui::Frame::NONE
            .inner_margin(egui::Margin { left: 24, right: 22, top: 8, bottom: 8 })
            .show(ui, |ui| crate::widgets::preview_only_notice(ui, "ir", why))
            .inner;
        if reload {
            reload_preview_only(app, ui.ctx(), id);
        }
    }

    let n = app.session.selection.ids.len();
    if app.session.auto_sync && n > 1 {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 8, bottom: 4 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::tr_format!("Auto Sync: edits apply to {n} photos", n = n)).color(t.accent).size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if text_button(ui, "irAutoSyncOff", crate::i18n::tr("Turn Off"), false).clicked() {
                        let _ = app.run("develop.autoSync", json!({"on": false}));
                    }
                });
            });
        });
    }

    // 1. Camera Profile
    if fold_header(app, ui, "irProfile", "Camera Profile") {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 6 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::tr("Profile")).font(t.font(13.0)).color(t.text_dim));
                let name = app.session.profile_info(&d.profile.id).map(|(name, _)| name).unwrap_or("Color");
                let max = (ui.available_width() - 28.0 - ui.spacing().item_spacing.x).max(40.0);
                let r = dropdown_within(ui, "irProfile", crate::i18n::profile_label(&d.profile.id, name), t.font(14.0), t.text_label, max);
                let below = r.rect.bottom();
                egui::Popup::menu(&r).show(|ui| profile_menu(app, ui, &d, below));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, "irProfileBrowser", Icon::ProfileGrid, vec2(28.0, 28.0), false, true, "Browse Profiles").clicked() {
                        let _ = app.run("panel.profiles", json!({}));
                    }
                });
            });
        });

        let dcps: Vec<(String, String)> =
            app.session.imported_profiles_for(Some(id)).into_iter().filter(|(pid, _, _)| is_dcp_id(pid)).map(|(pid, name, _)| (pid, name)).collect();

        let mut quick: Vec<(String, String)> = vec![("lc.color".to_string(), crate::i18n::tr("Color").to_string())];
        quick.extend(dcps.iter().cloned());

        egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 0, bottom: 6 }).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            let row_w = ui.available_width();
            for (pid, label) in &quick {
                let active = d.profile.id == *pid;
                let fav = app.session.profile_favorites.iter().any(|f| f == pid);
                let shown = if fav { format!("★ {label}") } else { label.clone() };
                let (r, resp) = ui.allocate_exact_size(vec2(row_w, 24.0), Sense::click());
                crate::widgets::announce_selected(&resp, shown.clone(), active);
                register(ui, format!("irProfile:{pid}"), r);
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
                    let mut s = (*d).clone();
                    s.profile.id = pid.clone();
                    s.profile.amount = 100.0;
                    super::ir_mix::reset_swap_preview(&mut s);
                    app.hover_preview =
                        Some(HoverPreview { label: crate::i18n::tr_format!("Profile: {}", crate::i18n::profile_label(pid, label)), settings: s });
                }
                let resp = if elided { resp.on_hover_text(shown) } else { resp };
                if resp.clicked() && !active {
                    let _ = app.run("develop.profile", json!({"id": pid}));
                }
            }
        });

        if d.profile.id != "lc.color" {
            control(app, ui, &d, "profile.amount", true);
            ui.add_space(4.0);
        }

        let photo = app.session.catalog.photo(id);
        let is_raw = photo.is_some_and(|p| p.kind == MediaKind::Raw);
        let camera_empty = photo.is_some_and(|p| p.meta.camera.trim().is_empty());

        egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 8 }).show(ui, |ui| {
            if is_raw && camera_empty && preview_only.is_none() {
                ui.label(
                    egui::RichText::new(crate::i18n::tr("Camera unknown — reload from disk to match DCP profiles.")).size(11.5).color(t.text_dim),
                );
                ui.add_space(4.0);
                if text_button(ui, "irReloadFromDisk", "Reload from Disk", false).clicked() {
                    reload_preview_only(app, ui.ctx(), id);
                }
                ui.add_space(4.0);
            } else if is_raw && dcps.is_empty() {
                ui.label(egui::RichText::new(crate::i18n::tr("No camera profiles imported for this camera yet.")).size(11.5).color(t.text_dim));
                ui.add_space(4.0);
            }
            if text_button(ui, "irImportDcp", "Import DCP…", false).clicked() {
                let _ = app.run("file.importPresets", json!({}));
            }
        });
    }
    divider(ui);

    // 2. White Balance & Color (+ Exposure / Vibrance / Saturation per B13=A)
    if fold_header(app, ui, "irWb", "White Balance & Color") {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 2 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::tr("White Balance")).font(t.font(13.0)).color(t.text_dim));
                let r = dropdown(ui, "irWbMode", crate::i18n::tr(d.wb.mode.label()), t.font(14.0), t.text_label);
                egui::Popup::menu(&r).show(|ui| {
                    for m in WbMode::ALL {
                        if m == WbMode::Custom {
                            continue;
                        }
                        if ui.selectable_label(d.wb.mode == m, crate::i18n::tr(m.label())).clicked() {
                            let mode = serde_json::to_value(m).unwrap_or_default();
                            let _ = app.run("develop.wb", json!({"mode": mode}));
                        }
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let active = app.ui.tool == "wbPicker";
                    if icon_button(ui, "wbPicker", Icon::Picker, vec2(28.0, 28.0), active, true, "White Balance Selector (W)").clicked() {
                        if active {
                            app.ui.tool.clear();
                        } else if let Err(e) = app.run("tool.wbPicker", json!({})) {
                            app.toast(ui.ctx(), e);
                        }
                    }
                });
            });
        });
        if raw {
            control(app, ui, &d, "wb.temp", true);
            control(app, ui, &d, "wb.tint", true);
        } else {
            let out = crate::widgets::slider(ui, &REL_TEMP, k_to_rel(d.wb.temp), true, None);
            apply_slider_out(app, &REL_TEMP, out, |app, v| app.run("develop.set", json!({"control": "wb.temp", "value": rel_to_k(v)})));
            let out = crate::widgets::slider(ui, &REL_TINT, d.wb.tint.clamp(-100.0, 100.0), true, None);
            apply_slider_out(app, &REL_TINT, out, |app, v| app.run("develop.set", json!({"control": "wb.tint", "value": v})));
        }
        control(app, ui, &d, "light.exposure", true);
        control(app, ui, &d, "color.vibrance", true);
        control(app, ui, &d, "color.saturation", true);
        ui.add_space(6.0);
    }
    divider(ui);

    // 3. Infrared Channel Mixer
    if fold_header(app, ui, "irMixer", "Infrared Channel Mixer") {
        let mixer_on = d.section_enabled("channelMixer");
        egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 6 }).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let active = app.ui.tool == "mixerPicker";
                if icon_button(
                    ui,
                    "mixerPicker",
                    Icon::Picker,
                    vec2(28.0, 28.0),
                    active,
                    true,
                    "Click a spot that should be neutral (for infrared: foliage)",
                )
                .clicked()
                {
                    if active {
                        app.ui.tool.clear();
                    } else if let Err(e) = app.run("tool.mixerPicker", json!({})) {
                        app.toast(ui.ctx(), e);
                    }
                }
                if icon_button(ui, "irMixerEnabled", Icon::Eye, vec2(28.0, 28.0), mixer_on, true, "Show / hide").clicked() {
                    let _ = app.run("develop.sectionEnabled", json!({"section": "channelMixer", "enabled": !mixer_on}));
                }
                if text_button(ui, "irResetMixer", "Reset", false).clicked() {
                    let _ = app.run("develop.resetSection", json!({"section": "channelMixer"}));
                }
            });
            super::ir_mix::buttons(app, ui);
        });

        super::ir_mix::catalog_recipes(app, ui, &d);

        let sliders_id = egui::Id::unique("irChannelSlidersOpen");
        let sliders_open: bool = ui.data(|m| m.get_temp(sliders_id)).unwrap_or(false);
        if flyout_row(ui, "irChannelSliders", "Channel Sliders", Icon::Sliders, sliders_open).clicked() {
            ui.data_mut(|m| m.insert_temp(sliders_id, !sliders_open));
        }
        if sliders_open {
            for (title, k) in [("Red Output", "red"), ("Green Output", "green"), ("Blue Output", "blue")] {
                sub_title(ui, title);
                for (name, src) in [("Red", "red"), ("Green", "green"), ("Blue", "blue")] {
                    control_override(app, ui, &d, &format!("channelMixer.{k}.{src}"), true, Some(name));
                }
            }
        }
        ui.add_space(8.0);
    }
    divider(ui);
    ui.add_space(40.0);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lightcraft_catalog::{MediaKind, Op, Photo, Source};
    use lightcraft_engine::Session;
    use serde_json::json;

    use crate::headless::Headless;
    use crate::state::{RIGHT_WIDTH, RightPanel, UiState, ViewMode};
    use crate::{LightcraftApp, Services};

    const T: Duration = Duration::from_secs(20);
    const SETTLE: Duration = Duration::from_secs(60);

    /// Minimal valid `.dcp` bytes (LE TIFF with `RC` magic, UniqueCameraModel, ColorMatrix1,
    /// CalibrationIlluminant1, ProfileName) so tests exercise real `profile.import` & `develop.profile`.
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

    fn app_with_nikon_raw_and_dcp(tag: &str) -> (Headless, String, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("lc-ir-tab-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let dcp_path = dir.join("zf-590.dcp");
        std::fs::write(&dcp_path, minimal_dcp("Zf IR 590nm Very Long Name For Narrow Panel Check", "Nikon Z f")).unwrap();

        let mut s = Session::new().with_fs();
        s.open_library(dir.join("lib"), false).unwrap();
        let pid = s.catalog.alloc_photo_id();
        let mut ph =
            Photo::new(pid, Source::File { path: dir.join("DSC_0001.NEF").to_string_lossy().into() }, "DSC_0001.NEF", "NEF", 600, 400, "2026-11-25");
        ph.kind = MediaKind::Raw;
        ph.meta.make = "NIKON CORPORATION".into();
        ph.meta.camera = "NIKON CORPORATION NIKON Z f".into();
        s.catalog.apply(Op::AddPhoto { photo: Box::new(ph) }).unwrap();

        let dcp_r = s.execute("profile.import", &json!({"paths": [dcp_path.to_string_lossy()]})).unwrap();
        let dcp_id = dcp_r["imported"][0]["id"].as_str().unwrap().to_string();
        s.execute("library.select", &json!({"ids": [pid.0]})).unwrap();

        let mut app = LightcraftApp::new(s, Services { png: None, ..Default::default() });
        app.ui.view = ViewMode::Detail;
        app.ui.right = RightPanel::Edit;
        let mut h = Headless::new(app, [1200.0, 1200.0], 1.0);
        h.settle(SETTLE);
        (h, dcp_id, dir)
    }

    fn widget(h: &Headless, id: &str) -> Option<egui::Rect> {
        h.app.widgets.iter().rev().find(|(w, _)| w == id).map(|(_, r)| *r)
    }

    #[test]
    fn switching_edit_and_infrared_tabs_via_tab_bar_commands_and_serde() {
        let (mut h, _, dir) = app_with_nikon_raw_and_dcp("switch");
        assert!(!h.app.ui.ir_tab);
        assert!(widget(&h, "tab:edit").is_some());
        assert!(widget(&h, "tab:infrared").is_some());

        // Click Infrared tab
        assert_eq!(h.request("ui.clickWidget", json!({"id": "tab:infrared"}), T)["ok"], true);
        h.settle(SETTLE);
        assert!(h.app.ui.ir_tab);
        assert!(widget(&h, "button:irImportDcp").is_some());

        // Click Edit tab back
        assert_eq!(h.request("ui.clickWidget", json!({"id": "tab:edit"}), T)["ok"], true);
        h.settle(SETTLE);
        assert!(!h.app.ui.ir_tab);
        assert!(widget(&h, "button:irImportDcp").is_none());

        // Strip / menu commands
        h.app.run("panel.infrared", json!({})).unwrap();
        assert_eq!((h.app.ui.right, h.app.ui.ir_tab), (RightPanel::Edit, true));
        assert_eq!(crate::menubar::checked(&h.app, "panel.infrared"), Some(true));
        assert_eq!(crate::menubar::checked(&h.app, "panel.edit"), Some(false));

        h.app.run("panel.edit", json!({})).unwrap();
        assert_eq!((h.app.ui.right, h.app.ui.ir_tab), (RightPanel::Edit, false));
        assert_eq!(crate::menubar::checked(&h.app, "panel.edit"), Some(true));

        // UiState JSON round-trip preserves ir_tab and remains backward-compatible
        let saved = serde_json::to_string(&UiState { ir_tab: true, ..Default::default() }).unwrap();
        assert!(serde_json::from_str::<UiState>(&saved).unwrap().ir_tab);
        assert!(!serde_json::from_str::<UiState>("{}").unwrap().ir_tab);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pickers_and_browse_profiles_preserve_infrared_tab() {
        let (mut h, _, dir) = app_with_nikon_raw_and_dcp("pickers");
        h.app.run("panel.infrared", json!({})).unwrap();
        h.settle(SETTLE);

        // Pickers stay on Infrared
        h.app.run("tool.wbPicker", json!({})).unwrap();
        assert_eq!((h.app.ui.right, h.app.ui.ir_tab, h.app.ui.tool.as_str()), (RightPanel::Edit, true, "wbPicker"));
        h.app.run("tool.mixerPicker", json!({})).unwrap();
        assert_eq!((h.app.ui.right, h.app.ui.ir_tab, h.app.ui.tool.as_str()), (RightPanel::Edit, true, "mixerPicker"));

        // Browse Profiles -> Back returns to Infrared
        assert_eq!(h.request("ui.clickWidget", json!({"id": "icon:irProfileBrowser"}), T)["ok"], true);
        h.settle(SETTLE);
        assert_eq!(h.app.ui.right, RightPanel::Profiles);
        assert_eq!(h.request("ui.clickWidget", json!({"id": "icon:profilesBack"}), T)["ok"], true);
        h.settle(SETTLE);
        assert_eq!((h.app.ui.right, h.app.ui.ir_tab), (RightPanel::Edit, true));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quick_dcp_hover_click_and_mixer_reset_work() {
        let (mut h, dcp_id, dir) = app_with_nikon_raw_and_dcp("dcp-reset");
        h.app.run("panel.infrared", json!({})).unwrap();
        h.settle(SETTLE);

        let item_id = format!("irProfile:{dcp_id}");
        assert_eq!(h.request("ui.hoverWidget", json!({"id": item_id}), T)["ok"], true);
        assert!(h.app.hover_preview.as_ref().is_some_and(|p| p.settings.profile.id == dcp_id));

        assert_eq!(h.request("ui.clickWidget", json!({"id": item_id}), T)["ok"], true);
        h.settle(SETTLE);
        let id = h.app.session.active().unwrap();
        assert_eq!(h.app.session.develop_of(id).unwrap().profile.id, dcp_id);

        // Apply sRGB swap then Reset via button:irResetMixer
        assert_eq!(h.request("ui.clickWidget", json!({"id": "button:mixerSwapRBsRGB"}), T)["ok"], true);
        h.settle(SETTLE);
        assert!(h.app.session.develop_of(id).unwrap().ir_mix.is_some());

        assert_eq!(h.request("ui.clickWidget", json!({"id": "button:irResetMixer"}), T)["ok"], true);
        h.settle(SETTLE);
        assert!(h.app.session.develop_of(id).unwrap().ir_mix.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn infrared_tab_fits_minimum_right_panel_width() {
        let (mut h, dcp_id, dir) = app_with_nikon_raw_and_dcp("narrow");
        h.app.run("panel.infrared", json!({})).unwrap();
        h.request("ui.set", json!({"rightWidth": RIGHT_WIDTH.min}), T);
        h.settle(SETTLE);
        // Collapse top sections so Channel Sliders expand well inside the scroll viewport
        h.app.ui.folded_sections.extend(["irProfile".to_string(), "irWb".to_string()]);
        h.settle(SETTLE);
        assert_eq!(h.request("ui.clickWidget", json!({"id": "flyout:irChannelSliders"}), T)["ok"], true);
        h.settle(SETTLE);

        let panel = widget(&h, "panel:right_panel").expect("right panel");
        assert!((panel.width() - RIGHT_WIDTH.min).abs() < 1.0, "{panel:?}");
        for id in [
            "tab:edit",
            "tab:infrared",
            "icon:mixerPicker",
            "button:irResetMixer",
            "button:mixerSwapRBsRGB",
            "button:irMixes",
            "flyout:irCatalogMixes",
            "flyout:irChannelSliders",
            "slider:channelMixer.red.red",
            "slider:channelMixer.blue.blue",
        ] {
            let r = widget(&h, id).unwrap_or_else(|| panic!("missing widget {id}"));
            assert!(r.left() >= panel.left() - 0.5 && r.right() <= panel.right() + 0.5, "{id} {r:?} overflows {panel:?}");
        }
        assert_eq!(h.request("ui.clickWidget", json!({"id": "flyout:irCatalogMixes"}), T)["ok"], true);
        h.settle(SETTLE);
        for id in [
            "button:irCatalogFilter:all",
            "button:irCatalogFilter:swap",
            "button:irCatalogFilter:swap-variant",
            "button:irCatalogFilter:mono",
            "button:irCatalogFilter:creative",
            "button:irCatalogFilter:fit",
            "irMix:uvf-cyclic-sub-g40-b30",
        ] {
            let r = widget(&h, id).unwrap_or_else(|| panic!("missing widget {id}"));
            assert!(r.left() >= panel.left() - 0.5 && r.right() <= panel.right() + 0.5, "{id} {r:?} overflows {panel:?}");
        }
        // Unfold top sections and verify DCP/WB widgets at minimum width too
        h.app.ui.folded_sections.clear();
        h.settle(SETTLE);
        for id in [
            "dropdown:irProfile",
            "icon:irProfileBrowser",
            &format!("irProfile:{dcp_id}"),
            "button:irImportDcp",
            "icon:wbPicker",
            "slider:light.exposure",
            "slider:color.vibrance",
            "slider:color.saturation",
        ] {
            let r = widget(&h, id).unwrap_or_else(|| panic!("missing widget {id}"));
            assert!(r.left() >= panel.left() - 0.5 && r.right() <= panel.right() + 0.5, "{id} {r:?} overflows {panel:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hub_ir_mix_catalog_lists_filters_hovers_applies_and_toggles() {
        let (mut h, _, dir) = app_with_nikon_raw_and_dcp("catalog");
        h.app.run("panel.infrared", json!({})).unwrap();
        h.app.ui.folded_sections.extend(["irProfile".to_string(), "irWb".to_string()]);
        h.settle(SETTLE);

        assert_eq!(h.request("ui.clickWidget", json!({"id": "flyout:irCatalogMixes"}), T)["ok"], true);
        h.settle(SETTLE);
        for id in ["irMix:own-swap-amber", "irMix:course-mono-swap", "irMix:uvf-fit-gold-foliage-blue-sky"] {
            assert!(widget(&h, id).is_some(), "missing {id}");
        }

        // Filter to monochrome
        assert_eq!(h.request("ui.clickWidget", json!({"id": "button:irCatalogFilter:mono"}), T)["ok"], true);
        h.settle(SETTLE);
        assert!(widget(&h, "irMix:course-mono-swap").is_some());
        assert!(widget(&h, "irMix:own-swap-amber").is_none());

        // Back to all
        assert_eq!(h.request("ui.clickWidget", json!({"id": "button:irCatalogFilter:all"}), T)["ok"], true);
        h.settle(SETTLE);
        assert!(widget(&h, "irMix:own-swap-amber").is_some());

        // Hover preview on extended recipe
        assert_eq!(h.request("ui.hoverWidget", json!({"id": "irMix:own-swap-amber"}), T)["ok"], true);
        assert_eq!(h.app.hover_preview.as_ref().and_then(|p| p.settings.ir_mix.as_ref()).map(|m| m.mix.as_str()), Some("own-swap-amber"));

        // Apply extended recipe
        let pid = h.app.session.active().unwrap();
        assert_eq!(h.request("ui.clickWidget", json!({"id": "irMix:own-swap-amber"}), T)["ok"], true);
        h.settle(SETTLE);
        let d = h.app.session.develop_of(pid).unwrap();
        assert_eq!(d.ir_mix.as_ref().map(|m| m.mix.as_str()), Some("own-swap-amber"));
        assert!(!d.channel_mixer.is_identity());

        // Filter to Fitted Looks and replace with another extended recipe (never stacks)
        assert_eq!(h.request("ui.clickWidget", json!({"id": "button:irCatalogFilter:fit"}), T)["ok"], true);
        h.settle(SETTLE);
        assert_eq!(h.request("ui.clickWidget", json!({"id": "irMix:uvf-fit-gold-foliage-blue-sky"}), T)["ok"], true);
        h.settle(SETTLE);
        let d = h.app.session.develop_of(pid).unwrap();
        assert_eq!(d.ir_mix.as_ref().map(|m| m.mix.as_str()), Some("uvf-fit-gold-foliage-blue-sky"));

        // Click active recipe again to toggle off back to identity
        assert_eq!(h.request("ui.clickWidget", json!({"id": "irMix:uvf-fit-gold-foliage-blue-sky"}), T)["ok"], true);
        h.settle(SETTLE);
        let d = h.app.session.develop_of(pid).unwrap();
        assert!(d.ir_mix.is_none() && d.channel_mixer.is_identity());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
