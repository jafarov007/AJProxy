use egui::{self, RichText, Rounding, Stroke, ScrollArea, FontFamily};
use crate::models::*;
use crate::theme::*;
use crate::proxy::cert;

pub fn render(ui: &mut egui::Ui, settings: &mut AppSettings) {
    ScrollArea::vertical()
        .id_source("settings_scroll_area")
        .show(ui, |ui| {
            // ── Section 1: Proxy Listeners & Binding Settings ─────────────────
            section_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Proxy Listeners & Binding Interfaces").size(13.0).color(TEXT_0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("+ Add New Listener").size(11.0).color(TEXT_0).strong())
                                .fill(ACCENT_BLUE)
                                .rounding(Rounding::same(4.0))
                        ).clicked() {
                            let next_port = 8080 + settings.listeners.len() as u16;
                            settings.listeners.push(ProxyListenerConfig {
                                enabled: true,
                                bind_address: "0.0.0.0".into(),
                                bind_port: next_port,
                                protocol: "Auto".into(),
                                tls_mitm: true,
                            });
                        }
                    });
                });
                ui.label(RichText::new("Configure binding addresses and ports (Use 0.0.0.0 for mobile device proxying).").size(11.0).color(TEXT_2));
                ui.separator();
                ui.add_space(4.0);

                // Listeners Grid Table
                let mut to_delete: Option<usize> = None;

                // Table Header
                ui.horizontal(|ui| {
                    ui.add_space(30.0);
                    ui.label(RichText::new("Bind Address").size(11.0).color(TEXT_1).strong());
                    ui.add_space(70.0);
                    ui.label(RichText::new("Port").size(11.0).color(TEXT_1).strong());
                    ui.add_space(35.0);
                    ui.label(RichText::new("Protocol").size(11.0).color(TEXT_1).strong());
                    ui.add_space(30.0);
                    ui.label(RichText::new("TLS MITM").size(11.0).color(TEXT_1).strong());
                });
                ui.separator();

                for (idx, listener) in settings.listeners.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut listener.enabled, "");

                        // Address Input
                        ui.add(egui::TextEdit::singleline(&mut listener.bind_address).desired_width(140.0));

                        // Port Input
                        let mut p_str = listener.bind_port.to_string();
                        if ui.add(egui::TextEdit::singleline(&mut p_str).desired_width(60.0)).changed() {
                            if let Ok(p) = p_str.parse::<u16>() {
                                listener.bind_port = p;
                            }
                        }

                        // Protocol Selector
                        egui::ComboBox::from_id_source(format!("proto_combo_{}", idx))
                            .selected_text(&listener.protocol)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut listener.protocol, "Auto".into(), "Auto");
                                ui.selectable_value(&mut listener.protocol, "HTTP/1.1".into(), "HTTP/1.1");
                                ui.selectable_value(&mut listener.protocol, "HTTP/2".into(), "HTTP/2");
                            });

                        // TLS Checkbox
                        ui.checkbox(&mut listener.tls_mitm, "Enable TLS");

                        // Delete Listener Button
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(
                                egui::Button::new(RichText::new("✖ Delete").size(11.0).color(ACCENT_RED))
                                    .fill(BG_RAISED)
                                    .stroke(Stroke::new(0.5_f32, BORDER))
                            ).clicked() {
                                to_delete = Some(idx);
                            }
                        });
                    });
                    ui.add_space(2.0);
                }

                if let Some(idx) = to_delete {
                    if settings.listeners.len() > 1 {
                        settings.listeners.remove(idx);
                    } else {
                        settings.cert_status_msg = "Cannot delete the last remaining listener.".into();
                    }
                }

                // Keep main listen_address and listen_port in sync with active listener
                settings.sync_active_listener();
            });

            ui.add_space(8.0);

            // ── Section 2: CA Certificate Management (3 Action Buttons + File Dialogs) ─────
            section_frame().show(ui, |ui| {
                ui.label(RichText::new("CA Certificate Management").size(13.0).color(TEXT_0).strong());
                ui.label(RichText::new("Manage Root CA Certificate for HTTPS MITM inspection.").size(11.0).color(TEXT_2));
                ui.separator();
                ui.add_space(4.0);

                // Current Active CA Location
                let ca_path = cert::get_cert_path();
                let ca_exists = ca_path.exists();
                let status_icon = if ca_exists { "● Active" } else { "○ Missing" };
                let status_color = if ca_exists { ACCENT_GREEN } else { ACCENT_RED };

                ui.horizontal(|ui| {
                    ui.label(RichText::new("CA Store Path:").size(11.0).color(TEXT_1).strong());
                    ui.label(
                        RichText::new(ca_path.to_string_lossy())
                            .size(11.0)
                            .color(TEXT_0)
                            .family(FontFamily::Monospace)
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(status_icon)
                            .size(11.0)
                            .color(status_color)
                            .strong()
                    );
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    // Button 1: Export CA Certificate (.crt)
                    if ui.add(
                        egui::Button::new(RichText::new("📥 Export CA Certificate (.crt)").size(12.0).color(TEXT_0).strong())
                            .fill(ACCENT_BLUE)
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        if let Some(dest_path) = rfd::FileDialog::new()
                            .set_file_name("ajproxy_ca.crt")
                            .add_filter("Certificate", &["crt", "pem"])
                            .save_file()
                        {
                            match cert::export_ca_cert(&dest_path) {
                                Ok(_) => {
                                    settings.cert_status_msg = format!("✅ CA Certificate exported successfully to {}", dest_path.display());
                                }
                                Err(e) => {
                                    settings.cert_status_msg = format!("❌ Export error: {}", e);
                                }
                            }
                        }
                    }

                    ui.add_space(8.0);

                    // Button 2: Regenerate CA Certificate
                    if ui.add(
                        egui::Button::new(RichText::new("🔄 Regenerate CA Certificate").size(12.0).color(TEXT_0).strong())
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(1.0_f32, ACCENT_AMBER))
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        match cert::generate_and_save_ca() {
                            Ok(_) => {
                                settings.cert_status_msg = format!("✅ New Root CA generated & saved to {}", cert::get_cert_dir().display());
                            }
                            Err(e) => {
                                settings.cert_status_msg = format!("❌ Regeneration error: {}", e);
                            }
                        }
                    }

                    ui.add_space(8.0);

                    // Button 3: Auto-Trust CA System-Wide
                    if ui.add(
                        egui::Button::new(RichText::new("⚡ Auto-Trust CA System-Wide (Ubuntu)").size(12.0).color(TEXT_0).strong())
                            .fill(ACCENT_GREEN)
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        match cert::install_ca_system_wide() {
                            Ok(msg) => {
                                settings.cert_status_msg = msg;
                            }
                            Err(e) => {
                                settings.cert_status_msg = format!("❌ Auto-trust error: {}", e);
                            }
                        }
                    }

                    ui.add_space(8.0);

                    // Button 3: Upload / Import CA Certificate
                    if ui.add(
                        egui::Button::new(RichText::new("📤 Upload / Import CA Certificate").size(12.0).color(TEXT_0).strong())
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(1.0_f32, BORDER))
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        if let Some(src_path) = rfd::FileDialog::new()
                            .add_filter("Certificate File", &["crt", "pem", "cer", "key"])
                            .pick_file()
                        {
                            match cert::import_ca_cert(&src_path) {
                                Ok(_) => {
                                    settings.cert_status_msg = format!("✅ Custom CA Certificate imported & activated from {}", src_path.display());
                                }
                                Err(e) => {
                                    settings.cert_status_msg = format!("❌ Import error: {}", e);
                                }
                            }
                        }
                    }
                });

                if !settings.cert_status_msg.is_empty() {
                    ui.add_space(8.0);
                    let is_err = settings.cert_status_msg.contains("❌");
                    let msg_color = if is_err { ACCENT_RED } else { ACCENT_GREEN };

                    ui.label(
                        RichText::new(&settings.cert_status_msg)
                            .size(11.0)
                            .color(msg_color)
                            .strong()
                            .family(FontFamily::Monospace)
                    );
                }
            });

            ui.add_space(8.0);

            // ── Section 3: Intercept & Passthrough Options ───────────────────
            section_frame().show(ui, |ui| {
                ui.label(RichText::new("Interception & Scope Rules").size(13.0).color(TEXT_0).strong());
                ui.separator();
                ui.add_space(4.0);

                ui.checkbox(&mut settings.intercept_requests, "Intercept HTTP Requests in real-time");
                ui.checkbox(&mut settings.intercept_responses, "Intercept HTTP Responses in real-time");
                ui.add_space(6.0);

                ui.label(RichText::new("SSL Passthrough Hosts (comma separated):").size(11.0).color(TEXT_1).strong());
                ui.add(egui::TextEdit::singleline(&mut settings.passthrough_hosts).desired_width(f32::INFINITY));
            });
        });
}