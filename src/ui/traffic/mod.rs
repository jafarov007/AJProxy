pub mod modals;
pub mod inspector;
pub mod table;

use eframe::egui::{self, Color32, RichText, Stroke};
use crate::models::{HttpEntry, FilterState, HeaderInjectionRule, TrafficAction};
pub use inspector::render_inspector_section;
pub use modals::{render_export_modal, render_host_filter_modal, render_method_filter_modal, render_path_filter_modal};
pub use table::render_traffic_table;

const BG_DARK: Color32 = Color32::from_rgb(18, 18, 20);

const ACCENT_BLUE: Color32 = Color32::from_rgb(2, 114, 176);
const ACCENT_CYAN: Color32 = Color32::from_rgb(0, 210, 255);
const ACCENT_GREEN: Color32 = Color32::from_rgb(74, 222, 128);
const ACCENT_AMBER: Color32 = Color32::from_rgb(251, 146, 60);
const ACCENT_RED: Color32 = Color32::from_rgb(248, 113, 113);

const TEXT_1: Color32 = Color32::from_rgb(240, 246, 252);
const TEXT_2: Color32 = Color32::from_rgb(148, 163, 184);

// ── Main UI Function ──────────────────────────────────────────────────────────
pub fn render(
    ui: &mut egui::Ui,
    entries: &[HttpEntry],
    selected_id: &mut Option<usize>,
    filter_state: &mut FilterState,
    active_tab: &mut usize, // 0 = Request, 1 = Response, 2 = Split View
    _header_rules: &mut Vec<HeaderInjectionRule>,
    _show_header_panel: &mut bool,
    ctx: &egui::Context,
) -> TrafficAction {
    let mut action = TrafficAction::default();

    // ── Filter Captured Entries ───────────────────────────────────────────────
    let filtered_entries: Vec<&HttpEntry> = entries
        .iter()
        .filter(|e| {
            // 1. Hide zero-size response packets if checkbox enabled
            if filter_state.hide_zero_size && e.length == 0 {
                return false;
            }

            // 2. Host Filters (wildcard/substring search e.g. "target" matches *target*)
            if !filter_state.host_filters.is_empty() {
                let host_lower = e.host.to_lowercase();
                let matches_host = filter_state.host_filters.iter().any(|filter| {
                    host_lower.contains(&filter.to_lowercase())
                });
                if !matches_host {
                    return false;
                }
            }

            // 3. Method Filters
            if !filter_state.method_filters.is_empty() {
                let method_upper = e.method.to_uppercase();
                let matches_method = filter_state.method_filters.iter().any(|m| {
                    method_upper == m.to_uppercase()
                });
                if !matches_method {
                    return false;
                }
            }

            // 4. Path Filters (substring match)
            if !filter_state.path_filters.is_empty() {
                let path_lower = e.path.to_lowercase();
                let matches_path = filter_state.path_filters.iter().any(|p| {
                    path_lower.contains(&p.to_lowercase())
                });
                if !matches_path {
                    return false;
                }
            }

            // 5. Search query filter (deep search: headers + body + URL)
            if !filter_state.search_query.is_empty() {
                let q = filter_state.search_query.to_lowercase();
                let matches_q = e.host.to_lowercase().contains(&q)
                    || e.url.to_lowercase().contains(&q)
                    || e.method.to_lowercase().contains(&q)
                    || e.request_headers.to_lowercase().contains(&q)
                    || e.response_headers.to_lowercase().contains(&q)
                    || e.request_body.to_lowercase().contains(&q)
                    || e.response_body.to_lowercase().contains(&q);
                if !matches_q {
                    return false;
                }
            }

            true
        })
        .collect();

    ui.vertical(|ui| {
        // ── Top Toolbar ───────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(RichText::new("HTTP HISTORY").size(14.0).color(ACCENT_CYAN).strong());
            ui.label(RichText::new(format!("({}/{} shown)", filtered_entries.len(), entries.len())).size(11.0).color(TEXT_2));
            
            ui.add_space(10.0);

            // 🗑 Clear History Button
            if ui.add(egui::Button::new(RichText::new("🗑 Clear History").size(11.0).color(ACCENT_RED))).clicked() {
                action.clear_history = true;
                *selected_id = None;
            }

            // 📥 Export History Button
            if ui.add(egui::Button::new(RichText::new("📥 Export History").size(11.0).color(ACCENT_BLUE))).clicked() {
                filter_state.show_export_modal = true;
                filter_state.export_status_msg = String::new();
            }

            // Top Right: Send to Repeater Button (Always visible!)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_sel = selected_id.is_some() && entries.iter().any(|e| selected_id.map(|id| e.id as usize == id).unwrap_or(false));
                let btn_color = if has_sel { ACCENT_GREEN } else { TEXT_2 };
                if ui.add_enabled(has_sel, egui::Button::new(RichText::new("🚀 Send to Repeater").size(11.0).color(btn_color))).clicked() {
                    if let Some(id) = *selected_id {
                        action.send_to_repeater = Some(id);
                    }
                }
            });
        });
        ui.add_space(4.0);

        // ── Action Bar / Filter Controls (Directly under Send to Repeater) ────
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Host Filter Button
                let host_btn_label = if filter_state.host_filters.is_empty() {
                    "➕ Host Filter".to_string()
                } else {
                    format!("🎯 Host ({})", filter_state.host_filters.len())
                };
                if ui.button(RichText::new(host_btn_label).size(10.0).color(if filter_state.host_filters.is_empty() { ACCENT_BLUE } else { ACCENT_AMBER })).clicked() {
                    filter_state.show_host_filter_modal = true;
                }

                ui.add_space(6.0);

                // Method Filter Button
                let method_btn_label = if filter_state.method_filters.is_empty() {
                    "🔧 Method Filter".to_string()
                } else {
                    format!("🔧 Methods ({})", filter_state.method_filters.join(","))
                };
                if ui.button(RichText::new(method_btn_label).size(10.0).color(if filter_state.method_filters.is_empty() { ACCENT_BLUE } else { ACCENT_AMBER })).clicked() {
                    filter_state.show_method_filter_modal = true;
                }

                ui.add_space(6.0);

                // Path Filter Button
                let path_btn_label = if filter_state.path_filters.is_empty() {
                    "📂 Path Filter".to_string()
                } else {
                    format!("📂 Paths ({})", filter_state.path_filters.len())
                };
                if ui.button(RichText::new(path_btn_label).size(10.0).color(if filter_state.path_filters.is_empty() { ACCENT_BLUE } else { ACCENT_AMBER })).clicked() {
                    filter_state.show_path_filter_modal = true;
                }

                ui.add_space(6.0);

                // Checkbox (Hide 0-byte responses)
                ui.checkbox(&mut filter_state.hide_zero_size, RichText::new("🚫 Hide 0B").size(10.0).color(TEXT_1));
            });
        });
        ui.add_space(4.0);

        let has_selection = selected_id.is_some() && entries.iter().any(|e| selected_id.map(|id| e.id as usize == id).unwrap_or(false));
        let avail_h = ui.available_height();
        let table_max_h = if has_selection { (avail_h * 0.38).clamp(140.0, 240.0) } else { avail_h - 10.0 };

        // ── Traffic Table ─────────────────────────────────────────────────────
        render_traffic_table(ui, &filtered_entries, selected_id, table_max_h, &mut action);

        ui.add_space(6.0);

        // ── Bottom Inspector Panel ───────────────────────────────────────────
        if let Some(id) = *selected_id {
            if let Some(entry) = entries.iter().find(|e| e.id as usize == id) {
                egui::Frame::none()
                    .fill(BG_DARK)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 40, 55)))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        // Tabs Header
                        ui.horizontal(|ui| {
                            ui.selectable_value(active_tab, 0, RichText::new("REQUEST").size(11.0).strong());
                            ui.selectable_value(active_tab, 1, RichText::new("RESPONSE").size(11.0).strong());
                            ui.selectable_value(active_tab, 2, RichText::new("SPLIT VIEW").size(11.0).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let trunc_url = crate::ui::traffic::table::truncate_str(&format!("URL: {}", entry.url), 90);
                                ui.add(
                                    egui::Label::new(RichText::new(trunc_url).size(10.0).color(ACCENT_CYAN))
                                        .truncate(true)
                                ).on_hover_text(&entry.url);
                            });
                        });
                        ui.separator();

                        egui::ScrollArea::vertical()
                            .id_source("traffic_inspector_scroll")
                            .max_height(ui.available_height() - 8.0)
                            .show(ui, |ui| {
                                match *active_tab {
                                    0 => render_inspector_section(ui, entry, true),
                                    1 => render_inspector_section(ui, entry, false),
                                    _ => {
                                        // Split View: Left = Request, Right = Response
                                        ui.columns(2, |cols| {
                                            render_inspector_section(&mut cols[0], entry, true);
                                            render_inspector_section(&mut cols[1], entry, false);
                                        });
                                    }
                                }
                            });
                    });
            }
        }
    });

    // ── Render Modals ─────────────────────────────────────────────────────────
    render_export_modal(ctx, filter_state, entries);
    render_host_filter_modal(ctx, filter_state);
    render_method_filter_modal(ctx, filter_state);
    render_path_filter_modal(ctx, filter_state);

    action
}
