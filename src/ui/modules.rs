use egui::{self, RichText, Rounding, Stroke, ScrollArea};
use crate::models::*;
use crate::theme::*;

pub fn render(ui: &mut egui::Ui, modules: &mut [ModuleInfo]) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Extensions").size(11.0).color(TEXT_0).strong());
        ui.add_space(12.0);
        let active = modules.iter().filter(|m| m.enabled).count();
        ui.label(RichText::new(format!("{}/{} active", active, modules.len())).size(10.0).color(TEXT_2));
    });

    ui.add_space(4.0);

    ScrollArea::vertical()
        .id_source("modules_list_scroll")
        .show(ui, |ui| {
            for module in modules.iter_mut() {
                let border = if module.enabled { ACCENT_BLUE } else { BORDER_DIM };

                section_frame()
                    .stroke(Stroke::new(1.0_f32, border))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&module.name).size(11.0).color(TEXT_0).strong());
                            ui.add_space(6.0);

                            let (cat, color) = match module.category {
                                ModuleCategory::Scanner => ("Scanner", ACCENT_RED),
                                ModuleCategory::Analyzer => ("Analyzer", ACCENT_AMBER),
                                ModuleCategory::Custom => ("Custom", ACCENT_VIOLET),
                            };
                            ui.label(RichText::new(cat).size(9.0).color(color));

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let (label, color) = if module.enabled { ("ON", ACCENT_GREEN) } else { ("OFF", TEXT_2) };
                                if ui.add(
                                    egui::Button::new(RichText::new(label).size(10.0).color(color))
                                        .rounding(Rounding::same(2.0))
                                        .stroke(Stroke::new(1.0_f32, color))
                                ).clicked() {
                                    module.enabled = !module.enabled;
                                }
                            });
                        });

                        ui.label(RichText::new(format!("v{} \u{2014} {}", module.version, module.author)).size(10.0).color(TEXT_2));
                        ui.label(RichText::new(&module.description).size(10.0).color(TEXT_1));
                    });
                ui.add_space(2.0);
            }
        });
}
