//! A small subset of TOML for the redrawn airport files: `# comments`,
//! `key = value` lines, `[table]` and `[[array of tables]]` headers, and
//! values that are numbers, quoted strings, booleans or (nested) arrays. An
//! array may span lines until its brackets close. Nothing else is accepted,
//! so a file that parses here also parses as TOML.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Text(String),
    Bool(bool),
    Array(Vec<Value>),
}

/// One table: its keys in sorted order.
pub type Table = BTreeMap<String, Value>;

/// A parsed file: the top-level keys, the named tables and the arrays of
/// tables in file order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub root: Table,
    pub tables: BTreeMap<String, Table>,
    pub arrays: BTreeMap<String, Vec<Table>>,
}

enum Target {
    Root,
    Table(String),
    Array(String),
}

impl Document {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut document = Self::default();
        let mut target = Target::Root;
        let mut pending: Option<(usize, String, String)> = None;
        for (index, raw) in text.lines().enumerate() {
            let line_number = index + 1;
            let line = strip_comment(raw);
            if let Some((start, key, mut value)) = pending.take() {
                value.push(' ');
                value.push_str(line.trim());
                if brackets_open(&value) {
                    pending = Some((start, key, value));
                } else {
                    let parsed = parse_value(&value).map_err(|e| format!("line {start}: {e}"))?;
                    document.insert(&target, key, parsed, start)?;
                }
                continue;
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix("[[") {
                let name = name
                    .strip_suffix("]]")
                    .ok_or_else(|| format!("line {line_number}: unclosed [[ header"))?
                    .trim();
                check_key(name, line_number)?;
                document
                    .arrays
                    .entry(name.to_owned())
                    .or_default()
                    .push(Table::new());
                target = Target::Array(name.to_owned());
                continue;
            }
            if let Some(name) = line.strip_prefix('[') {
                let name = name
                    .strip_suffix(']')
                    .ok_or_else(|| format!("line {line_number}: unclosed [ header"))?
                    .trim();
                check_key(name, line_number)?;
                if document
                    .tables
                    .insert(name.to_owned(), Table::new())
                    .is_some()
                {
                    return Err(format!("line {line_number}: table [{name}] repeated"));
                }
                target = Target::Table(name.to_owned());
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {line_number}: expected key = value"))?;
            let key = key.trim();
            check_key(key, line_number)?;
            let value = value.trim().to_owned();
            if brackets_open(&value) {
                pending = Some((line_number, key.to_owned(), value));
                continue;
            }
            let parsed = parse_value(&value).map_err(|e| format!("line {line_number}: {e}"))?;
            document.insert(&target, key.to_owned(), parsed, line_number)?;
        }
        if let Some((start, ..)) = pending {
            return Err(format!("line {start}: array never closed"));
        }
        Ok(document)
    }

    fn insert(
        &mut self,
        target: &Target,
        key: String,
        value: Value,
        line: usize,
    ) -> Result<(), String> {
        let table = match target {
            Target::Root => &mut self.root,
            Target::Table(name) => self.tables.get_mut(name).expect("table opened"),
            Target::Array(name) => self
                .arrays
                .get_mut(name)
                .and_then(|tables| tables.last_mut())
                .expect("array table opened"),
        };
        if table.contains_key(&key) {
            return Err(format!("line {line}: key {key} repeated"));
        }
        table.insert(key, value);
        Ok(())
    }
}

fn check_key(key: &str, line: usize) -> Result<(), String> {
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!("line {line}: bad key {key:?}"));
    }
    Ok(())
}

/// The line up to a `#` outside a string.
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (at, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..at],
            _ => {}
        }
    }
    line
}

fn brackets_open(text: &str) -> bool {
    let mut depth = 0i32;
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            '[' if !quoted => depth += 1,
            ']' if !quoted => depth -= 1,
            _ => {}
        }
    }
    depth > 0
}

