//! Fork (irchrome/lightcraft, infrared): the Channel Mixer's infrared mixes, drawn under upstream's
//! picker and swap, plus the extended clean-room catalog in the Infrared tab. One call from `edit.rs`
//! keeps the fork's footprint in upstream files small.

use egui::{CornerRadius, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use lightcraft_develop::DevelopSettings;
use lightcraft_pipeline::ir_swap::catalog::{HUB_IR_MIXES, HubIrMixSpec};
use lightcraft_pipeline::ir_swap::{IrMix, active_hub_ir_mix, choose_hub_ir_mix};
use serde_json::json;

use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{flyout_row, one_line, register, text_button};
use crate::{HoverPreview, LightcraftApp};

const FAMILIES: &[(&str, &str)] = &[
    ("all", "All"),
    ("swap", "Swaps"),
    ("swap-variant", "Swap Variants"),
    ("mono", "Monochrome"),
    ("creative", "Creative"),
    ("fit", "Fitted Looks"),
];

/// Run one fork mix command and say when the mix had to be scaled into ±200 % (the photo is darker).
fn run(app: &mut LightcraftApp, ui: &Ui, command: &str, params: serde_json::Value) {
    match app.run(command, params) {
        Ok(r) => {
            let darker = r["darker"].as_f64().unwrap_or(0.0);
            if darker >= 0.05 {
                app.toast(ui.ctx(), crate::i18n::tr_format!("Darker by {ev:.1} EV to fit the mix in range: raise Exposure if needed", ev = darker));
            }
        }
        Err(e) => app.toast(ui.ctx(), e),
    }
}

/// The sRGB swap, a toggle for the other infrared mixes, and those mixes when open (state per session). The IR mix
/// that is on is drawn selected: clicking it again turns it off, clicking another replaces it.
pub fn buttons(app: &mut LightcraftApp, ui: &mut Ui) {
    let open_id = egui::Id::unique("irMixesOpen");
    let open: bool = ui.data(|m| m.get_temp(open_id)).unwrap_or(false);
    let on = app.session.active().and_then(|id| app.session.develop_of(id)).and_then(|d| active_hub_ir_mix(&d));
    ui.horizontal_wrapped(|ui| {
        if text_button(ui, "mixerSwapRBsRGB", "Swap Red and Blue (sRGB)", on.is_some_and(|s| s.ir_mix_id == Some("swapRB"))).clicked() {
            run(app, ui, "develop.mixerSwapRBsRGB", json!({}));
        }
        if text_button(ui, "irMixes", "More IR mixes", open).clicked() {
            ui.data_mut(|m| m.insert_temp(open_id, !open));
        }
    });
    if open {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            // the sRGB red/blue swap has its own button above
            for (mix, id, label) in IrMix::ALL {
                if mix == IrMix::SwapRB {
                    continue;
                }
                if text_button(ui, &format!("irMix:{id}"), label, on.is_some_and(|s| s.ir_mix_id == Some(id))).clicked() {
                    run(app, ui, "develop.mixerIrMix", json!({"mix": id}));
                }
            }
        });
    }
}

fn catalog_row(app: &mut LightcraftApp, ui: &mut Ui, d: &DevelopSettings, spec: &HubIrMixSpec, widget_id: &str, active: bool) {
    let t = Tokens::get(ui.ctx());
    let row_w = ui.available_width().max(40.0);
    let (r, resp) = ui.allocate_exact_size(vec2(row_w, 24.0), Sense::click());
    crate::widgets::announce_selected(&resp, spec.name.to_string(), active);
    register(ui, widget_id, r);
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
    let galley = one_line(p, spec.name, t.font(12.0), text_c, (row_w - 14.0).max(16.0));
    let elided = galley.elided;
    p.galley(pos2(r.left() + 7.0, r.center().y - galley.size().y / 2.0), galley, text_c);
    if resp.hovered() && !active {
        let mut s = d.clone();
        if choose_hub_ir_mix(&mut s, spec).is_ok() {
            if !s.section_enabled("channelMixer") {
                s.set_section_enabled("channelMixer", true);
            }
            app.hover_preview = Some(HoverPreview { label: format!("IR Mix: {}", spec.name), settings: s });
        }
    }
    let resp = if elided { resp.on_hover_text(spec.name) } else { resp };
    if resp.clicked() {
        run(app, ui, "develop.mixerIrMix", json!({"mix": spec.id}));
    }
}

fn render_group(app: &mut LightcraftApp, ui: &mut Ui, d: &DevelopSettings, active_id: Option<&str>, heading: Option<&str>, specs: &[&HubIrMixSpec]) {
    if specs.is_empty() {
        return;
    }
    if let Some(title) = heading {
        ui.add_space(2.0);
        let t = Tokens::get(ui.ctx());
        ui.label(egui::RichText::new(crate::i18n::tr(title)).font(t.semibold(11.0)).color(t.text_dim));
    }
    for spec in specs {
        let active = active_id == Some(spec.id);
        catalog_row(app, ui, d, spec, &format!("irMix:{}", spec.id), active);
    }
}

/// Collapsible catalog of extended clean-room recipes from `HUB_IR_MIXES` with family filters and hover preview.
pub fn catalog_recipes(app: &mut LightcraftApp, ui: &mut Ui, d: &DevelopSettings) {
    let open_id = egui::Id::unique("irCatalogMixesOpen");
    let open: bool = ui.data(|m| m.get_temp(open_id)).unwrap_or(false);
    let active_spec = active_hub_ir_mix(d);
    let active_extended = active_spec.filter(|s| s.ir_mix_id.is_none());

    if flyout_row(ui, "irCatalogMixes", "IR Mix Catalog", Icon::Presets, open).clicked() {
        ui.data_mut(|m| m.insert_temp(open_id, !open));
    }
    if !open {
        if let Some(spec) = active_extended {
            egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 0, bottom: 4 }).show(ui, |ui| {
                catalog_row(app, ui, d, spec, "irMixActive", true);
            });
        }
        return;
    }

    let fam_id = egui::Id::unique("irCatalogFamily");
    let selected: &'static str = ui.data(|m| m.get_temp(fam_id)).unwrap_or("all");
    let active_id = active_extended.map(|s| s.id);

    egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 22, top: 2, bottom: 6 }).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (key, label) in FAMILIES {
                if text_button(ui, &format!("irCatalogFilter:{key}"), label, selected == *key).clicked() {
                    ui.data_mut(|m| m.insert_temp(fam_id, *key));
                }
            }
        });
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 3.0;
        if selected == "all" {
            for (key, label) in &FAMILIES[1..] {
                let group: Vec<&HubIrMixSpec> = HUB_IR_MIXES.iter().filter(|s| s.ir_mix_id.is_none() && s.family == *key).collect();
                render_group(app, ui, d, active_id, Some(label), &group);
            }
            let other: Vec<&HubIrMixSpec> =
                HUB_IR_MIXES.iter().filter(|s| s.ir_mix_id.is_none() && !FAMILIES[1..].iter().any(|(k, _)| *k == s.family)).collect();
            render_group(app, ui, d, active_id, Some("Other"), &other);
        } else {
            let filtered: Vec<&HubIrMixSpec> = HUB_IR_MIXES.iter().filter(|s| s.ir_mix_id.is_none() && s.family == selected).collect();
            render_group(app, ui, d, active_id, None, &filtered);
        }
    });
}
