//! A small, bounded reader for the few XML elements a router's device
//! description and its SOAP answers carry.
//!
//! It turns a document into a flat list of start tags, end tags and text,
//! with namespace prefixes dropped (`s:Envelope` is `Envelope`), attributes
//! skipped, comments, processing instructions and declarations skipped,
//! CDATA kept as text and the five named entities and character references
//! decoded. Nesting is checked. It never builds a tree; the callers look for
//! the elements they need by name. Input is bounded by the HTTP reader
//! (64 KB); events and depth are bounded here.

/// The most events one document may give.
pub(super) const MAX_EVENTS: usize = 20_000;
/// The deepest nesting accepted.
pub(super) const MAX_DEPTH: usize = 64;

/// One piece of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Event {
    /// An element opens (an empty element gives a start and an end).
    Start(String),
    /// An element closes.
    End(String),
    /// Text, trimmed, with entities decoded; never empty.
    Text(String),
}

/// Why a document was not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum XmlError {
    /// A tag, comment or section that never ends, or elements left open.
    Unclosed,
    /// An end tag that does not close the open element.
    Mismatch,
    /// Nested deeper than [`MAX_DEPTH`].
    TooDeep,
    /// More than [`MAX_EVENTS`] events.
    TooLong,
    /// A tag with no name.
    Malformed,
}

/// Reads `text` into events.
pub(super) fn parse(text: &str) -> Result<Vec<Event>, XmlError> {
    let mut events = Vec::new();
    let mut open: Vec<String> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if events.len() > MAX_EVENTS {
            return Err(XmlError::TooLong);
        }
        let Some(after) = rest.strip_prefix('<') else {
            let end = rest.find('<').unwrap_or(rest.len());
            push_text(&mut events, &decode(&rest[..end]));
            rest = &rest[end..];
            continue;
        };
        if let Some(body) = after.strip_prefix("!--") {
            let end = body.find("-->").ok_or(XmlError::Unclosed)?;
            rest = &body[end + 3..];
        } else if let Some(body) = after.strip_prefix("![CDATA[") {
            let end = body.find("]]>").ok_or(XmlError::Unclosed)?;
            push_text(&mut events, &body[..end]);
            rest = &body[end + 3..];
        } else if let Some(body) = after.strip_prefix('?') {
            let end = body.find("?>").ok_or(XmlError::Unclosed)?;
            rest = &body[end + 2..];
        } else if after.starts_with('!') {
            rest = &after[declaration_end(after)?..];
        } else if let Some(body) = after.strip_prefix('/') {
            let end = body.find('>').ok_or(XmlError::Unclosed)?;
            let name = local_name(body[..end].trim())?;
            if open.pop().as_deref() != Some(name) {
                return Err(XmlError::Mismatch);
            }
            events.push(Event::End(name.to_owned()));
            rest = &body[end + 1..];
        } else {
            let end = tag_end(after)?;
            let inside = &after[..end];
            let (inside, empty) = match inside.strip_suffix('/') {
                Some(inside) => (inside, true),
                None => (inside, false),
            };
            let name_end = inside.find(char::is_whitespace).unwrap_or(inside.len());
            let name = local_name(&inside[..name_end])?.to_owned();
            events.push(Event::Start(name.clone()));
            if empty {
                events.push(Event::End(name));
            } else {
                open.push(name);
                if open.len() > MAX_DEPTH {
                    return Err(XmlError::TooDeep);
                }
            }
            rest = &after[end + 1..];
        }
    }
    if !open.is_empty() {
        return Err(XmlError::Unclosed);
    }
    Ok(events)
}

/// The name after any namespace prefix; an empty name is malformed.
fn local_name(name: &str) -> Result<&str, XmlError> {
    let local = name.rsplit(':').next().unwrap_or(name);
    if local.is_empty() || local.contains(['<', '>', '"', '\'', '=', '&']) {
        return Err(XmlError::Malformed);
    }
    Ok(local)
}

/// Where a start tag's `>` is, outside quoted attribute values.
fn tag_end(after: &str) -> Result<usize, XmlError> {
    let mut quote: Option<u8> = None;
    for (i, byte) in after.bytes().enumerate() {
        match (quote, byte) {
            (None, b'>') => return Ok(i),
            (None, b'"' | b'\'') => quote = Some(byte),
            (Some(q), _) if q == byte => quote = None,
            (None, b'<') => return Err(XmlError::Malformed),
            _ => {}
        }
    }
    Err(XmlError::Unclosed)
}

/// Just past the `>` that ends a `<!...>` declaration, skipping a bracketed
/// internal subset.
fn declaration_end(after: &str) -> Result<usize, XmlError> {
    let mut depth = 0usize;
    for (i, byte) in after.bytes().enumerate() {
        match byte {
            b'[' => depth += 1,
            b']' => depth = depth.saturating_sub(1),
            b'>' if depth == 0 => return Ok(i + 1),
            _ => {}
        }
    }
    Err(XmlError::Unclosed)
}

fn push_text(events: &mut Vec<Event>, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if let Some(Event::Text(previous)) = events.last_mut() {
        previous.push_str(text);
    } else {
        events.push(Event::Text(text.to_owned()));
    }
}

