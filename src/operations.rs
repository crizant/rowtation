//! Stable view ordering, bounded search and sparse transactional editing.
use crate::{
    document::{CellValue, Document, RowCells},
    error::EngineError,
};
use nsh_core::prelude::*;
use regex::RegexBuilder;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
    str::FromStr,
};

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_MATCHES: usize = 100_000;
#[derive(Default)]
pub(crate) struct Operations {
    pub order: Vec<usize>,
    sort: Option<(u32, bool)>,
    search_job: Option<SearchJob>,
    pub matches: Vec<SearchMatch>,
    pub matching_cells: HashSet<(u32, u32)>,
    originals: HashMap<(u32, u32), CellValue>,
    undo: VecDeque<Vec<Change>>,
    redo: Vec<Vec<Change>>,
}
#[derive(Clone)]
struct Change {
    row: u32,
    column: u32,
    before: CellValue,
    original: CellValue,
    after: CellValue,
}
/// Deterministic nonoverlapping Unicode matches. Empty patterns match nothing.
#[derive(JsObject, Default)]
pub struct SearchOptions {
    /// Literal text, or Rust regex syntax when regex is true. At most 4096 bytes.
    pub term: String,
    /// Null means every schema column; an ID restricts both search and replacement.
    pub column_id: Option<u32>,
    /// Defaults to Unicode simple case-insensitive matching.
    pub case_sensitive: Option<bool>,
    /// Rust regex syntax; no lookaround or backreferences. Defaults false.
    pub regex: Option<bool>,
}
/// Stable cell identity and UTF-8 byte range, valid at the returned revision.
#[derive(JsObject, Default, Clone)]
pub struct SearchMatch {
    pub row_id: u32,
    pub column_id: u32,
    pub start: u32,
    pub end: u32,
    pub revision: u32,
    /// Current view position, for revealing this identity, never for mutation.
    pub row_index: u32,
}
/// Search keeps matches in Rust; only one is returned for navigation.
#[derive(JsObject, Default)]
pub struct SearchResult {
    pub count: u32,
    pub current: Option<SearchMatch>,
}
/// Progress counts completed view rows; partial matches are never published.
#[derive(JsObject, Default)]
pub struct SearchProgress {
    pub scanned: u32,
    pub total: u32,
    /// Set only when the entire search has completed successfully.
    pub result: Option<SearchResult>,
}
struct SearchJob {
    regex: regex::Regex,
    column: Option<usize>,
    row: usize,
    cell: usize,
    offset: usize,
    last_end: Option<usize>,
    bytes: usize,
    charged: bool,
    found: Vec<SearchMatch>,
    indexed: usize,
    cells: HashSet<(u32, u32)>,
}
/// Counts describe occurrences, affected cells, and the resulting revision.
#[derive(JsObject, Default)]
pub struct MutationResult {
    pub affected_cells: u32,
    pub replacements: u32,
    pub revision: u32,
}
/// Small history/dirty snapshot, independent of grid windows.
#[derive(JsObject, Default)]
pub struct EditStatus {
    pub modified_cells: u32,
    pub can_undo: bool,
    pub can_redo: bool,
}
fn error(code: &'static str, message: &str) -> EngineError {
    EngineError::new(code, message)
}
fn text(value: &CellValue) -> Option<&str> {
    match value {
        CellValue::Text(s) | CellValue::Number(s) => Some(s),
        CellValue::Boolean(true) => Some("true"),
        CellValue::Boolean(false) => Some("false"),
        _ => None,
    }
}
fn numeric_key(value: &CellValue) -> Option<bigdecimal::BigDecimal> {
    let CellValue::Number(value) = value else {
        return None;
    };
    #[cfg(test)]
    NUMBER_PARSES.with(|count| count.set(count.get() + 1));
    bigdecimal::BigDecimal::from_str(value).ok()
}
#[cfg(test)]
thread_local! { static NUMBER_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

fn compare_keyed(
    a: &CellValue,
    b: &CellValue,
    a_key: Option<&bigdecimal::BigDecimal>,
    b_key: Option<&bigdecimal::BigDecimal>,
) -> Ordering {
    fn rank(v: &CellValue) -> u8 {
        match v {
            CellValue::Number(_) => 0,
            CellValue::Boolean(_) => 1,
            CellValue::Text(s) if !s.is_empty() => 2,
            CellValue::Text(_) => 3,
            CellValue::Null => 4,
            CellValue::Missing => 5,
        }
    }
    rank(a).cmp(&rank(b)).then_with(|| match (a, b) {
        (CellValue::Number(a), CellValue::Number(b)) => match (a_key, b_key) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.cmp(b),
        },
        (CellValue::Text(a), CellValue::Text(b)) => a.cmp(b),
        (CellValue::Boolean(a), CellValue::Boolean(b)) => a.cmp(b),
        _ => Ordering::Equal,
    })
}
impl Operations {
    pub fn status(&self) -> EditStatus {
        EditStatus {
            modified_cells: self.originals.len() as u32,
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }
    pub fn sort(
        &mut self,
        doc: &Document,
        column: Option<u32>,
        descending: bool,
    ) -> Result<(), EngineError> {
        if let Some(c) = column
            && !doc.columns().iter().any(|v| v.id() == c)
        {
            return Err(error("UNKNOWN_COLUMN", "Select a column."));
        }
        self.sort = column.map(|c| (c, descending));
        self.reorder(doc);
        self.search_job = None;
        self.matches.clear();
        self.matching_cells.clear();
        Ok(())
    }
    fn reorder(&mut self, doc: &Document) {
        let Some((c, descending)) = self.sort else {
            self.order = (0..doc.row_count()).collect();
            return;
        };
        // Allocate numeric keys only when this column actually contains numbers.
        // The immutable per-sort keys are dropped afterward; edits/undo naturally
        // rebuild them, and lexical CellValue payloads are never altered.
        let mut numbers: Option<Vec<Option<bigdecimal::BigDecimal>>> = None;
        self.order = (0..doc.row_count())
            .map(|i| {
                if let Some(value @ CellValue::Number(_)) = doc.rows[i].cells.get((c - 1) as usize)
                {
                    let keys =
                        numbers.get_or_insert_with(|| (0..doc.row_count()).map(|_| None).collect());
                    keys[i] = numeric_key(value);
                }
                i
            })
            .collect();
        {
            self.order.sort_by(|&a, &b| {
                let a_key = numbers.as_ref().and_then(|keys| keys[a].as_ref());
                let b_key = numbers.as_ref().and_then(|keys| keys[b].as_ref());
                let a = doc.rows[a]
                    .cells
                    .get((c - 1) as usize)
                    .unwrap_or(&CellValue::Missing);
                let b = doc.rows[b]
                    .cells
                    .get((c - 1) as usize)
                    .unwrap_or(&CellValue::Missing);
                let order = compare_keyed(a, b, a_key, b_key);
                if descending { order.reverse() } else { order }
            });
        }
    }
    pub fn search(
        &mut self,
        doc: &Document,
        options: SearchOptions,
    ) -> Result<SearchResult, EngineError> {
        self.start_search(doc, options)?;
        loop {
            if let Some(result) = self.search_step(doc, f64::INFINITY)?.result {
                return Ok(result);
            }
        }
    }
    pub fn cancel_search(&mut self) {
        self.search_job = None;
        self.matches.clear();
        self.matching_cells.clear();
    }
    pub fn start_search(
        &mut self,
        doc: &Document,
        options: SearchOptions,
    ) -> Result<(), EngineError> {
        self.cancel_search();
        let column_index = options
            .column_id
            .map(|id| {
                doc.columns()
                    .iter()
                    .position(|column| column.id() == id)
                    .ok_or_else(|| error("UNKNOWN_COLUMN", "Select a column."))
            })
            .transpose()?;
        if options.term.is_empty() {
            return Ok(());
        }
        if options.term.len() > 4096 {
            return Err(error("SEARCH_LIMIT", "Search term exceeds 4096 bytes."));
        }
        let pattern = if options.regex.unwrap_or(false) {
            options.term
        } else {
            regex::escape(&options.term)
        };
        let regex = RegexBuilder::new(&pattern)
            .case_insensitive(!options.case_sensitive.unwrap_or(false))
            .size_limit(1 << 20)
            .dfa_size_limit(1 << 20)
            .build()
            .map_err(|e| error("INVALID_REGEX", &e.to_string()))?;
        self.search_job = Some(SearchJob {
            regex,
            column: column_index,
            row: 0,
            cell: 0,
            offset: 0,
            last_end: None,
            bytes: 0,
            charged: false,
            found: std::mem::take(&mut self.matches),
            indexed: 0,
            cells: std::mem::take(&mut self.matching_cells),
        });
        Ok(())
    }
    /// Cooperatively scan for the requested milliseconds. A single regex search
    /// is atomic; deadlines are checked between cells and occurrences, not by
    /// truncating haystacks (which would change anchors and greedy matches).
    pub fn search_step(
        &mut self,
        doc: &Document,
        milliseconds: f64,
    ) -> Result<SearchProgress, EngineError> {
        let total = doc.row_count() as u32;
        let Some(mut job) = self.search_job.take() else {
            return Ok(SearchProgress {
                scanned: total,
                total,
                result: Some(SearchResult {
                    count: self.matches.len() as u32,
                    current: self.matches.first().cloned(),
                }),
            });
        };
        let deadline = crate::diagnostics::now_ms() + milliseconds.max(0.0);
        let mut work = 0usize;
        while job.row < doc.row_count() {
            // Amortize clock/bridge costs while bounding even empty/sparse rows.
            if work >= 64 {
                work = 0;
                if milliseconds.is_finite() && crate::diagnostics::now_ms() >= deadline {
                    let scanned = job.row as u32;
                    self.search_job = Some(job);
                    return Ok(SearchProgress {
                        scanned,
                        total,
                        result: None,
                    });
                }
            }
            work += 1;
            let row = &doc.rows[self.order.get(job.row).copied().unwrap_or(job.row)];
            let entry = if let Some(column) = job.column {
                if job.cell == 0 {
                    row.cells.get(column).map(|v| (column, v))
                } else {
                    None
                }
            } else {
                match &row.cells {
                    RowCells::Dense(values) => values.get(job.cell).map(|v| (job.cell, v)),
                    RowCells::Sparse(values) => values.get(job.cell).map(|(c, v)| (*c, v)),
                }
            };
            let Some((column, cell)) = entry else {
                job.row += 1;
                job.cell = 0;
                continue;
            };
            if let Some(value) = text(cell) {
                if !job.charged {
                    job.charged = true;
                    job.bytes = job.bytes.saturating_add(value.len());
                    if job.bytes > 64 * 1024 * 1024 {
                        return Err(error(
                            "SEARCH_LIMIT",
                            "Search scope exceeds 64 MiB. Select a smaller column.",
                        ));
                    }
                }
                let resumed = job.last_end.is_some();
                let mut iter = (!resumed).then(|| job.regex.find_iter(value));
                loop {
                    if work >= 64 {
                        work = 0;
                        if milliseconds.is_finite() && crate::diagnostics::now_ms() >= deadline {
                            let scanned = job.row as u32;
                            drop(iter);
                            self.search_job = Some(job);
                            return Ok(SearchProgress {
                                scanned,
                                total,
                                result: None,
                            });
                        }
                    }
                    work += 1;
                    let m = if resumed {
                        if job.offset > value.len() {
                            None
                        } else {
                            job.regex.find_at(value, job.offset)
                        }
                    } else {
                        iter.as_mut().unwrap().next()
                    };
                    let Some(m) = m else { break };
                    // find_iter already handles adjacent empty matches. On a
                    // resumed cell, preserve its end sentinel and UTF-8 advance.
                    if resumed && m.is_empty() && job.last_end == Some(m.end()) {
                        job.offset =
                            m.end() + value[m.end()..].chars().next().map_or(1, char::len_utf8);
                        continue;
                    }
                    if job.found.len() >= MAX_MATCHES {
                        return Err(error(
                            "SEARCH_LIMIT",
                            "More than 100,000 matches. Narrow the search.",
                        ));
                    }
                    job.found.push(SearchMatch {
                        row_id: row.id(),
                        column_id: doc.columns()[column].id(),
                        start: m.start() as u32,
                        end: m.end() as u32,
                        revision: doc.revision(),
                        row_index: job.row as u32,
                    });
                    job.offset = m.end();
                    job.last_end = Some(m.end());
                }
            }
            job.cell += 1;
            job.offset = 0;
            job.last_end = None;
            job.charged = false;
        }
        // Reserve once, as the original collect did. Indexing is also sliced;
        // publishing never hides a second O(matches) pass in the final frame.
        if job.indexed == 0 {
            job.cells.reserve(job.found.len());
        }
        while job.indexed < job.found.len() {
            if work >= 64 {
                work = 0;
                if milliseconds.is_finite() && crate::diagnostics::now_ms() >= deadline {
                    self.search_job = Some(job);
                    return Ok(SearchProgress {
                        scanned: total,
                        total,
                        result: None,
                    });
                }
            }
            work += 1;
            let m = &job.found[job.indexed];
            job.cells.insert((m.row_id, m.column_id));
            job.indexed += 1;
        }
        self.matches = job.found;
        self.matching_cells = job.cells;
        Ok(SearchProgress {
            scanned: total,
            total,
            result: Some(SearchResult {
                count: self.matches.len() as u32,
                current: self.matches.first().cloned(),
            }),
        })
    }
    pub fn match_at(&self, index: u32) -> Option<SearchMatch> {
        self.matches.get(index as usize).cloned()
    }
    /// The retained index of this occurrence. `hint` is one comparison; the
    /// rest of the list is scanned in Rust, not across the JS bridge.
    pub fn find_match(&self, row_id: u32, column_id: u32, start: u32, hint: u32) -> Option<u32> {
        let same =
            |m: &SearchMatch| m.row_id == row_id && m.column_id == column_id && m.start == start;
        if self.matches.get(hint as usize).is_some_and(same) {
            return Some(hint);
        }
        self.matches.iter().position(same).map(|i| i as u32)
    }
    /// The matched slice, or empty when it sits inside the `preview_bytes` prefix.
    /// At most 512 bytes, on a character boundary. The rest of the cell stays in Rust.
    pub fn snippet(&self, doc: &Document, index: u32, preview_bytes: usize) -> Option<String> {
        let m = self.matches.get(index as usize)?;
        let value = text(doc.cell(m.row_id, m.column_id).ok()?)?;
        let start = m.start as usize;
        let end = m.end as usize;
        if end > value.len()
            || start > end
            || !value.is_char_boundary(start)
            || !value.is_char_boundary(end)
        {
            return None;
        }
        if end <= crate::session::prefix(value, preview_bytes).len() {
            return Some(String::new());
        }
        Some(crate::session::prefix(&value[start..end], 512).to_owned())
    }
    pub fn edit(
        &mut self,
        doc: &mut Document,
        row: u32,
        column: u32,
        value: String,
        revision: u32,
    ) -> Result<MutationResult, EngineError> {
        self.check(doc, revision)?;
        let before = doc.cell(row, column)?.clone();
        let after = if text(&before) == Some(value.as_str()) {
            before.clone()
        } else {
            CellValue::Text(value)
        };
        self.commit(
            doc,
            vec![Change {
                row,
                column,
                original: self
                    .originals
                    .get(&(row, column))
                    .unwrap_or(&before)
                    .clone(),
                before,
                after,
            }],
            0,
        )
    }
    fn check(&self, doc: &Document, revision: u32) -> Result<(), EngineError> {
        if revision != doc.revision() {
            Err(error(
                "STALE_REVISION",
                "Document changed. Retry the operation.",
            ))
        } else {
            Ok(())
        }
    }
    pub fn replace(
        &mut self,
        doc: &mut Document,
        index: Option<u32>,
        replacement: String,
        revision: u32,
    ) -> Result<MutationResult, EngineError> {
        self.check(doc, revision)?;
        let matches: &[SearchMatch] = if let Some(i) = index {
            self.matches
                .get(i as usize..i as usize + 1)
                .ok_or_else(|| error("UNKNOWN_MATCH", "Search again before replacing."))?
        } else {
            &self.matches
        };
        let mut changes = Vec::new();
        let mut count = 0;
        for m in matches {
            if m.revision != revision {
                return Err(error("STALE_REVISION", "Search again before replacing."));
            }
            if changes
                .last()
                .is_none_or(|c: &Change| c.row != m.row_id || c.column != m.column_id)
            {
                let before = doc.cell(m.row_id, m.column_id)?.clone();
                changes.push(Change {
                    row: m.row_id,
                    column: m.column_id,
                    original: self
                        .originals
                        .get(&(m.row_id, m.column_id))
                        .unwrap_or(&before)
                        .clone(),
                    after: before.clone(),
                    before,
                });
            }
            count += 1;
        }
        // Groups are contiguous in view/column order; visit each occurrence once.
        let mut offset = 0;
        let mut total_bytes = 0usize;
        for change in &mut changes {
            let group_len = matches[offset..]
                .iter()
                .take_while(|m| m.row_id == change.row && m.column_id == change.column)
                .count();
            let group = &matches[offset..offset + group_len];
            offset += group_len;
            let before = text(&change.before).unwrap_or("");
            let removed: usize = group.iter().map(|m| (m.end - m.start) as usize).sum();
            let added = replacement
                .len()
                .checked_mul(group_len)
                .ok_or_else(|| error("EDIT_LIMIT", "Replacement too large."))?;
            total_bytes = total_bytes
                .saturating_add(text(&change.original).map_or(0, str::len))
                .saturating_add(before.len())
                .saturating_add(before.len() - removed)
                .saturating_add(added)
                .saturating_add(64);
            if total_bytes > MAX_BYTES {
                return Err(error(
                    "EDIT_LIMIT",
                    "Change exceeds the 16 MiB transaction limit.",
                ));
            }
            let mut value = String::with_capacity(before.len() - removed + added);
            let mut start = 0;
            for m in group {
                value.push_str(&before[start..m.start as usize]);
                value.push_str(&replacement);
                start = m.end as usize;
            }
            value.push_str(&before[start..]);
            change.after = if value == before {
                change.before.clone()
            } else {
                CellValue::Text(value)
            };
        }
        self.commit(doc, changes, count)
    }
    fn commit(
        &mut self,
        doc: &mut Document,
        mut changes: Vec<Change>,
        count: u32,
    ) -> Result<MutationResult, EngineError> {
        changes.retain(|c| c.before != c.after);
        if cost(&changes) > MAX_BYTES {
            return Err(error(
                "EDIT_LIMIT",
                "Change exceeds the 16 MiB transaction limit.",
            ));
        }
        let result = MutationResult {
            affected_cells: changes.len() as u32,
            replacements: count,
            revision: doc
                .revision()
                .checked_add(u32::from(!changes.is_empty()))
                .ok_or_else(|| error("REVISION_LIMIT", "Reopen the document."))?,
        };
        if changes.is_empty() {
            return Ok(result);
        }
        self.apply(doc, &changes, false)?;
        self.redo.clear();
        self.undo.push_back(changes);
        while self.undo.len() > 50
            || self.undo.iter().map(|c| cost(c)).sum::<usize>() > 32 * 1024 * 1024
        {
            self.undo.pop_front();
        }
        Ok(result)
    }
    fn apply(
        &mut self,
        doc: &mut Document,
        changes: &[Change],
        reverse: bool,
    ) -> Result<(), EngineError> {
        let revision = doc
            .revision()
            .checked_add(1)
            .ok_or_else(|| error("REVISION_LIMIT", "Reopen the document before continuing."))?;
        for c in changes {
            let value = if reverse { &c.before } else { &c.after };
            let original = self
                .originals
                .entry((c.row, c.column))
                .or_insert_with(|| c.original.clone());
            let clean = value == original;
            let cells = &mut doc.rows[(c.row - 1) as usize].cells;
            cells.set((c.column - 1) as usize, value.clone());
            if clean {
                self.originals.remove(&(c.row, c.column));
            }
        }
        doc.revision = revision;
        doc.dirty = !self.originals.is_empty();
        self.search_job = None;
        self.matches.clear();
        self.matching_cells.clear();
        if self
            .sort
            .is_some_and(|(column, _)| changes.iter().any(|c| c.column == column))
        {
            self.reorder(doc);
        }
        Ok(())
    }
    pub fn history(
        &mut self,
        doc: &mut Document,
        redo: bool,
    ) -> Result<MutationResult, EngineError> {
        if doc.revision() == u32::MAX {
            return Err(error("REVISION_LIMIT", "Reopen the document."));
        }
        let changes = if redo {
            self.redo.pop()
        } else {
            self.undo.pop_back()
        };
        let Some(changes) = changes else {
            return Ok(MutationResult {
                revision: doc.revision(),
                ..Default::default()
            });
        };
        self.apply(doc, &changes, !redo)?;
        let result = MutationResult {
            affected_cells: changes.len() as u32,
            revision: doc.revision(),
            replacements: 0,
        };
        if redo {
            self.undo.push_back(changes);
        } else {
            self.redo.push(changes);
        }
        Ok(result)
    }
}
fn cost(changes: &[Change]) -> usize {
    changes
        .iter()
        .map(|c| {
            text(&c.original).map_or(0, str::len)
                + text(&c.before).map_or(0, str::len)
                + text(&c.after).map_or(0, str::len)
                + 64
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn compare(a: &CellValue, b: &CellValue) -> Ordering {
        compare_keyed(a, b, numeric_key(a).as_ref(), numeric_key(b).as_ref())
    }
    #[test]
    fn sliced_search_matches_find_iter_including_resumed_utf8_and_empty_ranges() {
        let value = "a雪 café CAFÉ\n".repeat(1500);
        let input = format!("value\n\"{}\"", value);
        let doc = crate::formats::delimited::parse(input.as_bytes(), ",", true).unwrap();
        for pattern in [
            "a*",
            ".*",
            "(?s).*",
            "^|$",
            "(?m)^|$",
            "\\b",
            "雪|café",
            "a|",
            "(?:)",
        ] {
            let regex = RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .unwrap();
            let expected = regex
                .find_iter(&value)
                .map(|m| (m.start() as u32, m.end() as u32))
                .collect::<Vec<_>>();
            let mut ops = Operations::default();
            ops.start_search(
                &doc,
                SearchOptions {
                    term: pattern.into(),
                    regex: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();
            let mut previous = 0;
            loop {
                let progress = ops.search_step(&doc, 0.0).unwrap();
                assert!(progress.scanned >= previous);
                previous = progress.scanned;
                if let Some(result) = progress.result {
                    assert_eq!(result.count as usize, expected.len(), "{pattern}");
                    break;
                }
                assert!(ops.matches.is_empty());
                assert!(ops.matching_cells.is_empty());
            }
            assert_eq!(
                ops.matches
                    .iter()
                    .map(|m| (m.start, m.end))
                    .collect::<Vec<_>>(),
                expected,
                "{pattern}"
            );
            assert!(ops.matching_cells.contains(&(1, 1)));
        }
    }
    #[test]
    fn cancellation_restart_sort_and_mutation_discard_partial_search() {
        let input = format!("value\n{}", "hit hit\n".repeat(50_000));
        let mut doc = crate::formats::delimited::parse(input.as_bytes(), ",", true).unwrap();
        let mut ops = Operations::default();
        let options = || SearchOptions {
            term: "hit".into(),
            ..Default::default()
        };
        ops.start_search(&doc, options()).unwrap();
        assert!(ops.search_step(&doc, 0.0).unwrap().result.is_none());
        assert!(!ops.search_job.as_ref().unwrap().found.is_empty());
        ops.cancel_search();
        assert!(ops.search_job.is_none());
        assert_eq!(ops.search_step(&doc, 0.0).unwrap().result.unwrap().count, 0);
        ops.start_search(&doc, options()).unwrap();
        ops.search_step(&doc, 0.0).unwrap();
        ops.start_search(
            &doc,
            SearchOptions {
                term: "absent".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            ops.search_step(&doc, f64::INFINITY)
                .unwrap()
                .result
                .unwrap()
                .count,
            0
        );
        ops.start_search(&doc, options()).unwrap();
        ops.search_step(&doc, 0.0).unwrap();
        ops.sort(&doc, Some(1), true).unwrap();
        assert!(ops.search_job.is_none());
        ops.start_search(&doc, options()).unwrap();
        ops.search_step(&doc, 0.0).unwrap();
        ops.edit(&mut doc, 1, 1, "changed".into(), 0).unwrap();
        assert!(ops.search_job.is_none());
        assert!(ops.matches.is_empty());
        assert_eq!(ops.search(&doc, options()).unwrap().count, 99_998);
        ops.start_search(&doc, options()).unwrap();
        assert!(
            ops.start_search(
                &doc,
                SearchOptions {
                    term: "[".into(),
                    regex: Some(true),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert!(ops.search_job.is_none());
        assert!(ops.matches.is_empty());
    }
    #[test]
    fn slices_charge_each_cell_once_and_limits_never_publish_partial_hits() {
        // Many slices within a single cell must not count its payload repeatedly.
        let input = format!("value\n{}", "a".repeat(100_000));
        let doc = crate::formats::delimited::parse(input.as_bytes(), ",", true).unwrap();
        let mut ops = Operations::default();
        let options = || SearchOptions {
            term: "a".into(),
            ..Default::default()
        };
        ops.start_search(&doc, options()).unwrap();
        loop {
            let progress = ops.search_step(&doc, 0.0).unwrap();
            if let Some(job) = &ops.search_job {
                assert_eq!(job.bytes, 100_000);
            }
            if let Some(result) = progress.result {
                assert_eq!(result.count, 100_000);
                break;
            }
        }
        let input = format!("value\n{}", "a".repeat(100_001));
        let doc = crate::formats::delimited::parse(input.as_bytes(), ",", true).unwrap();
        ops.start_search(&doc, options()).unwrap();
        loop {
            match ops.search_step(&doc, 0.0) {
                Ok(progress) => assert!(progress.result.is_none()),
                Err(error) => {
                    assert_eq!(error.code(), "SEARCH_LIMIT");
                    break;
                }
            }
        }
        assert!(ops.search_job.is_none());
        assert!(ops.matches.is_empty());
        assert!(ops.matching_cells.is_empty());
    }
    #[test]
    fn search_budget_counts_searchable_payload_only_at_the_boundary() {
        use crate::document::{Column, Row};
        for sparse in [false, true] {
            let values = vec![
                CellValue::Text("x".repeat(64 * 1024 * 1024)),
                CellValue::Missing,
                CellValue::Null,
                CellValue::Text(String::new()),
                CellValue::Boolean(false),
            ];
            let cells = if sparse {
                RowCells::Sparse(
                    values
                        .into_iter()
                        .enumerate()
                        .filter(|(_, value)| *value != CellValue::Missing)
                        .collect(),
                )
            } else {
                RowCells::Dense(values)
            };
            let columns = (1..=5)
                .map(|id| Column {
                    id,
                    name: id.to_string(),
                    original_name: None,
                })
                .collect();
            let mut doc = Document::new(columns, vec![Row { id: 1, cells }], b',', false).unwrap();
            let mut ops = Operations::default();
            ops.sort(&doc, None, false).unwrap();
            let options = |column| SearchOptions {
                term: "false".into(),
                column_id: column,
                ..Default::default()
            };
            assert_eq!(ops.search(&doc, options(Some(5))).unwrap().count, 1);
            assert_eq!(
                ops.search(&doc, options(None)).err().unwrap().code(),
                "SEARCH_LIMIT"
            );
            assert!(ops.matches.is_empty());
            assert!(ops.matching_cells.is_empty());
            doc.rows[0].cells.set(4, CellValue::Missing);
            // Exactly 64 MiB remains: gaps, Null and empty Text must add no bytes.
            assert_eq!(ops.search(&doc, options(None)).unwrap().count, 0);
            assert_eq!(
                ops.search(
                    &doc,
                    SearchOptions {
                        term: "^$".into(),
                        regex: Some(true),
                        ..Default::default()
                    }
                )
                .unwrap()
                .count,
                1
            );
        }
    }
    #[test]
    fn numeric_sort_parses_each_key_once() {
        let input = (0..2000)
            .map(|i| format!("{{\"n\":{}.000000000000000000001}}\n", (i * 7919) % 2000))
            .collect::<String>();
        let doc = crate::formats::structured::parse(input.as_bytes(), "ndjson").unwrap();
        let mut ops = Operations::default();
        NUMBER_PARSES.with(|count| count.set(0));
        ops.sort(&doc, Some(1), false).unwrap();
        NUMBER_PARSES.with(|count| assert_eq!(count.get(), 2000));
        assert_eq!(ops.order[0], 0);
        ops.sort(&doc, Some(1), true).unwrap();
        NUMBER_PARSES.with(|count| assert_eq!(count.get(), 4000));
    }
    #[test]
    fn mixed_types_and_exact_numeric_comparison() {
        let n = |s: &str| CellValue::Number(s.into());
        assert_eq!(
            compare(&n("9007199254740993"), &n("9007199254740992")),
            Ordering::Greater
        );
        assert_eq!(compare(&n("1.00"), &n("1")), Ordering::Equal);
        assert_eq!(compare(&n("-10"), &n("-2")), Ordering::Less);
        assert_eq!(
            compare(&n("2"), &CellValue::Text("001".into())),
            Ordering::Less
        );
        assert_eq!(
            compare(&CellValue::Text("".into()), &CellValue::Null),
            Ordering::Less
        );
        assert_eq!(
            compare(&CellValue::Null, &CellValue::Missing),
            Ordering::Less
        );
    }
}
