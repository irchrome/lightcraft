//! Fork (irchrome/lightcraft, infrared): the Channel Mixer's infrared mixes, drawn under upstream's
//! picker and swap. One call from `edit.rs` keeps the fork's footprint in upstream files small.

use egui::Ui;
use lightcraft_pipeline::ir_swap::IrMix;
use serde_json::json;

use crate::LightcraftApp;
use crate::widgets::text_button;

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

/// The sRGB swap, a toggle for the other infrared mixes, and those mixes when open (state per session).
pub fn buttons(app: &mut LightcraftApp, ui: &mut Ui) {
    let open_id = egui::Id::unique("irMixesOpen");
    let open: bool = ui.data(|m| m.get_temp(open_id)).unwrap_or(false);
    ui.horizontal_wrapped(|ui| {
        if text_button(ui, "mixerSwapRBsRGB", "Swap Red and Blue (sRGB)", false).clicked() {
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
                if text_button(ui, &format!("irMix:{id}"), label, false).clicked() {
                    run(app, ui, "develop.mixerIrMix", json!({"mix": id}));
                }
            }
        });
    }
}
