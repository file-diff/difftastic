use std::cmp::PartialEq;
use std::collections::BTreeMap;

use line_numbers::LineNumber;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::display::context::{all_matched_lines_filled, opposite_positions};
use crate::display::hunks::{matched_lines_indexes_for_hunk, matched_pos_to_hunks, merge_adjacent};
use crate::display::side_by_side::lines_with_novel;
use crate::lines::MaxLine;
use crate::parse::syntax::{self, MatchedPos};
use crate::summary::{DiffResult, FileContent, FileFormat};

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Unchanged,
    Changed,
    Created,
    Deleted,
}

#[derive(Debug)]
struct File<'f> {
    language: &'f FileFormat,
    path: &'f str,
    aligned_lines: Vec<(Option<u32>, Option<u32>)>,
    chunks: Vec<Vec<Line>>,
    status: Status,
}

impl<'f> File<'f> {
    fn with_sections(
        language: &'f FileFormat,
        path: &'f str,
        aligned_lines: Vec<(Option<u32>, Option<u32>)>,
        chunks: Vec<Vec<Line>>,
    ) -> Self {
        File {
            language,
            path,
            aligned_lines,
            chunks,
            status: Status::Changed,
        }
    }

    fn with_status(language: &'f FileFormat, path: &'f str, status: Status) -> Self {
        File {
            language,
            path,
            aligned_lines: Vec::new(),
            chunks: Vec::new(),
            status,
        }
    }
}

impl<'f> From<&'f DiffResult> for File<'f> {
    fn from(summary: &'f DiffResult) -> Self {
        match (&summary.lhs_src, &summary.rhs_src) {
            (FileContent::Text(lhs_src), FileContent::Text(rhs_src)) => {
                // TODO: move into function as it is effectively duplicates lines 365-375 of main::print_diff_result
                let opposite_to_lhs = opposite_positions(&summary.lhs_positions);
                let opposite_to_rhs = opposite_positions(&summary.rhs_positions);

                let hunks = matched_pos_to_hunks(&summary.lhs_positions, &summary.rhs_positions);
                let hunks = merge_adjacent(
                    &hunks,
                    &opposite_to_lhs,
                    &opposite_to_rhs,
                    lhs_src.max_line(),
                    rhs_src.max_line(),
                    0,
                );

                if hunks.is_empty() {
                    return File::with_status(
                        &summary.file_format,
                        &summary.display_path,
                        Status::Unchanged,
                    );
                }

                if lhs_src.is_empty() {
                    return File::with_status(
                        &summary.file_format,
                        &summary.display_path,
                        Status::Created,
                    );
                }
                if rhs_src.is_empty() {
                    return File::with_status(
                        &summary.file_format,
                        &summary.display_path,
                        Status::Deleted,
                    );
                }

                let lhs_lines = lhs_src.split('\n').collect::<Vec<&str>>();
                let rhs_lines = rhs_src.split('\n').collect::<Vec<&str>>();

                let (lhs_lines_with_novel, rhs_lines_with_novel) =
                    lines_with_novel(&summary.lhs_positions, &summary.rhs_positions);

                let matched_lines = all_matched_lines_filled(
                    &summary.lhs_positions,
                    &summary.rhs_positions,
                    &lhs_lines,
                    &rhs_lines,
                );

                // Convert alignment to serializable format (Option<u32>, Option<u32>)
                let aligned_lines: Vec<(Option<u32>, Option<u32>)> = matched_lines
                    .iter()
                    .map(|(lhs, rhs)| (lhs.map(|l| l.0), rhs.map(|l| l.0)))
                    .collect();

                let mut matched_lines = &matched_lines[..];

                let mut chunks = Vec::with_capacity(hunks.len());
                for hunk in &hunks {
                    let mut lines = BTreeMap::new();

                    let (start_i, end_i) = matched_lines_indexes_for_hunk(matched_lines, hunk, 0);
                    let aligned_lines = &matched_lines[start_i..end_i];
                    matched_lines = &matched_lines[start_i..];

                    for &(lhs_line_num, rhs_line_num) in
                        aligned_lines.iter().filter(|(lhs, rhs)| {
                            let has_changes = lhs_lines_with_novel
                                .contains(&lhs.unwrap_or(LineNumber(0)))
                                || rhs_lines_with_novel.contains(&rhs.unwrap_or(LineNumber(0)));
                            has_changes && lhs.is_some() && rhs.is_some()
                        })
                    {
                        let (lhs_num, rhs_num) = match (lhs_line_num, rhs_line_num) {
                            (Some(lhs_num), Some(rhs_num)) => (lhs_num, rhs_num),
                            _ => continue,
                        };

                        let line = lines
                            .entry((Some(lhs_num.0), Some(rhs_num.0)))
                            .or_insert_with(|| Line::new(Some(lhs_num.0), Some(rhs_num.0)));

                        add_changes_to_side(
                            line.lhs.as_mut().unwrap(),
                            lhs_num,
                            &summary.lhs_positions,
                        );
                        add_changes_to_side(
                            line.rhs.as_mut().unwrap(),
                            rhs_num,
                            &summary.rhs_positions,
                        );
                    }
                    chunks.push(lines.into_values().collect());
                }

                File::with_sections(
                    &summary.file_format,
                    &summary.display_path,
                    aligned_lines,
                    chunks,
                )
            }
            (FileContent::Binary, FileContent::Binary) => {
                let status = if summary.has_byte_changes.is_some() {
                    Status::Changed
                } else {
                    Status::Unchanged
                };
                File::with_status(&FileFormat::Binary, &summary.display_path, status)
            }
            (_, FileContent::Binary) | (FileContent::Binary, _) => {
                File::with_status(&FileFormat::Binary, &summary.display_path, Status::Changed)
            }
        }
    }
}

