pub mod history;
pub mod intercept;
pub mod repeater;

use egui::{self, RichText, Rounding, Stroke};
use crate::models::*;
use crate::theme::*;

pub fn render(
    ui: &mut egui::Ui,
    ws_sub_tab: &mut WsSubTab,
    history_state: &mut WsHistoryState,
    intercept_state: &mut WsInterceptState,
    repeater_tabs: &mut Vec<WsRepeaterTab>,
    active_repeater_tab: &mut usize,
    ws_connections: &[WsConnection],
    ws_frames: &[WsFrameEntry],
) {
    // ── WebSocket Sub-Navigation Bar ─────────────────────────────────
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(RichText::new("⚡ WebSocket Suite:").size(12.0).color(TEXT_0).strong());
        ui.add_space(10.0);

        let sub_tabs = [
            (WsSubTab::History, "📜 WS History"),
            (WsSubTab::Intercept, "🛡️ WS Intercept"),
            (WsSubTab::Repeater, "🚀 WS Repeater"),
        ];

        for (tab, label) in &sub_tabs {
            let active = *ws_sub_tab == *tab;
            let text_color = if active { TEXT_0 } else { TEXT_2 };
            let bg_color = if active { ACCENT_BLUE } else { BG_RAISED };

            let btn = ui.add(
                egui::Button::new(RichText::new(*label).size(11.0).color(text_color).strong())
                    .fill(bg_color)
                    .rounding(Rounding::same(4.0))
                    .stroke(if active { Stroke::new(1.0_f32, ACCENT_BLUE) } else { Stroke::new(0.5_f32, BORDER) })
            );

            if btn.clicked() {
                *ws_sub_tab = *tab;
            }
        }
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    // ── Sub-Tab View Switching ───────────────────────────────────────
    match ws_sub_tab {
        WsSubTab::History => {
            history::render(ui, history_state, ws_connections, ws_frames);
        }
        WsSubTab::Intercept => {
            intercept::render(ui, intercept_state);
        }
        WsSubTab::Repeater => {
            repeater::render(ui, repeater_tabs, active_repeater_tab);
        }
    }
}
