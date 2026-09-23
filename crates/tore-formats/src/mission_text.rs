//! Bounded reader for `.MT` mission text: numbered `.section` blocks of
//! briefing and debrief lines. Formatting directives stay as text for the
//! presenter. See docs/spec/debrief.md.
use crate::{Result, invalid};
use std::collections::BTreeMap;

const MAX_BYTES: usize = 64 * 1024;
const MAX_LINE: usize = 1024;
const MAX_SECTION: u8 = 16;

/// Lines of each numbered section. Section 3 is the success debrief and
/// section 4 the failure debrief.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissionText {
    pub sections: BTreeMap<u8, Vec<String>>,
}

impl MissionText {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(invalid("mission text too large"));
        }
        let mut sections = BTreeMap::new();
        let mut current: Option<u8> = None;
        // A final line break ends the last line rather than starting another.
        let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        for line in bytes.split(|b| *b == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.len() > MAX_LINE {
                return Err(invalid("mission text line too long"));
            }
            if line.iter().any(|b| !(32..=126).contains(b) && *b != b'\t') {
                return Err(invalid("mission text is not printable ASCII"));
            }
            let line = std::str::from_utf8(line).map_err(|_| invalid("mission text encoding"))?;
            if let Some(number) = line.trim().strip_prefix(".section ") {
                let number = number
                    .trim()
                    .parse::<u8>()
                    .ok()
                    .filter(|n| (1..=MAX_SECTION).contains(n))
                    .ok_or_else(|| invalid("invalid mission text section"))?;
                if sections.insert(number, Vec::new()).is_some() {
                    return Err(invalid("duplicate mission text section"));
                }
                current = Some(number);
            } else if let Some(section) = current {
                sections.get_mut(&section).unwrap().push(line.to_string());
            } else if !line.trim().is_empty() {
                return Err(invalid("mission text before first section"));
            }
        }
        Ok(Self { sections })
    }
    /// The debrief page for a result: section 3 on success, 4 otherwise.
    pub fn debrief(&self, success: bool) -> Option<&[String]> {
        self.sections
            .get(if success { &3 } else { &4 })
            .map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &[u8] = b".section 1\r\nTITLE\r\n.section 3\r\n.center\r\n.header\r\nWON\r\n\r\nWell done.\r\n.section 4\r\nLOST";
    #[test]
    fn sections_keep_directives_and_blank_lines() {
        let text = MissionText::parse(SAMPLE).unwrap();
        assert_eq!(text.sections[&1], ["TITLE"]);
        assert_eq!(
            text.debrief(true).unwrap(),
            [".center", ".header", "WON", "", "Well done."]
        );
        assert_eq!(text.debrief(false).unwrap(), ["LOST"]);
    }
    #[test]
    fn rejects_unbounded_or_malformed_text() {
        for bad in [
            &b"stray\n.section 1\n"[..],
            b".section 0\n",
            b".section 99\n",
            b".section 1\n.section 1\n",
            b".section 1\n\x01\n",
        ] {
            assert!(MissionText::parse(bad).is_err());
        }
        assert!(MissionText::parse(&vec![b' '; MAX_BYTES + 1]).is_err());
        assert!(MissionText::parse(&[b".section 1\n".as_slice(), &[b'a'; 2000]].concat()).is_err());
    }
}