fn parse_value(text: &str) -> Result<Value, String> {
    let mut chars = Parser {
        text: text.trim(),
        at: 0,
    };
    let value = chars.value()?;
    chars.skip_space();
    if chars.at != chars.text.len() {
        return Err(format!(
            "unexpected text after value: {:?}",
            &chars.text[chars.at..]
        ));
    }
    Ok(value)
}

struct Parser<'t> {
    text: &'t str,
    at: usize,
}

impl<'t> Parser<'t> {
    fn rest(&self) -> &'t str {
        &self.text[self.at..]
    }
    fn skip_space(&mut self) {
        while self.rest().starts_with([' ', '\t']) {
            self.at += 1;
        }
    }
    fn value(&mut self) -> Result<Value, String> {
        self.skip_space();
        let rest = self.rest();
        if let Some(body) = rest.strip_prefix('"') {
            let end = body.find('"').ok_or("unclosed string")?;
            if body[..end].contains('\\') {
                return Err("escapes are not supported".into());
            }
            self.at += end + 2;
            return Ok(Value::Text(body[..end].to_owned()));
        }
        if rest.starts_with('[') {
            self.at += 1;
            let mut items = Vec::new();
            loop {
                self.skip_space();
                if self.rest().starts_with(']') {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                items.push(self.value()?);
                self.skip_space();
                if self.rest().starts_with(',') {
                    self.at += 1;
                } else if !self.rest().starts_with(']') {
                    return Err("expected , or ] in array".into());
                }
            }
        }
        for (word, value) in [("true", true), ("false", false)] {
            if rest.starts_with(word) {
                self.at += word.len();
                return Ok(Value::Bool(value));
            }
        }
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | '_' | 'e' | 'E')))
            .unwrap_or(rest.len());
        let number = rest[..end].replace('_', "");
        let parsed: f64 = number
            .parse()
            .map_err(|_| format!("bad value {:?}", &rest[..end.max(1).min(rest.len())]))?;
        if !parsed.is_finite() {
            return Err("number out of range".into());
        }
        self.at += end;
        Ok(Value::Number(parsed))
    }
}

impl Value {
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }
    pub fn array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
    /// A fixed-size array of numbers, such as a `[x, z]` point.
    pub fn numbers<const N: usize>(&self) -> Option<[f64; N]> {
        let items = self.array()?;
        if items.len() != N {
            return None;
        }
        let mut out = [0.; N];
        for (slot, item) in out.iter_mut().zip(items) {
            *slot = item.number()?;
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_subset_reads_tables_arrays_and_values() {
        let document = Document::parse(
            "# airport\nlayout = \"UKR\" # trailing\nlength = 5_532\n\
             [anchors]\ntakeoff = [0, -1.5]\nparking = [\n  [1, 2],\n  [3, 4],\n]\n\
             [[runway]]\nname = \"36#R\"\nprimary = true\n[[runway]]\nname = \"36L\"\n",
        )
        .unwrap();
        assert_eq!(document.root["layout"], Value::Text("UKR".into()));
        assert_eq!(document.root["length"].number(), Some(5532.));
        let anchors = &document.tables["anchors"];
        assert_eq!(anchors["takeoff"].numbers::<2>(), Some([0., -1.5]));
        let parking = anchors["parking"].array().unwrap();
        assert_eq!(parking[1].numbers::<2>(), Some([3., 4.]));
        let runways = &document.arrays["runway"];
        assert_eq!(runways.len(), 2);
        assert_eq!(runways[0]["name"].text(), Some("36#R"));
        assert_eq!(runways[0]["primary"], Value::Bool(true));
        assert_eq!(runways[1]["name"].text(), Some("36L"));
    }

    #[test]
    fn the_subset_refuses_what_it_does_not_read() {
        for bad in [
            "a = 1\na = 2",
            "[t]\n[t]",
            "a = [1, 2",
            "a = 1 2",
            "a = 'single'",
            "a = \"x\\y\"",
            "bad key = 1",
            "[[open",
            "a = inf",
        ] {
            assert!(Document::parse(bad).is_err(), "{bad:?} parsed");
        }
    }
}
