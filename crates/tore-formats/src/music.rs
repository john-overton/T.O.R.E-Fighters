//! Reviewed FA music data. MUS is an inert score grammar, never executable code.
use crate::{Result, invalid, slice, u32_at};
use std::collections::{BTreeMap, BTreeSet};

pub const SCORES: &[&str] = &[
    "M_NORMAL.MUS",
    "M_AIR.MUS",
    "M_DANGER.MUS",
    "M_DECK.MUS",
    "M_LAUNCH.MUS",
    "M_HOME.MUS",
    "M_EJECT.MUS",
    "M_SUCC.MUS",
    "M_VALK.MUS",
];
// FA.EXE ShellMusicUpdate tables, verified against the reviewed build.
pub const MAIN: &[&str] = &[
    "XFI311AB.11K",
    "XFI310CA.11K",
    "XFI310CB.11K",
    "XFI526C.11K",
    "AIR28A.11K",
    "AIR02A.11K",
    "AIR30A.11K",
    "AIR30B.11K",
];
pub const BRIEF: &[&str] = &[
    "XFI204CA.11K",
    "XFI204CB.11K",
    "XFI107B.11K",
    "AIR37A.11K",
    "AIR14A.11K",
    "AIR14A.11K",
];
pub const WIN: &[&str] = &["XFI105B.11K", "XFI108B.11K", "AIR25A.11K"];
pub const LOSE: &[&str] = &[
    "XFI109B.11K",
    "XFI104B.11K",
    "AIR48A.11K",
    "AIR26A.11K",
    "AIR39A.11K",
];

/// Shared resource selection; no archive/title filename assumptions. MIDI is not selected.
pub fn resource(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    SCORES.contains(&name.as_str())
        || MAIN
            .iter()
            .chain(BRIEF)
            .chain(WIN)
            .chain(LOSE)
            .any(|n| *n == name)
        || name == "FINAL6.11K"
        || name.strip_suffix(".11K").is_some_and(|stem| {
            ["AIR", "VALK"].iter().any(|p| {
                stem.strip_prefix(p).is_some_and(|id| {
                    !id.is_empty() && id.len() <= 3 && id.bytes().all(|b| b.is_ascii_digit())
                })
            })
        })
}

