use std::collections::HashSet;
use crate::models::{BruteForceState, BruteResult, PayloadSet};

/// Engine module for managing Intruder payloads, positions, persistence, and result filtering.

pub struct IntruderEngine;

impl IntruderEngine {
    /// Count number of §...§ marked positions in template
    pub fn count_positions(template: &str) -> usize {
        let count = template.matches('§').count();
        count / 2
    }

    /// Generate combinations of payloads for different attack types and position mappings
    pub fn build_combinations(
        attack_type: &crate::models::AttackType,
        template: &str,
        payload_sets: &[PayloadSet],
        pos_set_indices: &[usize],
    ) -> Vec<(String, String)> {
        let pos_count = Self::count_positions(template);
        if pos_count == 0 {
            return Vec::new();
        }

        // Helper to extract lines from a set index
        let get_set_lines = |idx: usize| -> Vec<String> {
            if let Some(set) = payload_sets.get(idx) {
                set.payloads_text
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            } else {
                Vec::new()
            }
        };

        let mut results = Vec::new();

        match attack_type {
            crate::models::AttackType::Sniper => {
                // For each position, substitute payloads from its assigned payload set while keeping other positions default
                for pos_idx in 0..pos_count {
                    let set_idx = pos_set_indices.get(pos_idx).cloned().unwrap_or(0);
                    let lines = get_set_lines(set_idx);
                    for payload in lines {
                        let label = format!("[Pos {}] {}", pos_idx + 1, payload);
                        results.push((label, payload));
                    }
                }
            }
            crate::models::AttackType::BatteringRam => {
                // All positions get identical payload from Position 1's assigned set
                let set_idx = pos_set_indices.get(0).cloned().unwrap_or(0);
                let lines = get_set_lines(set_idx);
                for payload in lines {
                    results.push((payload.clone(), payload));
                }
            }
            crate::models::AttackType::Pitchfork => {
                // Parallel 1-to-1 iteration across assigned sets for each position
                let mut lists: Vec<Vec<String>> = Vec::new();
                for pos_idx in 0..pos_count {
                    let set_idx = pos_set_indices.get(pos_idx).cloned().unwrap_or(pos_idx % payload_sets.len().max(1));
                    lists.push(get_set_lines(set_idx));
                }
                let min_len = lists.iter().map(|l| l.len()).min().unwrap_or(0);
                for i in 0..min_len {
                    let combined_payloads: Vec<String> = lists.iter().map(|l| l[i].clone()).collect();
                    let label = combined_payloads.join(" | ");
                    results.push((label, combined_payloads.first().cloned().unwrap_or_default()));
                }
            }
            crate::models::AttackType::ClusterBomb => {
                // Cartesian product across all assigned sets
                let mut lists: Vec<Vec<String>> = Vec::new();
                for pos_idx in 0..pos_count {
                    let set_idx = pos_set_indices.get(pos_idx).cloned().unwrap_or(pos_idx % payload_sets.len().max(1));
                    lists.push(get_set_lines(set_idx));
                }
                fn cartesian(lists: &[Vec<String>], depth: usize, current: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
                    if depth == lists.len() {
                        out.push(current.clone());
                        return;
                    }
                    for item in &lists[depth] {
                        current.push(item.clone());
                        cartesian(lists, depth + 1, current, out);
                        current.pop();
                    }
                }
                let mut combos = Vec::new();
                if !lists.is_empty() {
                    cartesian(&lists, 0, &mut Vec::new(), &mut combos);
                }
                for combo in combos {
                    let label = combo.join(" | ");
                    results.push((label, combo.first().cloned().unwrap_or_default()));
                }
            }
        }

        results
    }

    /// Add position marker § around current selection or append if no selection
    pub fn add_marker(template: &mut String, selection_start: usize, selection_end: usize) {
        if selection_start < selection_end && selection_end <= template.len() {
            let selected_text = &template[selection_start..selection_end];
            let new_text = format!("§{}§", selected_text);
            template.replace_range(selection_start..selection_end, &new_text);
        } else {
            template.push_str("§payload§");
        }
    }

