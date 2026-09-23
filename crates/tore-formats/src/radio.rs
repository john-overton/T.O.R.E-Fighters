//! Reviewed wing and airport speech metadata. See docs/formats/radio.md.
use crate::{Result, invalid, slice, u16_at, u32_at};
use std::collections::BTreeMap;

/// Every reviewed phrase/recording pointer pair, 0x4fef10..0x4ff8f0, in the
/// 1.02F build. One address per stem: where a stem appears in several pairs,
/// the standalone phrase is kept over a sentence fragment. For the 1.0 disc
/// build subtract [`crate::executable::Layout::radio_shift`], which
/// [`phrases_with`] applies; all pairs share that shift.
pub const STEMS: &[(&str, usize)] = &[
    ("^NUM00", 0x4fef10),
    ("^NUM01", 0x4fef18),
    ("^NUM02", 0x4fef20),
    ("^NUM03", 0x4fef28),
    ("^NUM04", 0x4fef30),
    ("^NUM05", 0x4fef38),
    ("^NUM06", 0x4fef40),
    ("^NUM07", 0x4fef48),
    ("^NUM08", 0x4fef50),
    ("^NUM09", 0x4fef58),
    ("^NUM10", 0x4fef60),
    ("^NUM11", 0x4fef68),
    ("^NUM12", 0x4fef70),
    ("^MLTRY-A", 0x4fef78),
    ("^MLTRY-B", 0x4fef80),
    ("^MLTRY-C", 0x4fef88),
    ("^MLTRY-D", 0x4fef90),
    ("^MLTRY-E", 0x4fef98),
    ("^MLTRY-F", 0x4fefa0),
    ("^MLTRY-G", 0x4fefa8),
    ("^MLTRY-H", 0x4fefb0),
    ("^MLTRY-I", 0x4fefb8),
    ("^MLTRY-J", 0x4fefc0),
    ("^MLTRY-K", 0x4fefc8),
    ("^RED", 0x4fefd0),
    ("^BLUE", 0x4fefd8),
    ("^GREEN", 0x4fefe0),
    ("^BLACK", 0x4fefe8),
    ("^WHITE", 0x4feff0),
    ("^LOW", 0x4ff020),
    ("^HIGH", 0x4ff030),
    ("^HESAT", 0x4ff038),
    ("^HESYOUR", 0x4ff040),
    ("^BANDIT", 0x4ff060),
    ("^SHIP", 0x4ff068),
    ("^SAM", 0x4ff070),
    ("^TRIPLEA", 0x4ff078),
    ("^TANK", 0x4ff080),
    ("^VEHICLE", 0x4ff088),
    ("^STRUCTR", 0x4ff090),
    ("^MISSILE", 0x4ff098),
    ("^TARGET", 0x4ff0a0),
    ("^BANDITS", 0x4ff0a8),
    ("^SHIPS", 0x4ff0b0),
    ("^SAMS", 0x4ff0b8),
    ("^TRIPLAS", 0x4ff0c0),
    ("^TANKS", 0x4ff0c8),
    ("^VEHICLS", 0x4ff0d0),
    ("^STRUCTS", 0x4ff0d8),
    ("^MISSLES", 0x4ff0e0),
    ("^TARGETS", 0x4ff0e8),
    ("^YOUR", 0x4ff0f0),
    ("^MILE", 0x4ff0f8),
    ("^MILES", 0x4ff100),
    ("^PROCTO", 0x4ff108),
    ("^INBDTO", 0x4ff110),
    ("^BEARING", 0x4ff120),
    ("^DSCNDTO", 0x4ff128),
    ("^MAINTN", 0x4ff130),
    ("^CLIMBTO", 0x4ff138),
    ("^ANGELS", 0x4ff140),
    ("^CONTACT", 0x4ff148),
    ("^PAIROF", 0x4ff150),
    ("^MULTPLE", 0x4ff158),
    ("^PLSADVS", 0x4ff160),
    ("^2SHFORM", 0x4ff168),
    ("^BREAKRT", 0x4ff170),
    ("^BREAKLF", 0x4ff178),
    ("^BREAKHI", 0x4ff180),
    ("^BREAKLO", 0x4ff188),
    ("^STEADY", 0x4ff190),
    ("^APPRCRT", 0x4ff198),
    ("^APPRCLF", 0x4ff1a0),
    ("^APPRCHI", 0x4ff1a8),
    ("^APPRCLO", 0x4ff1b0),
    ("^APPRCST", 0x4ff1b8),
    ("^TIGHTEN", 0x4ff1c0),
    ("^CBTSPRD", 0x4ff1c8),
    ("^FORMHI", 0x4ff1d0),
    ("^FORMLVL", 0x4ff1d8),
    ("^FORMLOW", 0x4ff1e0),
    ("^ECHFORM", 0x4ff1e8),
    ("^ABRFORM", 0x4ff1f0),
    ("^ASTFORM", 0x4ff1f8),
    ("^LOSFORM", 0x4ff200),
    ("^MEDFORM", 0x4ff208),
    ("^TGTFORM", 0x4ff210),
    ("^DISENG", 0x4ff218),
    ("^CLRMY6", 0x4ff220),
    ("^WCHTAIL", 0x4ff228),
    ("^ATTACK", 0x4ff230),
    ("^EVADE", 0x4ff238),
    ("^BUGOUT", 0x4ff240),
    ("^FOXONE", 0x4ff248),
    ("^FOXTWO", 0x4ff250),
    ("^FOXTHR", 0x4ff258),
    ("^BOMBAWY", 0x4ff260),
    ("^FIRGUN", 0x4ff268),
    ("^IMSHOT", 0x4ff270),
    ("^MISSAWY", 0x4ff278),
    ("^FIRMISS", 0x4ff280),
    ("^WNGLDR", 0x4ff288),
    ("^BULLS1", 0x4ff290),
    ("^IMPACT", 0x4ff298),
    ("^OHYEAH", 0x4ff2a0),
    ("^ALRIGHT", 0x4ff2a8),
    ("^GDSHOT", 0x4ff2b0),
    ("^HEDAMGE", 0x4ff2c0),
    ("^MULTHIT", 0x4ff2c8),
    ("^FRAGGED", 0x4ff2d8),
    ("^HOTLEAD", 0x4ff2e0),
    ("^DEBRIS", 0x4ff2e8),
    ("^HURTIN", 0x4ff2f0),
    ("^OBJDEST", 0x4ff2f8),
    ("^GOODHIT", 0x4ff300),
    ("^GDKILL", 0x4ff308),
    ("^SPLBNDT", 0x4ff310),
    ("^YEEHAW1", 0x4ff320),
    ("^BEAUT1", 0x4ff328),
    ("^DNCNT", 0x4ff338),
    ("^CRSHBRN", 0x4ff340),
    ("^WIPEOUT", 0x4ff348),
    ("^BRKUP", 0x4ff350),
    ("^GOFLAM", 0x4ff358),
    ("^SPLMIG1", 0x4ff360),
    ("^SPLMIG2", 0x4ff368),
    ("^YEEHAW2", 0x4ff388),
    ("^BEAUT2", 0x4ff390),
    ("^GOTHIM", 0x4ff398),
    ("^BULLS2", 0x4ff3a0),
    ("^HOOHOO", 0x4ff3a8),
    ("^OHYES", 0x4ff3b0),
    ("^FIRBALL", 0x4ff3b8),
    ("^HISTORY", 0x4ff3c0),
    ("^WOOH", 0x4ff3c8),
    ("^SPLASH", 0x4ff3d0),
    ("^ENGAGE", 0x4ff3d8),
    ("^ISEEEM", 0x4ff3e0),
    ("^SHWTIME", 0x4ff3e8),
    ("^GETEM", 0x4ff3f0),
    ("^YAHOO1", 0x4ff3f8),
    ("^TALLYHO", 0x4ff400),
    ("^ONHIM", 0x4ff408),
    ("^IGO", 0x4ff410),
    ("^IGOAF", 0x4ff418),
    ("^IMHIT1", 0x4ff420),
    ("^IMDMGE1", 0x4ff428),
    ("^OFFME", 0x4ff430),
    ("^SCORCH", 0x4ff438),
    ("^HEAT", 0x4ff440),
    ("^IMHIT2", 0x4ff448),
    ("^IMDMGE2", 0x4ff450),
    ("^IMAAA", 0x4ff458),
    ("^EATLD", 0x4ff460),
    ("^APEXCHF", 0x4ff478),
    ("^ATOLFLR", 0x4ff480),
    ("^MISSBRK", 0x4ff488),
    ("^AARRRGH", 0x4ff490),
    ("^OHSH", 0x4ff498),
    ("^YAAAAAH", 0x4ff4a0),
    ("^EJECT", 0x4ff4a8),
    ("^SEEHELL", 0x4ff4b0),
    ("^PUNCH", 0x4ff4b8),
    ("^SAMLCH", 0x4ff4c0),
    ("^MISSLCH", 0x4ff4c8),
    ("^INRANGE", 0x4ff4d0),
    ("^TURNLFT", 0x4ff4e0),
    ("^TURNRGT", 0x4ff4e8),
    ("^HEADAWY", 0x4ff4f0),
    ("^MISSACC", 0x4ff4f8),
    ("^NOTPLSD", 0x4ff500),
    ("^NOMEDAL", 0x4ff508),
    ("^BLEWIT", 0x4ff510),
    ("^MESSUP", 0x4ff518),
    ("^SERIOUS", 0x4ff520),
    ("^ALMSTHM", 0x4ff528),
    ("^WHTHELL", 0x4ff538),
    ("^WTCHOUT", 0x4ff540),
    ("^YOUNUTS", 0x4ff548),
    ("^YOUCRZY", 0x4ff550),
    ("^WHOSIDE", 0x4ff558),
    ("^IMGOOD", 0x4ff560),
    ("^GETOFF", 0x4ff568),
    ("^IMYOUR", 0x4ff570),
    ("^BEEP2", 0x4ff5a8),
    ("^OUTGAS", 0x4ff5b0),
    ("^OUTFUEL", 0x4ff5b8),
    ("^WEFUMES", 0x4ff5c0),
    ("^IMFUMES", 0x4ff5c8),
    ("^BINGO", 0x4ff5d0),
    ("^JOKER", 0x4ff5d8),
    ("^EASEUP", 0x4ff5e0),
    ("^BARF1", 0x4ff5e8),
    ("^BARF2", 0x4ff5f0),
    ("^BARF3", 0x4ff5f8),
    ("^BARF4", 0x4ff600),
    ("^BARF5", 0x4ff608),
    ("^BARF6", 0x4ff610),
    ("^BARF7", 0x4ff618),
    ("^GRUNT1", 0x4ff620),
    ("^GRUNT2", 0x4ff628),
    ("^GRUNT3", 0x4ff630),
    ("^GRUNT4", 0x4ff638),
    ("^BREATH2", 0x4ff640),
    ("^BREATH3", 0x4ff648),
    ("^BREATH4", 0x4ff650),
    ("^FT_WETA", 0x4ff658),
    ("^FT_WETB", 0x4ff660),
    ("^FT_WETC", 0x4ff668),
    ("^FT_DRYA", 0x4ff670),
    ("^FT_DRYB", 0x4ff678),
    ("^FT_DRYC", 0x4ff680),
    ("^APPTRGT", 0x4ff688),
    ("^ATUS", 0x4ff690),
    ("^YRNOSE", 0x4ff698),
    ("^ATYOU", 0x4ff6a0),
    ("^OFFBEAM", 0x4ff6a8),
    ("^GETGUY", 0x4ff6b0),
    ("^CLOSING", 0x4ff6b8),
    ("^YAHOO2", 0x4ff6c0),
    ("^GOTNOW1", 0x4ff6c8),
    ("^GOTNOW2", 0x4ff6d0),
    ("^REELING", 0x4ff6d8),
    ("^WORM", 0x4ff6e0),
    ("^KNOCK", 0x4ff6e8),
    ("^SWCMISS", 0x4ff6f0),
    ("^DONTOVR", 0x4ff6f8),
    ("^CNTTONE", 0x4ff700),
    ("^LOCKHIM", 0x4ff708),
    ("^YAHOO3", 0x4ff710),
    ("^BURN1", 0x4ff728),
    ("^FINISH", 0x4ff730),
    ("^TKOUT", 0x4ff738),
    ("^COMARND", 0x4ff740),
    ("^ONTAIL", 0x4ff748),
    ("^WESHAKE", 0x4ff750),
    ("^INPOSI", 0x4ff758),
    ("^BREAK1", 0x4ff760),
    ("^BREAK2", 0x4ff768),
    ("^ONOUR6", 0x4ff770),
    ("^PLTSHT1", 0x4ff778),
    ("^PLTSHT2", 0x4ff780),
    ("^USOUT", 0x4ff788),
    ("^NOTGOOD", 0x4ff790),
    ("^BANDIT6", 0x4ff7a8),
    ("^GETOUT", 0x4ff7c0),
    ("^BANTAIL", 0x4ff7c8),
    ("^MNVR", 0x4ff7d0),
    ("^SHAKHM", 0x4ff7d8),
    ("^EVASV", 0x4ff7e0),
    ("^SLOWDWN", 0x4ff7e8),
    ("^DNTLIKE", 0x4ff7f0),
    ("^GUYGOOD", 0x4ff7f8),
    ("^NOROOK", 0x4ff800),
    ("^SMMOVE", 0x4ff808),
    ("^LKSKILL", 0x4ff810),
    ("^BURN2", 0x4ff818),
    ("^BRGARND", 0x4ff820),
    ("^LOSTHIM", 0x4ff828),
    ("^DO180", 0x4ff830),
    ("^VERTICL", 0x4ff838),
    ("^TAKOFF1", 0x4ff840),
    ("^RDYROLL", 0x4ff848),
    ("^TAKOFF2", 0x4ff850),
    ("^LAUNCH", 0x4ff858),
    ("^RDYCAT", 0x4ff868),
    ("^STANDBY", 0x4ff870),
    ("^CATONE", 0x4ff878),
    ("^CATFAIL", 0x4ff880),
    ("^AIRBORN", 0x4ff890),
    ("^ROTATE", 0x4ff898),
    ("^GDLUCK", 0x4ff8b0),
    ("^GDHUNT", 0x4ff8b8),
    ("^CLRDECK", 0x4ff8c0),
    ("^CLRLAND", 0x4ff8c8),
    ("^WINDAT", 0x4ff8d8),
    ("^KNOTS", 0x4ff8e0),
    ("^KNTGUST", 0x4ff8e8),
    ("^CALLBLL", 0x4ff8f0),
    ("^LWRGEAR", 0x4ff8f8),
    ("^LWRHOOK", 0x4ff900),
    ("^GOARND", 0x4ff908),
    ("^WAVEOFF", 0x4ff910),
    ("^GORIGHT", 0x4ff918),
    ("^GOLEFT", 0x4ff920),
    ("^LOWER", 0x4ff928),
    ("^HIGHER", 0x4ff930),
    ("^FASTER", 0x4ff938),
    ("^SLOWER", 0x4ff940),
    ("^MCHBANK", 0x4ff948),
    ("^ONBALL", 0x4ff950),
    ("^BADLAND", 0x4ff960),
    ("^FRLAND", 0x4ff968),
    ("^GDLAND", 0x4ff970),
    ("^WELBACK", 0x4ff978),
    ("^WELHOME", 0x4ff980),
    ("^NOBADET", 0x4ff998),
    ("^NOTOREP", 0x4ff9a0),
    ("^BANSVIR", 0x4ff9b0),
];

