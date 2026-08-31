//! Comparer Diff Engine for AJProxy

use crate::models::{ComparerState, DiffMode};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone)]
pub struct DiffItem {
    pub tag: ChangeTag,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct DiffResult {
    pub left_items: Vec<DiffItem>,
    pub right_items: Vec<DiffItem>,
    pub added_count: usize,
    pub deleted_count: usize,
    pub match_percentage: f32,
}

pub fn compute_diff(state: &ComparerState) -> DiffResult {
    let mut text1 = state.left_text.clone();
    let mut text2 = state.right_text.clone();

    if state.ignore_case {
        text1 = text1.to_lowercase();
        text2 = text2.to_lowercase();
    }

    if state.ignore_whitespace {
        text1 = text1.split_whitespace().collect::<Vec<_>>().join(" ");
        text2 = text2.split_whitespace().collect::<Vec<_>>().join(" ");
    }

    match state.diff_mode {
        DiffMode::Lines => compute_line_diff(&text1, &text2),
        DiffMode::Words => compute_word_diff(&text1, &text2),
        DiffMode::Bytes => compute_byte_diff(&text1, &text2),
    }
}

fn compute_line_diff(text1: &str, text2: &str) -> DiffResult {
    let diff = TextDiff::from_lines(text1, text2);
    let mut left_items = Vec::new();
    let mut right_items = Vec::new();
    let mut added_count = 0;
    let mut deleted_count = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                deleted_count += 1;
                left_items.push(DiffItem {
                    tag: ChangeTag::Delete,
                    text: change.value().to_string(),
                });
            }
            ChangeTag::Insert => {
                added_count += 1;
                right_items.push(DiffItem {
                    tag: ChangeTag::Insert,
                    text: change.value().to_string(),
                });
            }
            ChangeTag::Equal => {
                left_items.push(DiffItem {
                    tag: ChangeTag::Equal,
                    text: change.value().to_string(),
                });
                right_items.push(DiffItem {
                    tag: ChangeTag::Equal,
                    text: change.value().to_string(),
                });
            }
        }
    }

    let total = added_count + deleted_count + left_items.len();
    let match_percentage = if total > 0 {
        ((left_items.len().saturating_sub(deleted_count)) as f32 / total as f32) * 100.0
    } else {
        100.0
    };

    DiffResult {
        left_items,
        right_items,
        added_count,
        deleted_count,
        match_percentage: match_percentage.clamp(0.0, 100.0),
    }
}

fn compute_word_diff(text1: &str, text2: &str) -> DiffResult {
    let diff = TextDiff::from_words(text1, text2);
    let mut left_items = Vec::new();
    let mut right_items = Vec::new();
    let mut added_count = 0;
    let mut deleted_count = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                deleted_count += 1;
                left_items.push(DiffItem {
                    tag: ChangeTag::Delete,
                    text: change.value().to_string(),
                });
            }
            ChangeTag::Insert => {
                added_count += 1;
                right_items.push(DiffItem {
                    tag: ChangeTag::Insert,
                    text: change.value().to_string(),
                });
            }
            ChangeTag::Equal => {
                left_items.push(DiffItem {
                    tag: ChangeTag::Equal,
                    text: change.value().to_string(),
                });
                right_items.push(DiffItem {
                    tag: ChangeTag::Equal,
                    text: change.value().to_string(),
                });
            }
        }
    }

    let total = added_count + deleted_count + left_items.len();
    let match_percentage = if total > 0 {
        ((left_items.len().saturating_sub(deleted_count)) as f32 / total as f32) * 100.0
    } else {
        100.0
    };

    DiffResult {
        left_items,
        right_items,
        added_count,
        deleted_count,
        match_percentage: match_percentage.clamp(0.0, 100.0),
    }
}

fn compute_byte_diff(text1: &str, text2: &str) -> DiffResult {
    let hex1 = hex::encode(text1.as_bytes());
    let hex2 = hex::encode(text2.as_bytes());
    let diff = TextDiff::from_chars(&hex1, &hex2);
    let mut left_items = Vec::new();
    let mut right_items = Vec::new();
    let mut added_count = 0;
    let mut deleted_count = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                deleted_count += 1;
                left_items.push(DiffItem {
                    tag: ChangeTag::Delete,
                    text: change.value().to_string(),
                });
            }
            ChangeTag::Insert => {
                added_count += 1;
                right_items.push(DiffItem {
                    tag: ChangeTag::Insert,
                    text: change.value().to_string(),
                });
            }
            ChangeTag::Equal => {
                left_items.push(DiffItem {
                    tag: ChangeTag::Equal,
                    text: change.value().to_string(),
                });
                right_items.push(DiffItem {
                    tag: ChangeTag::Equal,
                    text: change.value().to_string(),
                });
            }
        }
    }

    let total = added_count + deleted_count + left_items.len();
    let match_percentage = if total > 0 {
        ((left_items.len().saturating_sub(deleted_count)) as f32 / total as f32) * 100.0
    } else {
        100.0
    };

    DiffResult {
        left_items,
        right_items,
        added_count,
        deleted_count,
        match_percentage: match_percentage.clamp(0.0, 100.0),
    }
}