/// Decodes the five named entities and character references. Anything else
/// after an `&` is kept as it is written.
fn decode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|end| *end <= 12).and_then(|end| {
            let name = &rest[1..end];
            let ch = match name {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => {
                    let number = name.strip_prefix('#')?;
                    let code = match number.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => number.parse().ok()?,
                    };
                    char::from_u32(code)
                }
            }?;
            Some((ch, end + 1))
        });
        match decoded {
            Some((ch, used)) => {
                out.push(ch);
                rest = &rest[used..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Escapes text for an element's content or an attribute.
pub(super) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// The text of the first element called `name`: empty when it has no text
/// of its own before its first child.
pub(super) fn first_text<'a>(events: &'a [Event], name: &str) -> Option<&'a str> {
    let at = events
        .iter()
        .position(|event| matches!(event, Event::Start(n) if n == name))?;
    match events.get(at + 1) {
        Some(Event::Text(text)) => Some(text),
        _ => Some(""),
    }
}

/// The inside of every element called `name`, outermost ones only, in
/// document order.
pub(super) fn each<'a>(events: &'a [Event], name: &str) -> Vec<&'a [Event]> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < events.len() {
        if !matches!(&events[at], Event::Start(n) if n == name) {
            at += 1;
            continue;
        }
        let mut depth = 0usize;
        let mut end = events.len();
        for (j, event) in events.iter().enumerate().skip(at + 1) {
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) if depth == 0 => {
                    end = j;
                    break;
                }
                Event::End(_) => depth -= 1,
                Event::Text(_) => {}
            }
        }
        found.push(&events[at + 1..end]);
        at = end + 1;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitMix64;

    const DOC: &str = r#"<?xml version="1.0"?>
<!DOCTYPE root [ <!ENTITY x "y"> ]>
<!-- a router -->
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <device a='1>2' b="x">
    <friendlyName>Tom &amp; Jerry&#33; &#x3C;3 &bogus; & more</friendlyName>
    <serviceList>
      <service><serviceType> urn:x:1 </serviceType><controlURL>/a</controlURL></service>
      <service><serviceType><![CDATA[urn:<y>:2]]></serviceType><controlURL/></service>
    </serviceList>
  </device>
</root>"#;

    #[test]
    fn reads_a_description() {
        let events = parse(DOC).unwrap();
        assert_eq!(
            first_text(&events, "friendlyName"),
            Some("Tom & Jerry! <3 &bogus; & more")
        );
        let services = each(&events, "service");
        assert_eq!(services.len(), 2);
        assert_eq!(first_text(services[0], "serviceType"), Some("urn:x:1"));
        assert_eq!(first_text(services[0], "controlURL"), Some("/a"));
        assert_eq!(first_text(services[1], "serviceType"), Some("urn:<y>:2"));
        assert_eq!(first_text(services[1], "controlURL"), Some(""));
        assert_eq!(first_text(&events, "missing"), None);
        assert_eq!(each(&events, "serviceList").len(), 1);
    }

    #[test]
    fn drops_prefixes_and_checks_nesting() {
        let events =
            parse("<s:Envelope><s:Body><u:R><X>1</X></u:R></s:Body></s:Envelope>").unwrap();
        assert_eq!(events[0], Event::Start("Envelope".into()));
        assert_eq!(first_text(&events, "X"), Some("1"));
        assert_eq!(parse("<a><b></a></b>"), Err(XmlError::Mismatch));
        assert_eq!(parse("<a>"), Err(XmlError::Unclosed));
        assert_eq!(parse("<a"), Err(XmlError::Unclosed));
        assert_eq!(parse("<!-- x"), Err(XmlError::Unclosed));
        assert_eq!(parse("< >"), Err(XmlError::Malformed));
        assert_eq!(parse("</a>"), Err(XmlError::Mismatch));
        let deep = "<a>".repeat(MAX_DEPTH + 1) + &"</a>".repeat(MAX_DEPTH + 1);
        assert_eq!(parse(&deep), Err(XmlError::TooDeep));
        let long = "<a/>".repeat(MAX_EVENTS);
        assert_eq!(parse(&long), Err(XmlError::TooLong));
    }

    #[test]
    fn escapes_round_trip() {
        let text = "a<b>&\"c'";
        let events = parse(&format!("<x>{}</x>", escape(text))).unwrap();
        assert_eq!(first_text(&events, "x"), Some(text));
    }

    #[test]
    fn fuzzed_documents_never_panic() {
        let mut rng = SplitMix64::new(11);
        let alphabet = b"<>/!?-[]&#;:='\" \nax0";
        for round in 0..20_000 {
            let bytes: Vec<u8> = if round % 4 == 0 {
                (0..rng.below(200))
                    .map(|_| alphabet[rng.below(alphabet.len() as u64) as usize])
                    .collect()
            } else {
                let mut bytes = DOC.as_bytes().to_vec();
                for _ in 0..1 + rng.below(6) {
                    let at = rng.below(bytes.len() as u64) as usize;
                    match rng.below(4) {
                        0 => bytes[at] = rng.next_u64() as u8,
                        1 => bytes[at] = alphabet[rng.below(alphabet.len() as u64) as usize],
                        2 => {
                            bytes.remove(at);
                        }
                        _ => bytes.truncate(at),
                    }
                    if bytes.is_empty() {
                        break;
                    }
                }
                bytes
            };
            let text = String::from_utf8_lossy(&bytes);
            if let Ok(events) = parse(&text) {
                let _ = first_text(&events, "serviceType");
                for inside in each(&events, "service") {
                    let _ = first_text(inside, "controlURL");
                }
            }
        }
    }
}