pub const AIRPORT_CLEAR_TO_LAND: &str = "^CLRLAND";
pub const AIRPORT_WELCOME_HOME: &str = "^WELHOME";

/// Speech recordings: every `^` (pilot radio) or `#` (second voice) `.5K`
/// resource with a short safe stem. Importing a sample assigns it no meaning;
/// only reviewed consumers select it. The whole set is about 3 MB.
pub fn resource(name: &str) -> bool {
    name.strip_suffix(".5K").is_some_and(|stem| {
        let mut chars = stem.chars();
        matches!(chars.next(), Some('^' | '#'))
            && (2..=8).contains(&stem.len())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    })
}

/// Select bounded inert records from the reviewed executable layout.
/// Expected stems are layout guards, not substitute metadata.
pub fn phrases(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    phrases_with(crate::executable::identify(data)?, data)
}

/// Read the pointer pairs using an already chosen build address set.
pub fn phrases_with(
    build: &crate::executable::Layout,
    data: &[u8],
) -> Result<BTreeMap<String, Vec<u8>>> {
    if data.len() > 16 * 1024 * 1024 || slice(data, 0, 2)? != b"MZ" {
        return Err(invalid("invalid radio image"));
    }
    let pe = u32_at(data, 60)?;
    if slice(data, pe, 4)? != b"PE\0\0" || u16_at(data, pe + 4)? != 0x14c {
        return Err(invalid("unsupported radio image"));
    }
    let count = u16_at(data, pe + 6)?;
    let optional = u16_at(data, pe + 20)?;
    if count > 32 || optional < 32 || u32_at(data, pe + 52)? != 0x400000 {
        return Err(invalid("unsupported radio layout"));
    }
    let mut section = None;
    for i in 0..count {
        let s = slice(data, pe + 24 + optional + i * 40, 40)?;
        if &s[..8] == b".data\0\0\0" {
            if section.is_some() {
                return Err(invalid("duplicate radio section"));
            }
            section = Some((
                slice(data, u32_at(s, 20)?, u32_at(s, 16)?.min(u32_at(s, 8)?))?,
                0x400000 + u32_at(s, 12)?,
            ));
        }
    }
    let (bytes, base) = section.ok_or_else(|| invalid("missing radio section"))?;
    let offset = |va: usize| {
        va.checked_sub(base)
            .ok_or_else(|| invalid("radio pointer outside section"))
    };
    let string = |va: usize| -> Result<&[u8]> {
        let start = offset(va)?;
        let rest = bytes
            .get(start..)
            .ok_or_else(|| invalid("radio pointer outside section"))?;
        let end = rest
            .iter()
            .take(128)
            .position(|b| *b == 0)
            .ok_or_else(|| invalid("unterminated radio text"))?;
        let text = &rest[..end];
        if text.is_empty() || !text.iter().all(|b| (32..127).contains(b)) {
            return Err(invalid("invalid radio text"));
        }
        Ok(text)
    };
    let mut result = BTreeMap::new();
    for (stem, va) in STEMS {
        let at = offset(
            va.checked_sub(build.radio_shift)
                .ok_or_else(|| invalid("radio pointer outside section"))?,
        )?;
        let text = string(u32_at(bytes, at)?)?;
        if string(u32_at(bytes, at + 4)?)? != stem.as_bytes() {
            return Err(invalid("unreviewed radio mapping"));
        }
        result.insert(format!("TORE_RADIO_{stem}"), text.to_vec());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The same synthetic pointer pairs at each reviewed build's addresses.
    fn fixture(build: &crate::executable::Layout) -> Vec<u8> {
        let base = 0x4fe000 - build.radio_shift;
        let mut data = vec![0u8; 0x4000];
        let mut pos = 0x2000;
        for (stem, va) in STEMS {
            let at = va - 0x4fe000;
            for (slot, text) in [(0, "Synthetic phrase"), (4, *stem)] {
                data[at + slot..at + slot + 4]
                    .copy_from_slice(&((base + pos) as u32).to_le_bytes());
                data[pos..pos + text.len()].copy_from_slice(text.as_bytes());
                pos += text.len() + 1;
            }
        }
        crate::executable::fixture(&[(".data", base, data, false)])
    }
    #[test]
    fn both_reviewed_builds_decode_the_same_phrases() {
        let [disc, patch] = crate::executable::LAYOUTS;
        let a = phrases_with(&disc, &fixture(&disc)).unwrap();
        assert_eq!(a, phrases_with(&patch, &fixture(&patch)).unwrap());
        assert_eq!(a.len(), STEMS.len());
        assert!(a.contains_key("TORE_RADIO_^ENGAGE"));
        // Unknown builds are refused rather than read at guessed addresses.
        assert!(phrases(&fixture(&patch)).is_err());
    }
    #[test]
    fn bounded_metadata_and_layout_guards() {
        let patch = &crate::executable::LAYOUTS[1];
        let b = fixture(patch);
        assert_eq!(phrases_with(patch, &b).unwrap().len(), STEMS.len());
        for n in 0..b.len() {
            assert!(phrases_with(patch, &b[..n]).is_err());
        }
        let start = b.len() - 0x4000;
        let mut bad = b.clone();
        bad[start + 0x1170..start + 0x1174].fill(255);
        assert!(phrases_with(patch, &bad).is_err());
        let mut bad = b;
        bad[start + 0x2000] = 0;
        assert!(phrases_with(patch, &bad).is_err());
        assert!(resource("^ENGAGE.5K"));
        assert!(resource("#TALLYHO.5K"));
        assert!(resource("^MLTRY-A.5K"));
        assert!(!resource("&EJECT.5K"));
        assert!(!resource("^ENGAGE.11K"));
        assert!(!resource("^TOOLONGXX.5K"));
        assert!(!resource("^../X.5K"));
        assert!(!resource("^.5K"));
    }
}