impl Serialize for File<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Count fields: language, path, status are always present (3)
        // + aligned_lines if not empty (1)
        // + chunks if not empty (1)
        let field_count =
            3 + (!self.aligned_lines.is_empty()) as usize + (!self.chunks.is_empty()) as usize;

        let mut file = serializer.serialize_struct("File", field_count)?;

        if !self.aligned_lines.is_empty() {
            file.serialize_field("aligned_lines", &self.aligned_lines)?;
        }
        if !self.chunks.is_empty() {
            file.serialize_field("chunks", &self.chunks)?;
        }

        file.serialize_field("language", &format!("{}", self.language))?;
        file.serialize_field("path", &self.path)?;
        file.serialize_field("status", &self.status)?;

        file.end()
    }
}

#[derive(Debug, Serialize)]
struct Line {
    #[serde(skip_serializing_if = "Option::is_none")]
    lhs: Option<Side>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rhs: Option<Side>,
}

impl Line {
    fn new(lhs_number: Option<u32>, rhs_number: Option<u32>) -> Self {
        Line {
            lhs: lhs_number.map(Side::new),
            rhs: rhs_number.map(Side::new),
        }
    }
}

#[derive(Debug, Serialize)]
struct Side {
    line_number: u32,
    changes: Vec<Change>,
}

impl Side {
    fn new(line_number: u32) -> Self {
        Side {
            line_number,
            changes: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Copy, Clone)]
struct Change {
    start: u32,
    end: u32,
    highlight: Highlight,
}

#[derive(Debug, Serialize, Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
// TODO: use syntax::TokenKind and syntax::AtomKind instead of this merged enum,
// blocked by https://github.com/serde-rs/serde/issues/1402
enum Highlight {
    Ignored,
    Unchanged,
    Novel,
    NovelWord,
    NovelUnchanged,
}

impl Highlight {
    fn from_match(kind: &syntax::MatchKind) -> Self {
        use syntax::MatchKind;

        match kind {
            MatchKind::Ignored { .. } => Highlight::Ignored,
            MatchKind::UnchangedToken { .. } => Highlight::Unchanged,
            MatchKind::Novel { .. } => Highlight::Novel,
            MatchKind::NovelWord { .. } => Highlight::NovelWord,
            MatchKind::UnchangedPartOfNovelItem { .. } => Highlight::NovelUnchanged,
        }
    }
}

pub(crate) fn print_directory(diffs: Vec<DiffResult>, print_unchanged: bool) {
    let files = diffs
        .iter()
        .map(File::from)
        .filter(|f| print_unchanged || f.status != Status::Unchanged)
        .collect::<Vec<File>>();
    println!(
        "{}",
        serde_json::to_string(&files).expect("failed to serialize files")
    );
}

pub(crate) fn print(diff: &DiffResult) {
    let file = File::from(diff);
    println!(
        "{}",
        serde_json::to_string_pretty(&file).expect("failed to serialize file")
    )
}

fn add_changes_to_side(side: &mut Side, line_num: LineNumber, all_matches: &[MatchedPos]) {
    for m in matches_for_line(all_matches, line_num) {
        let highlight = Highlight::from_match(&m.kind);
        let start = m.pos.start_col;
        let end = m.pos.end_col;

        // Check the last change pushed to the side. If it's adjacent/overlapping
        // and shares the same highlight, extend it instead of pushing a new one.
        if let Some(last) = side.changes.last_mut() {
            if last.highlight == highlight && start <= last.end {
                last.end = last.end.max(end);
                continue;
            }
        }

        // Otherwise, add it as a new distinct change
        side.changes.push(Change {
            start,
            end,
            highlight,
        });
    }
}

fn matches_for_line(matches: &[MatchedPos], line_num: LineNumber) -> Vec<&MatchedPos> {
    matches
        .iter()
        .filter(|m| m.pos.line == line_num)
        .filter(|m| m.kind.is_novel())
        .collect()
}