#[derive(Debug)]
enum Op {
    Prefix,
    Track(u8),
    Flag,
    ChanceJump(u8, usize),
    ChanceTrack(u8, u8),
    Stop,
    Random(Vec<u8>),
    Jump(usize),
}
#[derive(Debug)]
pub struct Score {
    pub prefix: String,
    ops: BTreeMap<usize, (Op, usize)>,
    pub tracks: BTreeSet<u8>,
    pub unreachable_bytes: usize,
}
#[derive(Default, Debug)]
pub struct Cursor {
    offset: usize,
    pub stopped: bool,
    /// F9 marks a reevaluation boundary; the host clears the flag once it has read it.
    pub host_flag: bool,
}
impl Score {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        Self::from_code(crate::module::code(bytes)?.0)
    }
    pub fn from_code(code: &[u8]) -> Result<Self> {
        if code.is_empty() || code.len() > 65536 || code[0] != 255 {
            return Err(invalid(
                "score requires a bounded CODE section and initial prefix",
            ));
        }
        let mut ops = BTreeMap::new();
        let mut occupied = BTreeSet::new();
        let mut pending = vec![0];
        let mut prefix = String::new();
        let mut tracks = BTreeSet::new();
        while let Some(start) = pending.pop() {
            if ops.contains_key(&start) {
                continue;
            }
            if occupied.contains(&start) || ops.len() >= 4096 {
                return Err(invalid("score overlap or instruction limit"));
            }
            let opcode = slice(code, start, 1)?[0];
            let mut end = start + 1;
            let mut byte = || -> Result<u8> {
                let v = slice(code, end, 1)?[0];
                end += 1;
                Ok(v)
            };
            let op = match opcode {
                255 => {
                    let tail = code.get(end..).ok_or_else(|| invalid("score prefix"))?;
                    let len = tail
                        .iter()
                        .take(32)
                        .position(|b| *b == 0)
                        .ok_or_else(|| invalid("unterminated score prefix"))?;
                    if len == 0 || !tail[..len].iter().all(u8::is_ascii_alphanumeric) {
                        return Err(invalid("unsafe score prefix"));
                    }
                    let p = String::from_utf8(tail[..len].to_vec())
                        .unwrap()
                        .to_ascii_uppercase();
                    if !prefix.is_empty() && prefix != p {
                        return Err(invalid("multiple score prefixes require review"));
                    }
                    prefix = p;
                    end += len + 1;
                    Op::Prefix
                }
                254 => {
                    let target = u32_at(code, end)?;
                    end += 4;
                    Op::Jump(target)
                }
                253 => {
                    let count = byte()? as usize;
                    if count == 0 {
                        return Err(invalid("empty score choice"));
                    }
                    let choices = slice(code, end, count)?.to_vec();
                    end += count;
                    if choices.iter().any(|n| *n == 0 || *n >= 249) {
                        return Err(invalid("invalid score track"));
                    }
                    tracks.extend(&choices);
                    Op::Random(choices)
                }
                252 => Op::Stop,
                251 => {
                    let chance = byte()?;
                    let track = byte()?;
                    if chance > 100 || track == 0 || track >= 249 {
                        return Err(invalid("invalid score chance/track"));
                    }
                    tracks.insert(track);
                    Op::ChanceTrack(chance, track)
                }
                250 => {
                    let chance = byte()?;
                    if chance > 100 {
                        return Err(invalid("invalid score chance"));
                    }
                    let target = u32_at(code, end)?;
                    end += 4;
                    Op::ChanceJump(chance, target)
                }
                249 => Op::Flag,
                n => {
                    if n != 0 {
                        tracks.insert(n);
                    }
                    Op::Track(n)
                }
            };
            for at in start..end {
                if !occupied.insert(at) {
                    return Err(invalid("overlapping score operands"));
                }
            }
            match op {
                Op::Jump(target) => pending.push(target),
                Op::ChanceJump(_, target) => {
                    pending.push(target);
                    pending.push(end);
                }
                Op::Stop => (),
                _ => pending.push(end),
            }
            ops.insert(start, (op, end));
        }
        Ok(Self {
            prefix,
            ops,
            tracks,
            unreachable_bytes: code.len() - occupied.len(),
        })
    }
    pub fn filename(&self, track: u8) -> String {
        format!("{}{:03}.11K", self.prefix, track)
    }
    /// Call only when the previous phrase finishes. RNG belongs to the caller.
    /// Fixed budget also bounds valid but nonproductive cycles. No allocation here.
    pub fn next(
        &self,
        cursor: &mut Cursor,
        mut draw: impl FnMut(u32) -> u32,
    ) -> Result<Option<u8>> {
        if cursor.stopped {
            return Ok(None);
        }
        for _ in 0..1024 {
            let (op, end) = self
                .ops
                .get(&cursor.offset)
                .ok_or_else(|| invalid("invalid score cursor"))?;
            cursor.offset = *end;
            match op {
                Op::Prefix | Op::Track(0) => (),
                Op::Track(n) => return Ok(Some(*n)),
                Op::Flag => cursor.host_flag = true,
                Op::ChanceJump(chance, target) => {
                    if draw(100) % 100 < *chance as u32 {
                        cursor.offset = *target;
                    }
                }
                Op::ChanceTrack(chance, track) => {
                    if draw(100) % 100 < *chance as u32 {
                        return Ok(Some(*track));
                    }
                }
                Op::Stop => {
                    cursor.stopped = true;
                    return Ok(None);
                }
                Op::Random(choices) => {
                    return Ok(Some(
                        choices[draw(choices.len() as u32) as usize % choices.len()],
                    ));
                }
                Op::Jump(target) => cursor.offset = *target,
            }
        }
        cursor.stopped = true;
        Err(invalid("score instruction budget exhausted"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phrases_flags_chances_and_stop() {
        let score = Score::from_code(&[255, b'T', 0, 249, 251, 0, 3, 253, 2, 4, 5, 252]).unwrap();
        let mut cursor = Cursor::default();
        assert_eq!(score.next(&mut cursor, |_| 1).unwrap(), Some(5));
        assert!(cursor.host_flag);
        assert_eq!(score.next(&mut cursor, |_| 0).unwrap(), None);
        assert_eq!(score.next(&mut cursor, |_| 0).unwrap(), None);
        assert_eq!(score.filename(5), "T005.11K");
    }
    #[test]
    fn malformed_and_nonproductive_scores_are_bounded() {
        let good = [255, b'T', 0, 3, 254, 3, 0, 0, 0];
        let score = Score::from_code(&good).unwrap();
        let mut c = Cursor::default();
        for _ in 0..10 {
            assert_eq!(score.next(&mut c, |_| 0).unwrap(), Some(3));
        }
        for n in 0..good.len() {
            assert!(Score::from_code(&good[..n]).is_err());
        }
        let mut overlap = good;
        overlap[5] = 1;
        assert!(Score::from_code(&overlap).is_err());
        let looped = Score::from_code(&[255, b'T', 0, 254, 0, 0, 0, 0]).unwrap();
        assert!(looped.next(&mut Cursor::default(), |_| 0).is_err());
        assert!(Score::from_code(&[255, b'T', 0, 253, 0, 252]).is_err());
        assert!(Score::from_code(&[255, b'T', 0, 251, 101, 1, 252]).is_err());
    }
    #[test]
    fn profile_excludes_midi_speech_and_art() {
        for n in ["air003.11k", "M_NORMAL.MUS", "XFI311AB.11K", "FINAL6.11K"] {
            assert!(resource(n));
        }
        for n in ["AIR003.XMI", "AIRMEDAL.PIC", "^MISSBRK.5K", "AIR../.11K"] {
            assert!(!resource(n));
        }
    }
}
