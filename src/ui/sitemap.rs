use egui::{self, RichText, Color32, ScrollArea};
use crate::models::*;
use crate::theme::*;

pub fn render(ui: &mut egui::Ui, nodes: &mut Vec<SiteMapNode>) {
    // ── Controls ──────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(RichText::new("Target Scope Tree").size(11.0).color(TEXT_0).strong());
        ui.add_space(16.0);
        ui.label(RichText::new(format!("{} endpoints", count_nodes(nodes))).size(10.0).color(TEXT_2));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(RichText::new("Collapse").size(10.0).color(TEXT_2)).clicked() {
                set_expanded_all(nodes, false);
            }
            if ui.button(RichText::new("Expand").size(10.0).color(ACCENT_BLUE)).clicked() {
                set_expanded_all(nodes, true);
            }
        });
    });

    ui.add_space(4.0);

    // ── Tree ──────────────────────────────────────────────────────
    section_frame().show(ui, |ui| {
        ScrollArea::vertical()
            .id_source("sitemap_tree_scroll")
            .show(ui, |ui| {
                for node in nodes.iter_mut() {
                    render_node(ui, node, 0);
                }
            });
    });
}

fn render_node(ui: &mut egui::Ui, node: &mut SiteMapNode, depth: usize) {
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 20.0);

        if !node.children.is_empty() {
            let arrow = if node.expanded { "\u{25BC}" } else { "\u{25B6}" };
            if ui.add(
                egui::Button::new(RichText::new(arrow).size(9.0).color(TEXT_2))
                    .fill(Color32::TRANSPARENT)
                    .frame(false)
            ).clicked() {
                node.expanded = !node.expanded;
            }
        } else {
            ui.add_space(18.0);
        }

        let color = if node.in_scope { TEXT_0 } else { TEXT_2 };
        ui.label(RichText::new(&node.name).size(11.0).color(color).family(egui::FontFamily::Monospace));
        ui.checkbox(&mut node.in_scope, "");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{}", node.request_count)).size(10.0).color(TEXT_2).family(egui::FontFamily::Monospace));
        });
    });

    if node.expanded {
        for child in node.children.iter_mut() {
            render_node(ui, child, depth + 1);
        }
    }
}

fn count_nodes(nodes: &[SiteMapNode]) -> usize {
    nodes.iter().fold(nodes.len(), |acc, n| acc + count_nodes(&n.children))
}

fn set_expanded_all(nodes: &mut [SiteMapNode], expanded: bool) {
    for node in nodes.iter_mut() {
        node.expanded = expanded;
        set_expanded_all(&mut node.children, expanded);
    }
}
