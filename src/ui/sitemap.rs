use egui::{self, RichText, Color32, ScrollArea, Stroke, FontFamily};
use crate::models::*;
use crate::theme::*;

pub fn render(ui: &mut egui::Ui, nodes: &mut Vec<SiteMapNode>, search_query: &mut String) {
    // ── Top Toolbar: Title, Search Input Bar & Controls ─────────────
    ui.horizontal(|ui| {
        ui.label(RichText::new("Target Scope Tree").size(13.0).color(TEXT_0).strong());
        ui.add_space(12.0);

        // 🔎 Search / Filter Box
        ui.label(RichText::new("🔍 Search:").size(11.0).color(TEXT_1).strong());
        ui.add(
            egui::TextEdit::singleline(search_query)
                .hint_text("Filter target map (e.g. *site*, admin, api)...")
                .desired_width(260.0)
        );

        if !search_query.is_empty() {
            if ui.add(
                egui::Button::new(RichText::new("✖ Clear").size(10.0).color(TEXT_2))
                    .fill(BG_RAISED)
                    .stroke(Stroke::new(0.5_f32, BORDER))
            ).clicked() {
                search_query.clear();
            }
        }

        ui.add_space(12.0);
        let total_count = count_nodes(nodes);
        ui.label(RichText::new(format!("{} endpoints", total_count)).size(10.0).color(TEXT_2));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(RichText::new("Collapse All").size(10.0).color(TEXT_2))).clicked() {
                set_expanded_all(nodes, false);
            }
            ui.add_space(4.0);
            if ui.add(egui::Button::new(RichText::new("Expand All").size(10.0).color(ACCENT_BLUE))).clicked() {
                set_expanded_all(nodes, true);
            }
        });
    });

    ui.add_space(6.0);

    // Prepare clean search keyword
    let clean_query = search_query.trim().trim_matches('*').to_lowercase();

    // ── Sitemap Tree Rendering ─────────────────────────────────────
    section_frame().show(ui, |ui| {
        ScrollArea::vertical()
            .id_source("sitemap_tree_scroll")
            .show(ui, |ui| {
                if nodes.is_empty() {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No HTTP traffic captured yet.").size(12.0).color(TEXT_2));
                        ui.label(RichText::new("Browse websites through AJProxy (127.0.0.1:8080) to automatically build the target sitemap.").size(10.0).color(TEXT_2));
                    });
                    ui.add_space(40.0);
                } else {
                    let mut matched_any = false;
                    for node in nodes.iter_mut() {
                        if matches_filter(node, &clean_query) {
                            matched_any = true;
                            render_node(ui, node, 0, &clean_query);
                        }
                    }

                    if !clean_query.is_empty() && !matched_any {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(format!("No targets matching '{}'", search_query)).size(12.0).color(ACCENT_AMBER).strong());
                            ui.label(RichText::new("Try a different search term or clear the filter.").size(10.0).color(TEXT_2));
                        });
                        ui.add_space(30.0);
                    }
                }
            });
    });
}

fn matches_filter(node: &SiteMapNode, clean_query: &str) -> bool {
    if clean_query.is_empty() {
        return true;
    }
    if node.name.to_lowercase().contains(clean_query) || node.full_path.to_lowercase().contains(clean_query) {
        return true;
    }
    node.children.iter().any(|c| matches_filter(c, clean_query))
}

fn render_node(ui: &mut egui::Ui, node: &mut SiteMapNode, depth: usize, clean_query: &str) {
    let self_matches = clean_query.is_empty()
        || node.name.to_lowercase().contains(clean_query)
        || node.full_path.to_lowercase().contains(clean_query);

    // Auto-expand if search query matches children
    let force_expand = !clean_query.is_empty() && node.children.iter().any(|c| matches_filter(c, clean_query));
    let is_expanded = node.expanded || force_expand;

    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 20.0);

        if !node.children.is_empty() {
            let arrow = if is_expanded { "\u{25BC}" } else { "\u{25B6}" };
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

        let name_color = if !clean_query.is_empty() && self_matches {
            ACCENT_CYAN
        } else if node.in_scope {
            TEXT_0
        } else {
            TEXT_2
        };

        ui.label(RichText::new(&node.name).size(11.0).color(name_color).strong().family(FontFamily::Monospace));
        ui.checkbox(&mut node.in_scope, "");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{}", node.request_count)).size(10.0).color(TEXT_2).family(FontFamily::Monospace));
        });
    });

    if is_expanded {
        for child in node.children.iter_mut() {
            if matches_filter(child, clean_query) {
                render_node(ui, child, depth + 1, clean_query);
            }
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
