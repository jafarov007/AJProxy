use egui::{self, RichText, Rounding, Stroke, ScrollArea, FontFamily, TextStyle};
use crate::models::*;
use crate::theme::*;
use crate::ui::syntax;

pub fn render(ui: &mut egui::Ui, state: &mut BruteForceState) {
    // ── Config bar ────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(RichText::new("Target:").size(12.0).color(TEXT_1).strong());
        ui.add(egui::TextEdit::singleline(&mut state.target_url).desired_width(280.0));

        ui.add_space(8.0);
        ui.label(RichText::new("Type:").size(12.0).color(TEXT_1).strong());
        for (at, label) in &[
            (AttackType::Sniper, "Sniper"),
            (AttackType::BatteringRam, "Battering Ram"),
            (AttackType::Pitchfork, "Pitchfork"),
            (AttackType::ClusterBomb, "Cluster Bomb"),
        ] {
            let active = state.attack_type == *at;
            let color = if active { TEXT_0 } else { TEXT_2 };
            if ui.add(
                egui::Button::new(RichText::new(*label).size(11.0).color(color).strong())
                    .fill(if active { BG_OVERLAY } else { BG_RAISED })
                    .rounding(Rounding::same(3.0))
                    .stroke(if active { Stroke::new(1.0_f32, ACCENT_BLUE) } else { Stroke::new(0.5_f32, BORDER) })
            ).clicked() {
                state.attack_type = at.clone();
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (label, color) = if state.running {
                ("Stop Attack", ACCENT_RED)
            } else {
                ("▶ Start Attack", ACCENT_AMBER)
            };

            if ui.add(
                egui::Button::new(RichText::new(label).size(12.0).color(TEXT_0).strong())
                    .fill(color)
                    .rounding(Rounding::same(4.0))
            ).clicked() {
                state.running = !state.running;
                state.is_running = state.running;
                if state.running && state.results.is_empty() {
                    for (i, p) in ["admin", "password", "123456", "' OR 1=1--", "root", "guest", "sec_admin"].iter().enumerate() {
                        state.results.push(BruteResult {
                            id: i + 1,
                            payload: p.to_string(),
                            status_code: if *p == "sec_admin" { 200 } else if i == 3 { 500 } else { 401 },
                            length: if *p == "sec_admin" { 3412 } else { 182 },
                            duration_ms: 35 + (i * 12) as u64,
                        });
                    }
                }
            }
        });
    });

    ui.add_space(6.0);

    // ── Template + Payloads split ─────────────────────────────────
    ui.columns(2, |cols| {
        section_frame().show(&mut cols[0], |ui| {
            ui.label(RichText::new("Request Template").size(12.0).color(TEXT_0).strong());
            ui.label(RichText::new("Mark position markers with §payload§").size(11.0).color(ACCENT_BLUE));
            ui.separator();

            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                syntax::http_layouter(ui, string, wrap_width)
            };

            ScrollArea::vertical()
                .id_source("intruder_template_scroll")
                .max_height(260.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.body_template)
                            .font(TextStyle::Monospace)
                            .layouter(&mut layouter)
                            .desired_width(f32::INFINITY)
                    );
                });
        });

        section_frame().show(&mut cols[1], |ui| {
            ui.label(RichText::new("Payload Positions / Options").size(12.0).color(TEXT_0).strong());
            ui.label(RichText::new("Enter test payloads (one per line)").size(11.0).color(TEXT_2));
            ui.separator();
            ScrollArea::vertical()
                .id_source("intruder_payloads_scroll")
                .max_height(260.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.payloads)
                            .font(TextStyle::Monospace)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                    );
                });
        });
    });

    ui.add_space(6.0);

    // ── Results table ─────────────────────────────────────────────
    section_frame().show(ui, |ui| {
        ui.label(RichText::new(format!("Attack Results ({})", state.results.len())).size(12.0).color(TEXT_0).strong());
        ui.separator();

        // Header
        ui.horizontal(|ui| {
            ui.label(RichText::new("#").size(11.0).color(TEXT_2).strong());
            ui.add_space(20.0);
            ui.label(RichText::new("Payload").size(11.0).color(TEXT_2).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("Latency").size(11.0).color(TEXT_2).strong());
                ui.add_space(20.0);
                ui.label(RichText::new("Size").size(11.0).color(TEXT_2).strong());
                ui.add_space(20.0);
                ui.label(RichText::new("Status").size(11.0).color(TEXT_2).strong());
            });
        });

        ScrollArea::vertical()
            .id_source("intruder_results_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                for r in &state.results {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}", r.id)).size(11.0).color(TEXT_2).family(FontFamily::Monospace));
                        ui.add_space(12.0);
                        ui.label(RichText::new(&r.payload).size(11.0).color(TEXT_0).strong().family(FontFamily::Monospace));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{}ms", r.duration_ms)).size(11.0).color(TEXT_2).family(FontFamily::Monospace));
                            ui.add_space(10.0);
                            ui.label(RichText::new(format!("{}B", r.length)).size(11.0).color(TEXT_2).family(FontFamily::Monospace));
                            ui.add_space(10.0);
                            ui.label(RichText::new(format!("{}", r.status_code)).size(11.0).color(status_color(r.status_code)).strong().family(FontFamily::Monospace));
                        });
                    });
                }
            });
    });
}
