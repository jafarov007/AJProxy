pub mod history;
pub mod intercept;
pub mod repeater;

use egui::{self, Color32, RichText, Rounding, Stroke};
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

        // ── Right-aligned Controls: WS Proxy Global Toggle & Scope Modal ──
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // WS Scope Host Filter Modal toggle
            static SHOW_WS_SCOPE_MODAL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            static SCOPE_INPUT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

            let scope_count = if let Ok(lock) = crate::proxy::store::WS_SCOPE_HOSTS.lock() { lock.len() } else { 0 };
            let scope_btn_text = if scope_count == 0 {
                "🎯 WS Scope (All)".to_string()
            } else {
                format!("🎯 WS Scope ({})", scope_count)
            };

            if ui.add(
                egui::Button::new(RichText::new(scope_btn_text).size(11.0).color(if scope_count == 0 { ACCENT_BLUE } else { ACCENT_AMBER }).strong())
                    .fill(BG_RAISED)
                    .rounding(Rounding::same(4.0))
            ).clicked() {
                SHOW_WS_SCOPE_MODAL.store(true, std::sync::atomic::Ordering::SeqCst);
            }

            ui.add_space(8.0);

            // WS Proxy ON/OFF Toggle
            let is_ws_on = crate::proxy::store::is_ws_proxy_enabled();
            let (ws_btn_text, ws_btn_bg, ws_btn_fg) = if is_ws_on {
                ("🌐 WS Proxy: ON", ACCENT_GREEN, TEXT_0)
            } else {
                ("🚫 WS Proxy: OFF (Bypass)", Color32::from_rgb(60, 20, 25), ACCENT_RED)
            };

            if ui.add(
                egui::Button::new(RichText::new(ws_btn_text).size(11.0).color(ws_btn_fg).strong())
                    .fill(ws_btn_bg)
                    .rounding(Rounding::same(4.0))
            ).clicked() {
                crate::proxy::store::set_ws_proxy_enabled(!is_ws_on);
            }

            // Scope Modal Window
            if SHOW_WS_SCOPE_MODAL.load(std::sync::atomic::Ordering::SeqCst) {
                let mut is_open = true;
                egui::Window::new(RichText::new("🎯 WebSocket Scope Filter Hosts").size(14.0).color(TEXT_0).strong())
                    .open(&mut is_open)
                    .collapsible(false)
                    .resizable(false)
                    .default_size([420.0, 260.0])
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ui.ctx(), |ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::new("Filter WebSocket inspection by target host. If empty, all WebSocket traffic is inspected.").size(11.0).color(TEXT_2));
                        ui.add_space(8.0);

                        let mut input = SCOPE_INPUT.lock().unwrap().take().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut input).hint_text("e.g. echo.websocket.org or socket.io").desired_width(260.0));
                            if ui.button(RichText::new("➕ Add Host").size(11.0).color(ACCENT_GREEN)).clicked() {
                                let val = input.trim().to_string();
                                if !val.is_empty() {
                                    if let Ok(mut lock) = crate::proxy::store::WS_SCOPE_HOSTS.lock() {
                                        if !lock.contains(&val) {
                                            lock.push(val);
                                        }
                                    }
                                    input.clear();
                                }
                            }
                        });
                        *SCOPE_INPUT.lock().unwrap() = Some(input);

                        ui.add_space(10.0);
                        ui.separator();
                        ui.label(RichText::new("Active WS Scope Hosts:").size(11.0).color(ACCENT_CYAN).strong());
                        ui.add_space(4.0);

                        egui::ScrollArea::vertical()
                            .id_source("ws_scope_scroll")
                            .max_height(100.0)
                            .show(ui, |ui| {
                                let hosts = crate::proxy::store::WS_SCOPE_HOSTS.lock().map(|l| l.clone()).unwrap_or_default();
                                if hosts.is_empty() {
                                    ui.label(RichText::new("No host filters active (Scope: ALL)").size(11.0).color(TEXT_2));
                                } else {
                                    let mut to_remove = None;
                                    for (idx, h) in hosts.iter().enumerate() {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("• {}", h)).size(11.0).color(ACCENT_GREEN).strong());
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.button(RichText::new("✖").size(10.0).color(ACCENT_RED)).clicked() {
                                                    to_remove = Some(idx);
                                                }
                                            });
                                        });
                                    }
                                    if let Some(idx) = to_remove {
                                        if let Ok(mut lock) = crate::proxy::store::WS_SCOPE_HOSTS.lock() {
                                            if idx < lock.len() {
                                                lock.remove(idx);
                                            }
                                        }
                                    }
                                }
                            });

                        ui.add_space(10.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Clear All").size(11.0).color(ACCENT_AMBER)).clicked() {
                                if let Ok(mut lock) = crate::proxy::store::WS_SCOPE_HOSTS.lock() {
                                    lock.clear();
                                }
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("Close").size(11.0).color(TEXT_0)).clicked() {
                                    SHOW_WS_SCOPE_MODAL.store(false, std::sync::atomic::Ordering::SeqCst);
                                }
                            });
                        });
                    });
                if !is_open {
                    SHOW_WS_SCOPE_MODAL.store(false, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    // ── Sub-Tab View Switching ───────────────────────────────────────
    match ws_sub_tab {
        WsSubTab::History => {
            history::render(
                ui,
                history_state,
                ws_connections,
                ws_frames,
                repeater_tabs,
                active_repeater_tab,
                ws_sub_tab,
            );
        }
        WsSubTab::Intercept => {
            intercept::render(ui, intercept_state, repeater_tabs, active_repeater_tab, ws_sub_tab);
        }
        WsSubTab::Repeater => {
            repeater::render(ui, repeater_tabs, active_repeater_tab);
        }
    }
}