    /// Clear all § markers from the template
    pub fn clear_markers(template: &mut String) {
        *template = template.replace('§', "");
    }

    /// Auto-detect parameters in HTTP request and wrap values with §
    pub fn auto_mark_params(template: &mut String) {
        let clean = template.replace('§', "");
        let lines: Vec<&str> = clean.lines().collect();
        let mut new_lines = Vec::new();
        let mut is_body = false;

        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                // Query string parameter detection (e.g., GET /path?param=value HTTP/1.1)
                if let Some((before_q, after_q)) = line.split_once('?') {
                    if let Some((query_str, proto)) = after_q.split_once(" HTTP/") {
                        let params: Vec<&str> = query_str.split('&').collect();
                        let mut new_params = Vec::new();
                        for param in params {
                            if let Some((k, v)) = param.split_once('=') {
                                new_params.push(format!("{}={}", k, format!("§{}§", v)));
                            } else {
                                new_params.push(param.to_string());
                            }
                        }
                        new_lines.push(format!("{}?{} HTTP/{}", before_q, new_params.join("&"), proto));
                        continue;
                    }
                }
                new_lines.push(line.to_string());
            } else if is_body {
                let trimmed = line.trim();
                if trimmed.contains("\": \"") || trimmed.contains("\":\"") {
                    // JSON value detection (e.g. "email": "user.a@example.com")
                    let mut result = String::new();
                    let parts: Vec<&str> = line.split("\": \"").collect();
                    if parts.len() > 1 {
                        for (idx, part) in parts.iter().enumerate() {
                            if idx == 0 {
                                result.push_str(part);
                            } else {
                                if let Some((val, rest)) = part.split_once('"') {
                                    result.push_str(&format!("\": \"§{}§\"{}", val, rest));
                                } else {
                                    result.push_str("\": \"");
                                    result.push_str(part);
                                }
                            }
                        }
                        new_lines.push(result);
                    } else {
                        new_lines.push(line.to_string());
                    }
                } else if line.contains('=') {
                    // Standard Form key=value detection
                    let parts: Vec<&str> = line.split('&').collect();
                    let mut new_parts = Vec::new();
                    for part in parts {
                        if let Some((k, v)) = part.split_once('=') {
                            new_parts.push(format!("{}={}", k, format!("§{}§", v)));
                        } else {
                            new_parts.push(part.to_string());
                        }
                    }
                    new_lines.push(new_parts.join("&"));
                } else {
                    new_lines.push(line.to_string());
                }
            } else {
                if line.is_empty() {
                    is_body = true;
                }
                new_lines.push(line.to_string());
            }
        }
        *template = new_lines.join("\n");
    }

    /// Filter attack results based on filter/ignore status code, length, or latency
    pub fn filter_results<'a>(state: &'a BruteForceState) -> Vec<&'a BruteResult> {
        let parse_codes = |s: &str| -> HashSet<u16> {
            s.split(',')
                .map(|item| item.trim())
                .filter_map(|item| item.parse::<u16>().ok())
                .collect()
        };

        let filter_codes = parse_codes(&state.filter_status_code);
        let ignore_codes = parse_codes(&state.ignore_status_code);

        state.results.iter().filter(|r| {
            // 1. Ignore filter check (Highest priority: if code is in ignore list, hide it)
            if !ignore_codes.is_empty() && ignore_codes.contains(&r.status_code) {
                return false;
            }

            // 2. Inclusion filter check (If filter set is not empty, only show codes in this list)
            if !filter_codes.is_empty() && !filter_codes.contains(&r.status_code) {
                return false;
            }

            // 3. Size / Length search filter
            if !state.search_filter.is_empty() {
                let query = state.search_filter.to_lowercase();
                let payload_match = r.payload.to_lowercase().contains(&query);
                let status_match = r.status_code.to_string().contains(&query);
                let len_match = r.length.to_string().contains(&query);
                if !payload_match && !status_match && !len_match {
                    return false;
                }
            }

            true
        }).collect()
    }
}
