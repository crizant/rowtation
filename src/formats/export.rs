//! Explicit local conversion policies. Only final serialized output crosses the bridge.
use super::structured::Shape;
use crate::{
    document::{CellValue, Document},
    error::EngineError,
    session::ExportOptions,
};
use std::collections::HashSet;
fn fail(message: &str) -> EngineError {
    EngineError::new("LOSSY_EXPORT", message)
}
fn format(o: &ExportOptions) -> Result<&str, EngineError> {
    match o.format.as_deref().unwrap_or("csv") {
        f @ ("csv" | "tsv" | "json" | "ndjson" | "xml") => Ok(f),
        _ => Err(EngineError::new(
            "INVALID_FORMAT",
            "Choose CSV, TSV, JSON, NDJSON or XML.",
        )),
    }
}
/// Returns the explicit conversion policy requiring acknowledgement, without serializing data.
pub fn warning(d: &Document, o: &ExportOptions) -> Result<String, EngineError> {
    let f = format(o)?;
    let mut warnings = Vec::new();
    if matches!(f, "csv" | "tsv") && d.structure().is_some() {
        warnings.push("Nested paths become literal column headers; empty containers and source envelopes are lost. Numbers and booleans become text; null and interior missing fields become empty text. Trailing missing fields remain absent.");
    }
    if matches!(f, "csv" | "tsv")
        && d.structure().is_none()
        && d.rows.iter().any(|row| {
            row.cells
                .iter()
                .any(|value| !matches!(value, CellValue::Text(_)))
        })
    {
        warnings.push("Interior missing and null cells become empty text; typed values become their lexical text. Trailing missing fields remain absent.");
    }
    if matches!(f, "json" | "ndjson") && d.source_format() == "xml" && o.reconstruct != Some(false)
    {
        warnings.push("XML maps to JSON objects: @name attributes, #text leaves and child-name arrays. XML namespace bindings, wrapper, record names and interleaved child order are omitted; expanded names remain in keys.");
    }
    if matches!(f, "json" | "ndjson") && d.structure().is_some() && o.reconstruct == Some(false) {
        warnings.push("Flat output uses literal column paths and discards source container structure, empty containers and the source envelope.");
    }
    if f == "ndjson" && d.structure().is_some_and(|s| s.envelope == "object") {
        warnings.push("The single-object JSON envelope becomes one NDJSON record. Converting it back to JSON produces an array.");
    }
    if f == "xml" && d.source_format() != "xml" {
        warnings.push("XML uses rows/row, named elements and item elements for arrays. Scalar types and array identity are lost on reimport. Null, invalid XML names and empty containers are rejected.");
    }
    if matches!(f, "csv" | "tsv") && o.safe_text.unwrap_or(false) {
        warnings.push("Formula-safe text prefixes an apostrophe to fields and headers starting with =, +, -, @, tab, CR or LF (also after leading whitespace). This changes their values.");
    }
    Ok(warnings.join("\n\n"))
}
fn quote(s: &str) -> Result<String, EngineError> {
    serde_json::to_string(s).map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()))
}
fn scalar(v: &CellValue) -> Result<String, EngineError> {
    Ok(match v {
        CellValue::Missing => return Err(fail("Missing array elements cannot be reconstructed.")),
        CellValue::Null => "null".into(),
        CellValue::Text(s) => quote(s)?,
        CellValue::Number(s) => s.clone(),
        CellValue::Boolean(b) => b.to_string(),
    })
}
fn json_shape(
    d: &Document,
    row: u32,
    s: &Shape,
    seen: &mut HashSet<u32>,
) -> Result<String, EngineError> {
    Ok(match s {
        Shape::Cell(id) => {
            seen.insert(*id);
            scalar(d.cell(row, *id)?)?
        }
        Shape::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(k, v)| Ok(format!("{}:{}", quote(k)?, json_shape(d, row, v, seen)?)))
                .collect::<Result<Vec<_>, EngineError>>()?
                .join(",")
        ),
        Shape::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(|v| json_shape(d, row, v, seen))
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        Shape::Xml {
            attributes,
            children,
            ..
        } => {
            let mut fields = Vec::new();
            for (name, value) in attributes {
                fields.push(format!(
                    "{}:{}",
                    quote(&format!("@{name}"))?,
                    json_shape(d, row, value, seen)?
                ));
            }
            let mut names = Vec::<String>::new();
            for child in children {
                match child {
                    Shape::Cell(_) => {
                        fields.push(format!("\"#text\":{}", json_shape(d, row, child, seen)?))
                    }
                    Shape::Xml { name, .. } if !names.contains(name) => names.push(name.clone()),
                    _ => (),
                }
            }
            for name in names {
                let values = children
                    .iter()
                    .filter(|c| matches!(c,Shape::Xml{name:n,..} if n==&name))
                    .map(|c| json_shape(d, row, c, seen))
                    .collect::<Result<Vec<_>, _>>()?;
                fields.push(format!("{}:[{}]", quote(&name)?, values.join(",")));
            }
            format!("{{{}}}", fields.join(","))
        }
    })
}
fn flat(d: &Document, row: u32) -> Result<String, EngineError> {
    let mut keys = HashSet::new();
    let mut fields = Vec::new();
    for c in d.columns() {
        if !keys.insert(c.name()) {
            return Err(fail(
                "Duplicate headers cannot become object keys. Use CSV or TSV.",
            ));
        }
        let v = d.cell(row, c.id())?;
        if v != &CellValue::Missing {
            fields.push(format!("{}:{}", quote(c.name())?, scalar(v)?));
        }
    }
    Ok(format!("{{{}}}", fields.join(",")))
}
// Insert only edited leaves, using retained paths from another row. No key/path
// parsing, value inference or document metadata mutation is involved.
fn contains(shape: &Shape, id: u32) -> bool {
    match shape {
        Shape::Cell(value) => *value == id,
        Shape::Object(fields) => fields.iter().any(|(_, s)| contains(s, id)),
        Shape::Array(items) => items.iter().any(|s| contains(s, id)),
        Shape::Xml { .. } => false,
    }
}
fn branch(template: &Shape, id: u32) -> Result<Shape, EngineError> {
    match template {
        Shape::Cell(value) if *value == id => Ok(Shape::Cell(id)),
        Shape::Object(fields) => {
            let (key, child) = fields
                .iter()
                .find(|(_, s)| contains(s, id))
                .ok_or_else(|| fail("Missing reconstruction path."))?;
            Ok(Shape::Object(vec![(key.clone(), branch(child, id)?)]))
        }
        Shape::Array(items) if items.first().is_some_and(|s| contains(s, id)) => {
            Ok(Shape::Array(vec![branch(&items[0], id)?]))
        }
        _ => Err(fail(
            "The edited field would create missing array elements. Fill earlier elements or choose flat output.",
        )),
    }
}
fn insert(target: &mut Shape, template: &Shape, id: u32) -> Result<(), EngineError> {
    match (target, template) {
        (Shape::Object(target), Shape::Object(source)) => {
            let (key, child) = source
                .iter()
                .find(|(_, s)| contains(s, id))
                .ok_or_else(|| fail("Missing reconstruction path."))?;
            if let Some((_, existing)) = target.iter_mut().find(|(k, _)| k == key) {
                insert(existing, child, id)
            } else {
                target.push((key.clone(), branch(child, id)?));
                Ok(())
            }
        }
        (Shape::Array(target), Shape::Array(source)) => {
            let index = source
                .iter()
                .position(|s| contains(s, id))
                .ok_or_else(|| fail("Missing reconstruction path."))?;
            if index < target.len() {
                insert(&mut target[index], &source[index], id)
            } else if index == target.len() {
                target.push(branch(&source[index], id)?);
                Ok(())
            } else {
                Err(fail(
                    "The edited field would create missing array elements. Fill earlier elements or choose flat output.",
                ))
            }
        }
        (Shape::Cell(existing), Shape::Cell(source)) if *existing == id && *source == id => Ok(()),
        _ => Err(fail(
            "An edited field conflicts with an existing scalar or container. Choose flat JSON/NDJSON or delimited output.",
        )),
    }
}
fn reconstructed_json(d: &Document, row: u32, index: usize) -> Result<String, EngineError> {
    let structure = d
        .structure()
        .ok_or_else(|| fail("Missing source structure."))?;
    let mut seen = HashSet::new();
    let result = json_shape(d, row, &structure.rows[index], &mut seen)?;
    let extras = d
        .columns()
        .iter()
        .filter(|c| !seen.contains(&c.id()))
        .filter_map(|c| match d.cell(row, c.id()) {
            Ok(CellValue::Missing) => None,
            Ok(_) => Some(Ok(c.id())),
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if extras.is_empty() {
        return Ok(result);
    }
    if d.source_format() == "xml" {
        ensure_seen(d, row, &seen)?;
        return Ok(result);
    }
    let mut shape = structure.rows[index].clone();
    for id in extras {
        let template = structure
            .rows
            .iter()
            .find(|s| contains(s, id))
            .ok_or_else(|| fail("Missing reconstruction path. Choose flat output."))?;
        insert(&mut shape, template, id)?;
    }
    seen.clear();
    let result = json_shape(d, row, &shape, &mut seen)?;
    ensure_seen(d, row, &seen)?;
    Ok(result)
}
fn ensure_seen(d: &Document, row: u32, seen: &HashSet<u32>) -> Result<(), EngineError> {
    for c in d.columns() {
        if !seen.contains(&c.id()) && d.cell(row, c.id())? != &CellValue::Missing {
            return Err(fail(
                "An edited previously absent field has no unambiguous source position. Choose flat JSON/NDJSON or delimited output.",
            ));
        }
    }
    Ok(())
}
fn xml_escape(s: &str, attr: bool) -> Result<String, EngineError> {
    if s.chars().any(|c| {
        !(matches!(c, '\t' | '\n' | '\r') || c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}')
    }) {
        return Err(fail(
            "A value contains characters forbidden by XML 1.0. Choose JSON.",
        ));
    }
    let s = s
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\r', "&#13;");
    Ok(if attr {
        s.replace('"', "&quot;")
            .replace('\n', "&#10;")
            .replace('\t', "&#9;")
    } else {
        s
    })
}
// XML 1.0 NameStartChar. Unicode alphabetic is a different set (for example µ).
fn xml_name_start(c: char) -> bool {
    matches!(
        c,
        ':' | 'A'..='Z'
            | '_'
            | 'a'..='z'
            | '\u{C0}'..='\u{D6}'
            | '\u{D8}'..='\u{F6}'
            | '\u{F8}'..='\u{2FF}'
            | '\u{370}'..='\u{37D}'
            | '\u{37F}'..='\u{1FFF}'
            | '\u{200C}'..='\u{200D}'
            | '\u{2070}'..='\u{218F}'
            | '\u{2C00}'..='\u{2FEF}'
            | '\u{3001}'..='\u{D7FF}'
            | '\u{F900}'..='\u{FDCF}'
            | '\u{FDF0}'..='\u{FFFD}'
            | '\u{10000}'..='\u{EFFFF}'
    )
}
// XML 1.0 NameChar. Unicode alphanumeric includes characters this production rejects, such as ².
fn xml_name_char(c: char) -> bool {
    xml_name_start(c)
        || matches!(
            c,
            '-' | '.' | '0'..='9' | '\u{B7}' | '\u{0300}'..='\u{036F}' | '\u{203F}'..='\u{2040}'
        )
}
fn xml_name(s: &str) -> Result<(), EngineError> {
    let mut chars = s.chars();
    if !chars.next().is_some_and(xml_name_start) || !chars.all(xml_name_char) {
        return Err(fail(
            "A column/key is not a supported XML name. Use JSON or delimited output.",
        ));
    }
    Ok(())
}
fn qualified(
    name: &str,
    namespaces: &[(Option<String>, String)],
    attribute: bool,
) -> Result<String, EngineError> {
    if let Some(rest) = name.strip_prefix('{') {
        if let Some((uri, local)) = rest.rsplit_once('}') {
            if uri == "http://www.w3.org/XML/1998/namespace" {
                return Ok(format!("xml:{local}"));
            }
            if let Some((prefix, _)) = namespaces
                .iter()
                .find(|(p, u)| u == uri && (!attribute || p.is_some()))
            {
                return Ok(prefix
                    .as_ref()
                    .map_or_else(|| local.into(), |p| format!("{p}:{local}")));
            }
            return Err(fail(
                "An XML namespace binding is unavailable; choose JSON.",
            ));
        }
    }
    Ok(name.into())
}
fn declarations(namespaces: &[(Option<String>, String)]) -> Result<String, EngineError> {
    let mut result = String::new();
    // Reset the inherited default when the source node had none.
    if !namespaces.iter().any(|(p, _)| p.is_none()) {
        result.push_str(" xmlns=\"\"");
    }
    for (prefix, uri) in namespaces {
        if prefix.as_deref() == Some("xml") {
            continue;
        }
        result.push_str(&format!(
            " xmlns{}=\"{}\"",
            prefix.as_ref().map_or(String::new(), |p| format!(":{p}")),
            xml_escape(uri, true)?
        ));
    }
    Ok(result)
}
fn xml_shape(
    d: &Document,
    row: u32,
    s: &Shape,
    seen: &mut HashSet<u32>,
) -> Result<String, EngineError> {
    match s {
        Shape::Cell(id) => {
            seen.insert(*id);
            match d.cell(row, *id)? {
                CellValue::Text(v) => xml_escape(v, false),
                _ => Err(fail("XML source leaves must remain text.")),
            }
        }
        Shape::Xml {
            name,
            attributes,
            children,
            namespaces,
        } => {
            let name = qualified(name, namespaces, false)?;
            let decl = declarations(namespaces)?;
            let mut out = format!("<{name}{decl}");
            for (key, value) in attributes {
                let key = qualified(key, namespaces, true)?;
                let Shape::Cell(id) = value else {
                    return Err(fail("Invalid XML attribute metadata."));
                };
                seen.insert(*id);
                let CellValue::Text(v) = d.cell(row, *id)? else {
                    return Err(fail("XML attributes must be text."));
                };
                out.push_str(&format!(" {key}=\"{}\"", xml_escape(v, true)?));
            }
            out.push('>');
            for child in children {
                out.push_str(&xml_shape(d, row, child, seen)?);
            }
            out.push_str(&format!("</{name}>"));
            Ok(out)
        }
        _ => Err(fail("Unsupported XML reconstruction.")),
    }
}
fn generic_xml(name: &str, raw: &str) -> Result<String, EngineError> {
    xml_name(name)?;
    let content = if raw == "null" {
        return Err(fail(
            "XML conversion cannot preserve null. Choose JSON or NDJSON.",
        ));
    } else if raw.starts_with('{') {
        let super::structured::Fields(fields) = serde_json::from_str(raw)
            .map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()))?;
        if fields.is_empty() {
            return Err(fail("XML conversion cannot preserve empty objects."));
        }
        fields
            .iter()
            .map(|(k, v)| generic_xml(k, v.get()))
            .collect::<Result<Vec<_>, _>>()?
            .join("")
    } else if raw.starts_with('[') {
        let values: Vec<&serde_json::value::RawValue> = serde_json::from_str(raw)
            .map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()))?;
        if values.is_empty() {
            return Err(fail("XML conversion cannot preserve empty arrays."));
        }
        values
            .iter()
            .map(|v| generic_xml("item", v.get()))
            .collect::<Result<Vec<_>, _>>()?
            .join("")
    } else if raw.starts_with('"') {
        let value: String = serde_json::from_str(raw)
            .map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()))?;
        xml_escape(&value, false)?
    } else {
        xml_escape(raw, false)?
    };
    Ok(format!("<{name}>{content}</{name}>"))
}
/// Serialize without changing revision, data, history or ordering.
pub fn serialize(d: &Document, order: &[usize], o: &ExportOptions) -> Result<Vec<u8>, EngineError> {
    let f = format(o)?;
    let warning = warning(d, o)?;
    if !warning.is_empty() && !o.allow_lossy.unwrap_or(false) {
        return Err(fail(&warning));
    }
    let indices: Vec<usize> = if o.sorted.unwrap_or(false) && order.len() == d.row_count() {
        order.to_vec()
    } else {
        (0..d.row_count()).collect()
    };
    if matches!(f, "csv" | "tsv") {
        let source_delimiter = char::from(d.source_delimiter()).to_string();
        // Preserve the existing strict serializer's exact delimited round-trip behavior.
        if (o.format.is_none() || (d.structure().is_none() && warning.is_empty()))
            && !o.sorted.unwrap_or(false)
            && !o.safe_text.unwrap_or(false)
        {
            return super::delimited::serialize(
                d,
                o.delimiter.as_deref().unwrap_or(if o.format.is_none() {
                    &source_delimiter
                } else if f == "tsv" {
                    "\t"
                } else {
                    ","
                }),
                o.headers.unwrap_or(if o.format.is_none() {
                    d.source_headers()
                } else {
                    true
                }),
            );
        }
        let delimiter =
            super::delimited::delimiter(o.delimiter.as_deref().unwrap_or(if f == "tsv" {
                "\t"
            } else {
                ","
            }))?;
        let safe = |s: String| {
            if o.safe_text.unwrap_or(false)
                && (s.starts_with(['\t', '\r', '\n'])
                    || s.trim_start().starts_with(['=', '+', '-', '@']))
            {
                format!("'{s}")
            } else {
                s
            }
        };
        let mut w = csv::WriterBuilder::new()
            .flexible(true)
            .quote_style(csv::QuoteStyle::Always)
            .delimiter(delimiter)
            .from_writer(Vec::new());
        let err = |e: csv::Error| EngineError::new("SERIALIZE_FAILED", e.to_string());
        if o.headers.unwrap_or(true) && d.column_count() > 0 {
            w.write_record(d.columns().iter().map(|c| safe(c.name().into())))
                .map_err(err)?;
        }
        for i in indices {
            let r = &d.rows[i];
            let length = (0..d.column_count())
                .rfind(|&j| r.cells.get(j).is_some_and(|c| c != &CellValue::Missing))
                .map_or(0, |j| j + 1);
            let fields = (0..length).map(|j| {
                safe(match r.cells.get(j) {
                    Some(CellValue::Text(s) | CellValue::Number(s)) => s.clone(),
                    Some(CellValue::Boolean(b)) => b.to_string(),
                    _ => String::new(),
                })
            });
            w.write_record(fields).map_err(err)?;
        }
        return w
            .into_inner()
            .map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()));
    }
    let mut records = Vec::new();
    for i in indices {
        let row = d.rows[i].id();
        let mut seen = HashSet::new();
        let record = if f == "xml" && d.source_format() == "xml" {
            let s = &d
                .structure()
                .ok_or_else(|| fail("Missing XML structure."))?
                .rows[i];
            let value = xml_shape(d, row, s, &mut seen)?;
            ensure_seen(d, row, &seen)?;
            value
        } else {
            let value = if d.structure().is_some() && o.reconstruct != Some(false) {
                reconstructed_json(d, row, i)?
            } else {
                flat(d, row)?
            };
            if f == "xml" {
                generic_xml("row", &value)?
            } else {
                value
            }
        };
        records.push(record);
    }
    let mut output = match f {
        "ndjson" => {
            if records.is_empty() {
                String::new()
            } else {
                records.join("\n") + "\n"
            }
        }
        "json" => {
            let single = d.structure().is_some_and(|s| s.envelope == "object")
                && o.reconstruct != Some(false);
            let raw = if single {
                records.into_iter().next().unwrap_or("{}".into())
            } else {
                format!("[{}]", records.join(","))
            };
            if o.pretty.unwrap_or(false) {
                pretty_json(&raw)?
            } else {
                raw
            }
        }
        "xml" => {
            let root = d.structure().and_then(|s| s.xml_root.as_ref());
            let (name, decl) = if let Some(Shape::Xml {
                name, namespaces, ..
            }) = root
            {
                (
                    qualified(name, namespaces, false)?,
                    declarations(namespaces)?,
                )
            } else {
                ("rows".into(), String::new())
            };
            format!("<{name}{decl}>{}</{name}>", records.join(""))
        }
        _ => {
            return Err(EngineError::new(
                "INVALID_FORMAT",
                "Choose a supported output format.",
            ));
        }
    };
    if f == "xml" && o.pretty.unwrap_or(false) {
        output = pretty_xml(&output)?;
    }
    Ok(output.into_bytes())
}
// RawValue keeps number tokens exact even through pretty formatting.
fn pretty_json(raw: &str) -> Result<String, EngineError> {
    fn write(s: &str, n: usize) -> Result<String, EngineError> {
        if s.starts_with('{') {
            let super::structured::Fields(fields) = serde_json::from_str(s)
                .map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()))?;
            if fields.is_empty() {
                return Ok("{}".into());
            }
            let parts = fields
                .iter()
                .map(|(k, v)| {
                    Ok(format!(
                        "{}{}: {}",
                        "  ".repeat(n + 1),
                        quote(k)?,
                        write(v.get(), n + 1)?
                    ))
                })
                .collect::<Result<Vec<_>, EngineError>>()?;
            Ok(format!("{{\n{}\n{}}}", parts.join(",\n"), "  ".repeat(n)))
        } else if s.starts_with('[') {
            let a: Vec<&serde_json::value::RawValue> = serde_json::from_str(s)
                .map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()))?;
            if a.is_empty() {
                return Ok("[]".into());
            }
            let parts = a
                .iter()
                .map(|v| Ok(format!("{}{}", "  ".repeat(n + 1), write(v.get(), n + 1)?)))
                .collect::<Result<Vec<_>, EngineError>>()?;
            Ok(format!("[\n{}\n{}]", parts.join(",\n"), "  ".repeat(n)))
        } else {
            Ok(s.into())
        }
    }
    write(raw, 0)
}
fn pretty_xml(raw: &str) -> Result<String, EngineError> {
    let doc = roxmltree::Document::parse(raw)
        .map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()))?;
    fn node(n: roxmltree::Node, raw: &str, level: usize) -> String {
        let children: Vec<_> = n.children().filter(|c| c.is_element()).collect();
        if children.is_empty() {
            return format!("{}{}", "  ".repeat(level), &raw[n.range()]);
        }
        let range = n.range();
        let start = raw[range.clone()].find('>').unwrap_or(0) + range.start + 1;
        let end = raw[range.clone()].rfind("</").unwrap_or(0) + range.start;
        format!(
            "{}{}\n{}\n{}{}",
            "  ".repeat(level),
            &raw[range.start..start],
            children
                .into_iter()
                .map(|c| node(c, raw, level + 1))
                .collect::<Vec<_>>()
                .join("\n"),
            "  ".repeat(level),
            &raw[end..range.end]
        )
    }
    Ok(node(doc.root_element(), raw, 0))
}
