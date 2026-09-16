//! Bounded metadata for the reviewed static STRIP definition.
//! See docs/formats/native-strip.md. No callbacks or world initialization run.
mod placement;
use crate::{
    Result,
    aircraft::{Brf, fields, schema},
    invalid,
};
pub use placement::Placement;

#[derive(Debug)]
pub struct Definition {
    pub shape: String,
    pub flags: u32,
    /// All source tokens, including unknown fields and scaling markers.
    /// This is diagnostic metadata, not a configuration for simulation ticks.
    pub source: Brf,
}

impl Definition {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let source = Brf::parse(bytes)?;
        let object = fields(source.block("")?, schema::OBJECT)?;
        let number = |name: &str| -> Result<i32> {
            let token = &object[name];
            if token.scaled {
                return Err(invalid("scaled STRIP header field unsupported"));
            }
            token.number()
        };
        if number("structType")? != 1
            || number("typeSize")? != 166
            || number("instanceSize")? != 0
            || number("obj_class")? != 0x100
        {
            return Err(invalid("unsupported STRIP type layout/class"));
        }
        let names = &object["ot_names"];
        if names.kind != "ptr" || names.scaled {
            return Err(invalid("STRIP requires an identity reference"));
        }
        let identity = source.strings(&names.value)?;
        if identity.len() != 3
            || !identity[2].eq_ignore_ascii_case("STRIP.OT")
            || source.block(&names.value)?.iter().any(|token| token.scaled)
        {
            return Err(invalid("expected reviewed STRIP.OT identity"));
        }
        let callback = &object["utilProc"];
        if callback.scaled || callback.value != "_STRIPProc" {
            return Err(invalid("unsupported STRIP callback selector"));
        }
        // SetupOT resolves these slots too. The reviewed STRIP has both null;
        // reject new dependencies instead of claiming this narrow closure fits.
        let shadow = &object["shadowShape"];
        if shadow.kind != "dword" || shadow.scaled || shadow.number()? != 0 || number("unk0")? != 0
        {
            return Err(invalid("additional STRIP shape slots unsupported"));
        }
        let shape = &object["shape"];
        if shape.kind != "ptr" || shape.scaled {
            return Err(invalid("STRIP requires an explicit shape reference"));
        }
        let resources = source.strings(&shape.value)?;
        if resources.len() != 1 || source.block(&shape.value)?.iter().any(|token| token.scaled) {
            return Err(invalid("STRIP shape must reference one resource"));
        }
        let shape = resources[0].to_ascii_uppercase();
        if shape.len() > 12
            || !shape.ends_with(".SH")
            || shape.len() <= 3
            || shape.contains("..")
            || !shape
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.~$-".contains(&b))
        {
            return Err(invalid("unsupported STRIP shape resource name"));
        }
        let flags = number("flags")? as u32;
        Ok(Self {
            shape,
            flags,
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(edits: &[(&str, &str)]) -> Vec<u8> {
        let mut text = String::from("[brent's_relocatable_format]\n");
        for &(kind, name) in schema::OBJECT {
            let default = match name {
                "structType" => "byte 1",
                "typeSize" => "word 166",
                "obj_class" => "word $100",
                "flags" => "dword $80000001",
                "ot_names" => "ptr identity",
                "shape" => "ptr mesh",
                "utilProc" => "symbol _STRIPProc",
                "_acc" => "dword ^-7",
                _ => "",
            };
            if let Some((_, value)) = edits.iter().find(|(key, _)| *key == name) {
                text.push_str(value);
            } else if !default.is_empty() {
                text.push_str(default);
            } else {
                text.push_str(if kind == "ptr" { "dword" } else { kind });
                text.push_str(" 0");
            }
            text.push('\n');
        }
        text.push_str(":identity\nstring \"Synthetic runway\"\nstring \"Synthetic airport\"\nstring \"STRIP.OT\"\n:mesh\nstring \"test.SH\"\nend\n");
        text.into_bytes()
    }

    #[test]
    fn definition_preserves_uninterpreted_tokens_and_resolves_labels() {
        let definition = Definition::parse(&fixture(&[])).unwrap();
        assert_eq!(definition.shape, "TEST.SH");
        assert_eq!(definition.flags, 0x80000001);
        let raw = fields(definition.source.block("").unwrap(), schema::OBJECT).unwrap();
        assert!(raw["_acc"].scaled);
        assert_eq!(raw["_acc"].value, "-7");
        assert_eq!(raw["_acc"].number().unwrap(), -7);
    }

    #[test]
    fn definition_rejects_unreviewed_layouts_callbacks_and_dependencies() {
        for (field, value) in [
            ("typeSize", "word 165"),
            ("instanceSize", "word 1"),
            ("structType", "byte 5"),
            ("obj_class", "word $8100"),
            ("utilProc", "symbol _UnknownProc"),
            ("utilProc", "symbol ^_STRIPProc"),
            ("flags", "dword ^1"),
            ("shadowShape", "ptr mesh"),
            ("unk0", "dword 1"),
            ("shape", "dword 0"),
            ("shape", "ptr absent"),
            ("ot_names", "dword 0"),
        ] {
            assert!(
                Definition::parse(&fixture(&[(field, value)])).is_err(),
                "{field}: {value}"
            );
        }
        let source = String::from_utf8(fixture(&[])).unwrap();
        for replacement in ["../x.SH", "x\\y.SH", "", "test.PIC", "toolongfilename.SH"] {
            assert!(Definition::parse(source.replace("test.SH", replacement).as_bytes()).is_err());
        }
        assert!(Definition::parse(source.replace("STRIP.OT", "OTHER.OT").as_bytes()).is_err());
        assert!(
            Definition::parse(
                source
                    .replace(
                        "string \"test.SH\"",
                        "string \"test.SH\"\nstring \"extra.SH\""
                    )
                    .as_bytes()
            )
            .is_err()
        );
        assert!(Definition::parse(source.replace("end\n", "").as_bytes()).is_err());
        assert!(Definition::parse(&vec![b' '; 1024 * 1024 + 1]).is_err());
    }
}
