//! IR fork: what the About dialog says about this build (fork-owned file; one call in `dialogs.rs`).

use crate::LightcraftApp;
use crate::theme::Tokens;

/// The fork's public repository.
pub const FORK_GITHUB: &str = "https://github.com/irchrome/lightcraft";

/// Shown under the version line: this build is the infrared fork, what it adds, and where its source lives.
pub fn about_ui(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(crate::i18n::tr("LightCraft IR — a fork for infrared photography.")).font(t.semibold(15.0)).color(t.accent));
    ui.label(crate::i18n::tr(
        "Adds red/blue swaps and infrared channel mixes in sRGB, camera profiles bound to their camera, and raw data from Nikon High Efficiency NEF files.",
    ));
    let r = ui.link(crate::i18n::tr("IR fork on GitHub")).on_hover_text(FORK_GITHUB);
    if r.clicked() {
        let _ = crate::links::open(app, FORK_GITHUB);
    }
    ui.add_space(4.0);
}
