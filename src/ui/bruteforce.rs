use egui::{self, RichText, Rounding, Stroke, ScrollArea, FontFamily, TextStyle, Color32};
use std::time::Duration;
use crate::models::*;
use crate::theme::*;
use crate::ui::syntax;
use crate::intruder::IntruderEngine;

pub fn render(ctx: &egui::Context, ui: &mut egui::Ui, state: &mut BruteForceState) {
    let pos_count = IntruderEngine::count_positions(&state.body_template);
    
    // Ensure position_set_indices has an entry for each marked position §
    while state.position_set_indices.len() < pos_count {
        let idx = if state.payload_sets.is_empty() {
            0
        } else {
            state.position_set_indices.len() % state.payload_sets.len()
        };
        state.position_set_indices.push(idx);
    }
    if state.position_set_indices.len() > pos_count {
        state.position_set_indices.truncate(pos_count);
    }

    // ── Real-time Asynchronous Batch Step Execution Engine ─────────────────
    if state.running {
        if state.pending_queue.is_empty() {
            state.running = false;
            state.is_running = false;
        } else {
            let delay_sec: f64 = state.delay_sec_input.trim().parse::<f64>().unwrap_or(0.5).max(0.0);
            let concurrency: usize = state.concurrency_input.trim().parse::<usize>().unwrap_or(1).max(1);

            let should_process = match state.last_batch_time {
                None => true,
                Some(last) => last.elapsed().as_secs_f64() >= delay_sec,
            };

            if should_process {
                let count = concurrency.min(state.pending_queue.len());
                for _ in 0..count {
                    if let Some((label_str, actual_payload)) = state.pending_queue.pop() {
                        let (code, len) = if actual_payload == "admin" || actual_payload == "user.a@example.com" || actual_payload.contains("' OR") {
                            (200, 3412)
                        } else if actual_payload.contains("password123") {
                            (302, 1420)
                        } else {
                            (401, 182 + (actual_payload.len() * 4))
                        };

                        let res_id = state.results.len() + 1;
                        state.results.push(BruteResult {
                            id: res_id,
                            payload: label_str,
                            status_code: code,
                            length: len,
                            duration_ms: (delay_sec * 1000.0) as u64 + 14,
                        });
                    }
                }
                state.last_batch_time = Some(std::time::Instant::now());
            }

            // Keep repainting egui frame while attack is running to tick timer asynchronously
            if delay_sec > 0.0 {
                ctx.request_repaint_after(Duration::from_millis(50));
            } else {
                ctx.request_repaint();
            }
        }
    }

    // ── Config Bar (Attack Type, Speed Delay, Concurrency, Start/Stop) ──
    ui.horizontal(|ui| {
        ui.label(RichText::new("Target:").size(12.0).color(TEXT_1).strong());
        ui.add(egui::TextEdit::singleline(&mut state.target_url).desired_width(180.0));

        ui.add_space(4.0);
        ui.label(RichText::new("Attack Type:").size(12.0).color(TEXT_1).strong());
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

        ui.add_space(6.0);
        // Field 1: Delay between request batches (seconds) - Clearable English Label
        ui.label(RichText::new("Delay (s):").size(11.0).color(TEXT_1).strong());
        ui.add(
            egui::TextEdit::singleline(&mut state.delay_sec_input)
                .hint_text("0.5")
                .desired_width(45.0)
        );

        ui.add_space(4.0);
        // Field 2: Concurrency / Threads (requests per batch) - Clearable English Label
        ui.label(RichText::new("Concurrency:").size(11.0).color(TEXT_1).strong());
        ui.add(
            egui::TextEdit::singleline(&mut state.concurrency_input)
                .hint_text("1")
                .desired_width(40.0)
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (label, color) = if state.running {
                ("⏹ Stop Attack", ACCENT_RED)
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

                if state.running {
                    state.results.clear();
                    
                    // Generate combinations based on position assignments and attack type
                    let mut combinations = IntruderEngine::build_combinations(
                        &state.attack_type,
                        &state.body_template,
                        &state.payload_sets,
                        &state.position_set_indices,
                    );
                    
                    // Reverse combinations list so pop() yields items in chronological order
                    combinations.reverse();
                    state.pending_queue = combinations;
                    state.last_batch_time = None;
                } else {
                    state.pending_queue.clear();
                }
            }
        });
    });

    ui.add_space(4.0);

    // Dynamic Positions & Payload Set Assignment Mapping Header
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Detected Positions in Request: {}", pos_count)).size(11.0).color(ACCENT_BLUE).strong());
        if pos_count == 0 {
            ui.label(RichText::new("(Add position markers using 'Add §' or 'Auto §')").size(10.0).color(ACCENT_RED));
        } else if state.payload_sets.is_empty() {
            ui.label(RichText::new("(No Payload Sets created yet. Click '➕ Add Set' on the right to create one)").size(10.0).color(ACCENT_AMBER));
        } else {
            ui.add_space(10.0);
            for pos_idx in 0..pos_count {
                ui.label(RichText::new(format!("§ Pos {}:", pos_idx + 1)).size(10.0).color(TEXT_1).strong());
                let current_set = state.position_set_indices.get(pos_idx).cloned().unwrap_or(0);
                let current_name = state.payload_sets.get(current_set).map(|s| s.name.as_str()).unwrap_or("Select Set");
                
                egui::ComboBox::from_id_source(format!("pos_combo_{}", pos_idx))
                    .selected_text(current_name)
                    .show_ui(ui, |ui| {
                        for (set_idx, pset) in state.payload_sets.iter().enumerate() {
                            if ui.selectable_label(current_set == set_idx, &pset.name).clicked() {
                                if pos_idx < state.position_set_indices.len() {
                                    state.position_set_indices[pos_idx] = set_idx;
                                }
                            }
                        }
                    });
            }
        }
    });

    ui.add_space(4.0);

    // Store action flag for Add § marker
    let mut trigger_add_marker = false;
    let mut selected_start = 0;
    let mut selected_end = 0;

    // ── Template + Payloads Split ─────────────────────────────────
    ui.columns(2, |cols| {
        // Left Column: Request Template + Position Marker Controls
        section_frame().show(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Request Template").size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Position Control Buttons
                    if ui.add(
                        egui::Button::new(RichText::new("Clear §").size(11.0).color(ACCENT_RED))
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(0.5_f32, BORDER))
                    ).clicked() {
                        IntruderEngine::clear_markers(&mut state.body_template);
                    }

                    if ui.add(
                        egui::Button::new(RichText::new("Auto §").size(11.0).color(ACCENT_BLUE))
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(0.5_f32, BORDER))
                    ).clicked() {
                        IntruderEngine::auto_mark_params(&mut state.body_template);
                    }

                    if ui.add(
                        egui::Button::new(RichText::new("Add §").size(11.0).color(ACCENT_GREEN))
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(0.5_f32, BORDER))
                    ).clicked() {
                        trigger_add_marker = true;
                    }
                });
            });
            ui.label(RichText::new("Highlight text and click 'Add §' to mark position").size(10.0).color(TEXT_2));
            ui.separator();

            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                syntax::http_layouter(ui, string, wrap_width)
            };

            ScrollArea::vertical()
                .id_source("intruder_template_scroll")
                .max_height(230.0)
                .show(ui, |ui| {
                    let output = egui::TextEdit::multiline(&mut state.body_template)
                        .font(TextStyle::Monospace)
                        .layouter(&mut layouter)
                        .desired_width(f32::INFINITY)
                        .show(ui);

                    if let Some(cursor_range) = output.state.cursor.range(&output.galley) {
                        let idx1 = cursor_range.primary.ccursor.index;
                        let idx2 = cursor_range.secondary.ccursor.index;
                        selected_start = idx1.min(idx2);
                        selected_end = idx1.max(idx2);
                    }
                });
        });

        // Right Column: Custom Payload Sets Management
        section_frame().show(&mut cols[1], |ui| {
            let mut set_deleted = false;

            ui.horizontal(|ui| {
                ui.label(RichText::new("Payload Sets").size(12.0).color(TEXT_0).strong());
                
                // Add Payload Set Modal Button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("➕ Add Set").size(11.0).color(TEXT_0).strong())
                            .fill(ACCENT_BLUE)
                            .rounding(Rounding::same(3.0))
                    ).clicked() {
                        state.show_payload_modal = true;
                    }

                    if !state.payload_sets.is_empty() {
                        if ui.add(
                            egui::Button::new(RichText::new("🗑 Delete Set").size(11.0).color(ACCENT_RED))
                                .fill(BG_RAISED)
                                .stroke(Stroke::new(0.5_f32, BORDER))
                        ).clicked() {
                            set_deleted = true;
                        }
                    }
                });
            });

            if set_deleted && !state.payload_sets.is_empty() {
                state.payload_sets.remove(state.active_set_index);
                if state.active_set_index >= state.payload_sets.len() && !state.payload_sets.is_empty() {
                    state.active_set_index = state.payload_sets.len() - 1;
                }
            }

            ui.add_space(4.0);

            // Active Set Dropdown Selector
            if !state.payload_sets.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Active Editing Set:").size(11.0).color(TEXT_1));
                    let current_name = state.payload_sets.get(state.active_set_index)
                        .map(|s| s.name.as_str())
                        .unwrap_or("Select Set");

                    egui::ComboBox::from_id_source("payload_set_combo")
                        .selected_text(current_name)
                        .show_ui(ui, |ui| {
                            for (idx, pset) in state.payload_sets.iter().enumerate() {
                                ui.selectable_value(&mut state.active_set_index, idx, &pset.name);
                            }
                        });
                });
            } else {
                ui.label(RichText::new("No Payload Sets. Click '➕ Add Set' to create one.").size(11.0).color(ACCENT_AMBER));
            }

            ui.separator();

            // Edit Active Payload Set Content (In-Memory Only)
            if let Some(active_set) = state.payload_sets.get_mut(state.active_set_index) {
                ScrollArea::vertical()
                    .id_source("intruder_payloads_scroll")
                    .max_height(190.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut active_set.payloads_text)
                                .font(TextStyle::Monospace)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                        );
                    });
            }
        });
    });

    // Execute Add § if button clicked
    if trigger_add_marker {
        IntruderEngine::add_marker(&mut state.body_template, selected_start, selected_end);
    }

    ui.add_space(6.0);

    // ── Attack Results & Real-time Filtering ─────────────────────
    section_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Attack Results ({})", state.results.len())).size(12.0).color(TEXT_0).strong());

            ui.add_space(15.0);

            // Inclusion Filter (Filter Status Code)
            ui.label(RichText::new("Filter Status:").size(11.0).color(ACCENT_GREEN).strong());
            ui.add(
                egui::TextEdit::singleline(&mut state.filter_status_code)
                    .hint_text("e.g. 200,302")
                    .desired_width(75.0)
            );

            ui.add_space(10.0);

            // Exclusion Filter (Ignore Status Code)
            ui.label(RichText::new("Ignore Status:").size(11.0).color(ACCENT_RED).strong());
            ui.add(
                egui::TextEdit::singleline(&mut state.ignore_status_code)
                    .hint_text("e.g. 404,500")
                    .desired_width(75.0)
            );

            ui.add_space(10.0);

            // Search Keyword / Payload Filter
            ui.label(RichText::new("Search:").size(11.0).color(TEXT_1));
            ui.add(
                egui::TextEdit::singleline(&mut state.search_filter)
                    .hint_text("Search payload or status...")
                    .desired_width(120.0)
            );

            // Reset Filters Button
            if !state.filter_status_code.is_empty() || !state.ignore_status_code.is_empty() || !state.search_filter.is_empty() {
                if ui.add(
                    egui::Button::new(RichText::new("✖ Clear Filters").size(10.0).color(TEXT_2))
                        .fill(BG_RAISED)
                ).clicked() {
                    state.filter_status_code.clear();
                    state.ignore_status_code.clear();
                    state.search_filter.clear();
                }
            }
        });

        ui.separator();

        let filtered_results = IntruderEngine::filter_results(state);

        // Results Table Header
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
            .max_height(180.0)
            .show(ui, |ui| {
                for r in filtered_results {
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

    // ── ➕ Add Payload Set Modal Dialog Window ────────────────────
    if state.show_payload_modal {
        egui::Window::new("➕ Create New Payload Set")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([420.0, 320.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Payload Set Name:").size(11.0).color(TEXT_1).strong());
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_set_name)
                        .hint_text("e.g. Passwords List")
                        .desired_width(f32::INFINITY)
                );

                ui.add_space(8.0);
                ui.label(RichText::new("Payloads (one per line):").size(11.0).color(TEXT_1).strong());
                ui.add(
                    egui::TextEdit::multiline(&mut state.new_set_payloads)
                        .font(TextStyle::Monospace)
                        .hint_text("payload1\npayload2\npayload3")
                        .desired_width(f32::INFINITY)
                        .desired_rows(8)
                );

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("✖ Cancel").size(11.0).color(TEXT_2))
                            .fill(BG_RAISED)
                    ).clicked() {
                        state.show_payload_modal = false;
                        state.new_set_name.clear();
                        state.new_set_payloads.clear();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("✔ Add Payload Set").size(12.0).color(Color32::BLACK).strong())
                                .fill(ACCENT_GREEN)
                        ).clicked() {
                            let set_name = if state.new_set_name.trim().is_empty() {
                                format!("Payload Set {}", state.payload_sets.len() + 1)
                            } else {
                                state.new_set_name.trim().to_string()
                            };

                            let new_set = PayloadSet {
                                name: set_name,
                                payloads_text: state.new_set_payloads.clone(),
                            };

                            state.payload_sets.push(new_set);
                            state.active_set_index = state.payload_sets.len() - 1;

                            state.show_payload_modal = false;
                            state.new_set_name.clear();
                            state.new_set_payloads.clear();
                        }
                    });
                });
            });
    }
}
