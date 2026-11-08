//! Structured input maps leaves into canonical cells; shapes contain no values.
use crate::{
    document::{CellValue, Column, Document, Row, RowCells},
    error::EngineError,
};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{collections::HashMap, fmt};

/// Reconstruction tree. Cell references are stable column IDs within each row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    Cell(u32),
    Object(Vec<(String, Shape)>),
    Array(Vec<Shape>),
    Xml {
        name: String,
        namespaces: Vec<(Option<String>, String)>,
        attributes: Vec<(String, Shape)>,
        children: Vec<Shape>,
    },
}
/// Original envelope and per-row shapes, including empty containers.
#[derive(Debug)]
pub struct Structure {
    pub envelope: String,
    pub rows: Vec<Shape>,
    pub xml_root: Option<Shape>,
}

pub(crate) struct Fields<'a>(pub(crate) Vec<(String, &'a RawValue)>);
impl<'de> Deserialize<'de> for Fields<'de> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Fields<'de>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object with unique keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Fields<'de>, M::Error> {
                let mut fields = Vec::new();
                let mut keys = std::collections::HashSet::new();
                while let Some((key, value)) = map.next_entry::<String, &'de RawValue>()? {
                    if !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom(format!(
                            "Duplicate key {key:?}; rename or remove it."
                        )));
                    }
                    fields.push((key, value));
                }
                Ok(Fields(fields))
            }
        }
        d.deserialize_map(V)
    }
}
fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::new("INVALID_STRUCTURE", message)
}
fn json_error(e: serde_json::Error) -> EngineError {
    EngineError::new("INVALID_JSON", e.to_string()).at(e.line(), 1)
}
fn decode<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T, EngineError> {
    serde_json::from_str(s).map_err(json_error)
}
fn depth(n: usize) -> Result<(), EngineError> {
    if n > 64 {
        Err(invalid(
            "Nesting exceeds 64 levels. Flatten the input before importing.",
        ))
    } else {
        Ok(())
    }
}
fn key_path(parent: &str, key: &str) -> String {
    let simple = !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_');
    if simple {
        if parent.is_empty() {
            key.into()
        } else {
            format!("{parent}.{key}")
        }
    } else {
        format!(
            "{parent}[{}]",
            serde_json::to_string(key).unwrap_or_default()
        )
    }
}
#[derive(Default)]
struct Builder<'a> {
    source: &'a str,
    columns: Vec<Column>,
    indices: HashMap<String, usize>,
    rows: Vec<Row>,
    shapes: Vec<Shape>,
}
impl Builder<'_> {
    fn decode<'a, T: Deserialize<'a>>(&self, raw: &'a str) -> Result<T, EngineError> {
        serde_json::from_str(raw).map_err(|e| {
            // RawValue borrows the original source: no substring copies or
            // per-value line table are needed. Scan the prefix only on failure.
            let offset = raw.as_ptr() as usize - self.source.as_ptr() as usize;
            let prefix = &self.source[..offset];
            let line = prefix.bytes().filter(|b| *b == b'\n').count() + e.line();
            let column = if e.line() == 1 {
                prefix.rsplit('\n').next().map_or(0, str::len) + e.column()
            } else {
                e.column()
            };
            let message = e.to_string();
            let message = message
                .rsplit_once(" at line ")
                .map_or(message.as_str(), |(reason, _)| reason);
            EngineError::new(
                "INVALID_JSON",
                format!("{message} at line {line} column {column}"),
            )
            .at(line, self.rows.len() + 1)
        })
    }
    fn leaf(
        &mut self,
        cells: &mut Vec<(usize, CellValue)>,
        path: String,
        value: CellValue,
    ) -> Result<Shape, EngineError> {
        let index = if let Some(i) = self.indices.get(&path) {
            *i
        } else {
            let i = self.columns.len();
            let id = u32::try_from(i + 1).map_err(|_| invalid("Too many columns."))?;
            self.columns.push(Column {
                id,
                name: path.clone(),
                original_name: Some(path.clone()),
            });
            self.indices.insert(path, i);
            i
        };
        cells.push((index, value));
        Ok(Shape::Cell(self.columns[index].id))
    }
    fn json(
        &mut self,
        raw: &str,
        path: &str,
        cells: &mut Vec<(usize, CellValue)>,
        level: usize,
    ) -> Result<Shape, EngineError> {
        depth(level)?;
        match raw.as_bytes().first() {
            Some(b'{') => {
                let Fields(fields) = self.decode(raw)?;
                let mut children = Vec::with_capacity(fields.len());
                for (key, value) in fields {
                    let child = self.json(value.get(), &key_path(path, &key), cells, level + 1)?;
                    children.push((key, child));
                }
                Ok(Shape::Object(children))
            }
            Some(b'[') => {
                let values: Vec<&RawValue> = self.decode(raw)?;
                let mut children = Vec::with_capacity(values.len());
                for (i, value) in values.into_iter().enumerate() {
                    children.push(self.json(
                        value.get(),
                        &format!("{path}[{i}]"),
                        cells,
                        level + 1,
                    )?);
                }
                Ok(Shape::Array(children))
            }
            _ => {
                let value = match raw {
                    "null" => CellValue::Null,
                    "true" => CellValue::Boolean(true),
                    "false" => CellValue::Boolean(false),
                    s if s.starts_with('"') => CellValue::Text(self.decode(s)?),
                    s => CellValue::Number(s.into()),
                };
                self.leaf(cells, path.into(), value)
            }
        }
    }
    fn record(&mut self, raw: &str) -> Result<(), EngineError> {
        if !raw.starts_with('{') {
            return Err(invalid(
                "Records must be objects. Wrap scalar or array records in a named property.",
            ));
        }
        let mut cells = Vec::new();
        let shape = self.json(raw, "", &mut cells, 0)?;
        self.push(cells, shape)
    }
    fn push(
        &mut self,
        mut cells: Vec<(usize, CellValue)>,
        shape: Shape,
    ) -> Result<(), EngineError> {
        let id = u32::try_from(self.rows.len() + 1).map_err(|_| invalid("Too many rows."))?;
        cells.sort_unstable_by_key(|(index, _)| *index);
        self.rows.push(Row {
            id,
            cells: RowCells::Sparse(cells),
        });
        self.shapes.push(shape);
        Ok(())
    }
    fn finish(
        self,
        format: &str,
        envelope: &str,
        xml_root: Option<Shape>,
    ) -> Result<Document, EngineError> {
        let mut doc = Document::new(self.columns, self.rows, b',', false)?;
        doc.source_format = format.into();
        doc.structure = Some(Structure {
            envelope: envelope.into(),
            rows: self.shapes,
            xml_root,
        });
        Ok(doc)
    }
}
/// Parse exact JSON numbers directly from validated lexical tokens; no f64 conversion.
pub fn parse(input: &[u8], format: &str) -> Result<Document, EngineError> {
    if !matches!(format, "json" | "ndjson" | "xml") {
        return Err(EngineError::new(
            "INVALID_FORMAT",
            "Choose JSON, NDJSON or XML for structured input.",
        ));
    }
    let text = std::str::from_utf8(input)
        .map_err(|_| EngineError::new("INVALID_UTF8", "Save the document as UTF-8."))?
        .trim_start_matches('\u{feff}');
    if format == "xml" {
        return xml(text);
    }
    let mut builder = Builder {
        source: text,
        ..Default::default()
    };
    if format == "ndjson" {
        for (line, value) in text.lines().enumerate() {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let result = decode::<&RawValue>(value).and_then(|raw| builder.record(raw.get()));
            result.map_err(|e| {
                EngineError::new(e.code(), e.message()).at(line + 1, builder.rows.len() + 1)
            })?;
        }
        return builder.finish(format, "lines", None);
    }
    let raw: &RawValue = decode(text)?;
    let envelope = if raw.get().starts_with('[') {
        let values: Vec<&RawValue> = decode(raw.get())?;
        for value in values {
            builder.record(value.get())?;
        }
        "array"
    } else {
        builder.record(raw.get())?;
        "object"
    };
    builder.finish(format, envelope, None)
}
fn expanded(ns: Option<&str>, name: &str) -> String {
    ns.filter(|ns| !ns.is_empty())
        .map_or_else(|| name.into(), |ns| format!("{{{ns}}}{name}"))
}
fn xml(text: &str) -> Result<Document, EngineError> {
    // roxmltree accepts a UTF-8 str and ignores declaration encodings. Reject a
    // conflicting declaration rather than silently reading it with another encoding.
    if let Some(declaration) = text
        .strip_prefix("<?xml")
        .and_then(|s| s.split_once("?>").map(|p| p.0))
    {
        if let Some((_, encoding)) = declaration.split_once("encoding") {
            let value = encoding
                .trim_start()
                .strip_prefix('=')
                .map(str::trim_start)
                .unwrap_or("");
            if let Some(quote) = value.chars().next().filter(|c| *c == '\'' || *c == '"') {
                if let Some((encoding, _)) = value[1..].split_once(quote) {
                    if !encoding.eq_ignore_ascii_case("utf-8") {
                        return Err(EngineError::new(
                            "INVALID_XML",
                            "Only UTF-8 XML is supported. Convert the input and its encoding declaration to UTF-8.",
                        ));
                    }
                }
            }
        }
    }
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, options)
        .map_err(|e| EngineError::new("INVALID_XML", e.to_string()).at(e.pos().row as usize, 1))?;
    // Comments and processing instructions are rejected rather than silently lost.
    for node in doc.descendants() {
        if node.is_comment() || node.is_pi() {
            return Err(invalid(
                "XML comments and processing instructions are unsupported; remove them before importing.",
            ));
        }
        depth(node.ancestors().count().saturating_sub(1))?;
    }
    let root = doc.root_element();
    if root.attributes().len() > 0
        || root
            .children()
            .any(|n| n.is_text() && !n.text().unwrap_or("").trim().is_empty())
    {
        return Err(invalid(
            "XML needs a wrapper containing repeated record elements, without wrapper attributes or text.",
        ));
    }
    let mut builder = Builder::default();
    let mut record_name = None;
    for record in root.children().filter(|n| n.is_element()) {
        let name = expanded(record.tag_name().namespace(), record.tag_name().name());
        if record_name.as_ref().is_some_and(|n| n != &name) {
            return Err(invalid(
                "XML wrapper children must share one record element name.",
            ));
        }
        record_name = Some(name);
        let mut cells = Vec::new();
        let shape = builder.xml_node(record, "", &mut cells, 0)?;
        builder.push(cells, shape)?;
    }
    let wrapper = Shape::Xml {
        name: expanded(root.tag_name().namespace(), root.tag_name().name()),
        namespaces: namespaces(root),
        attributes: Vec::new(),
        children: Vec::new(),
    };
    builder.finish("xml", "xml-records", Some(wrapper))
}
fn namespaces(node: roxmltree::Node) -> Vec<(Option<String>, String)> {
    node.namespaces()
        .map(|n| (n.name().map(str::to_owned), n.uri().into()))
        .collect()
}
impl Builder<'_> {
    fn xml_node(
        &mut self,
        node: roxmltree::Node,
        path: &str,
        cells: &mut Vec<(usize, CellValue)>,
        level: usize,
    ) -> Result<Shape, EngineError> {
        depth(level)?;
        let mut attributes = Vec::new();
        for attr in node.attributes() {
            let name = expanded(attr.namespace(), attr.name());
            attributes.push((
                name.clone(),
                self.leaf(
                    cells,
                    format!("{path}@{name}"),
                    CellValue::Text(attr.value().into()),
                )?,
            ));
        }
        let elements: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        let mut children = Vec::new();
        if elements.is_empty() {
            let value: String = node.children().filter_map(|n| n.text()).collect();
            children.push(self.leaf(cells, format!("{path}#text"), CellValue::Text(value))?);
        } else {
            if node
                .children()
                .any(|n| n.is_text() && !n.text().unwrap_or("").trim().is_empty())
            {
                return Err(invalid(
                    "Mixed XML text and child elements cannot be reconstructed safely; separate the text into an element.",
                ));
            }
            let mut counts = HashMap::new();
            for child in &elements {
                *counts
                    .entry(expanded(
                        child.tag_name().namespace(),
                        child.tag_name().name(),
                    ))
                    .or_insert(0usize) += 1;
            }
            let mut seen = HashMap::new();
            for child in elements {
                let name = expanded(child.tag_name().namespace(), child.tag_name().name());
                let index = seen.entry(name.clone()).or_insert(0usize);
                let base = key_path(path, &name);
                let child_path = if counts[&name] > 1 {
                    format!("{base}[{index}]")
                } else {
                    base
                };
                *index += 1;
                children.push(self.xml_node(child, &child_path, cells, level + 1)?);
            }
        }
        Ok(Shape::Xml {
            name: expanded(node.tag_name().namespace(), node.tag_name().name()),
            namespaces: namespaces(node),
            attributes,
            children,
        })
    }
}
