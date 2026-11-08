//! Stable view ordering, bounded search and sparse transactional editing.
use crate::{
    document::{CellValue, Document},
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
fn compare(a: &CellValue, b: &CellValue) -> Ordering {
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
        (CellValue::Number(a), CellValue::Number(b)) => match (
            bigdecimal::BigDecimal::from_str(a),
            bigdecimal::BigDecimal::from_str(b),
        ) {
            (Ok(a), Ok(b)) => a.cmp(&b),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => a.cmp(b),
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
        self.matches.clear();
        self.matching_cells.clear();
        Ok(())
    }
    fn reorder(&mut self, doc: &Document) {
        self.order = (0..doc.row_count()).collect();
        if let Some((c, descending)) = self.sort {
            self.order.sort_by(|&a, &b| {
                let a = doc.rows[a]
                    .cells
                    .get((c - 1) as usize)
                    .unwrap_or(&CellValue::Missing);
                let b = doc.rows[b]
                    .cells
                    .get((c - 1) as usize)
                    .unwrap_or(&CellValue::Missing);
                let order = compare(a, b);
                if descending { order.reverse() } else { order }
            });
        }
    }
    pub fn search(
        &mut self,
        doc: &Document,
        options: SearchOptions,
    ) -> Result<SearchResult, EngineError> {
        self.matches.clear();
        self.matching_cells.clear();
        if let Some(c) = options.column_id
            && !doc.columns().iter().any(|v| v.id() == c)
        {
            return Err(error("UNKNOWN_COLUMN", "Select a column."));
        }
        if options.term.is_empty() {
            return Ok(SearchResult::default());
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
        let mut found = Vec::new();
        let mut scanned = 0usize;
        let columns: Vec<_> = doc
            .columns()
            .iter()
            .filter(|c| options.column_id.is_none_or(|id| id == c.id()))
            .collect();
        for (position, &index) in self.order.iter().enumerate() {
            let row = &doc.rows[index];
            for column in &columns {
                let value = text(doc.cell(row.id(), column.id())?);
                scanned = scanned.saturating_add(value.map_or(1, |v| v.len().max(1)));
                if scanned > 64 * 1024 * 1024 {
                    return Err(error(
                        "SEARCH_LIMIT",
                        "Search scope exceeds 64 MiB. Select a smaller column.",
                    ));
                }
                let Some(value) = value else { continue };
                for m in regex.find_iter(value) {
                    if found.len() >= MAX_MATCHES {
                        return Err(error(
                            "SEARCH_LIMIT",
                            "More than 100,000 matches. Narrow the search.",
                        ));
                    }
                    found.push(SearchMatch {
                        row_id: row.id(),
                        column_id: column.id(),
                        start: m.start() as u32,
                        end: m.end() as u32,
                        revision: doc.revision(),
                        row_index: position as u32,
                    });
                }
            }
        }
        self.matching_cells = found.iter().map(|m| (m.row_id, m.column_id)).collect();
        self.matches = found;
        Ok(SearchResult {
            count: self.matches.len() as u32,
            current: self.matches.first().cloned(),
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
            cells.resize(cells.len().max(c.column as usize), CellValue::Missing);
            cells[(c.column - 1) as usize] = value.clone();
            while cells.last() == Some(&CellValue::Missing) {
                cells.pop();
            }
            if clean {
                self.originals.remove(&(c.row, c.column));
            }
        }
        doc.revision = revision;
        doc.dirty = !self.originals.is_empty();
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
