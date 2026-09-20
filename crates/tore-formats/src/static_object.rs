//! General inert static OT definition reader. It exposes only reviewed references.
use crate::{
    Result,
    aircraft::{Brf, fields, schema},
    invalid,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub display_name: String,
    pub class_name: String,
    pub resource_name: String,
    pub main_shape: Option<String>,
    pub callbacks: Vec<String>,
    pub hit_points: Option<i32>,
    pub category: u16,
    pub radar_signature: i32,
    pub infrared_signature: i32,
}

impl Definition {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let brf = Brf::parse(bytes)?;
        let root = brf.block("")?;
        let prefix = root
            .get(..schema::OBJECT.len())
            .ok_or_else(|| invalid("static definition shorter than OBJ_TYPE"))?;
        let object = fields(prefix, schema::OBJECT)?;
        let identity = &object["ot_names"];
        if identity.kind != "ptr" || identity.scaled {
            return Err(invalid("static OT requires an identity reference"));
        }
        let names = brf.strings(&identity.value)?;
        if names.len() != 3 {
            return Err(invalid("static OT expected three identity strings"));
        }
        let shape = &object["shape"];
        let shapes = if shape.kind == "dword" && !shape.scaled && shape.number()? == 0 {
            Vec::new()
        } else if shape.kind == "ptr" && !shape.scaled {
            brf.strings(&shape.value)?
        } else {
            return Err(invalid("invalid static OT main shape reference"));
        };
        if shapes.len() > 1 {
            return Err(invalid("static OT expected at most one main shape"));
        }
        let valid_resource = |name: &str| {
            !name.is_empty()
                && name.len() <= 64
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'$'))
        };
        if !valid_resource(&names[2]) || shapes.first().is_some_and(|shape| !valid_resource(shape))
        {
            return Err(invalid("invalid static OT resource reference"));
        }
        let callbacks = root
            .iter()
            .filter(|token| token.kind == "symbol")
            .map(|token| token.value.clone())
            .collect();
        let scalar = |key: &str| -> Result<i32> {
            let field = &object[key];
            if field.scaled {
                return Err(invalid("scaled static object statistic unsupported"));
            }
            field.number()
        };
        let hit_points = Some(scalar("hitPoints")?).filter(|v| *v > 0);
        let category = scalar("obj_class")? as u16;
        let radar_signature = scalar("sigs[3]")?;
        let infrared_signature = scalar("sigs[2]")?;
        if radar_signature < 0 || infrared_signature < 0 {
            return Err(invalid("negative static object signature"));
        }
        Ok(Self {
            display_name: names[0].clone(),
            class_name: names[1].clone(),
            resource_name: names[2].to_ascii_uppercase(),
            main_shape: shapes.first().map(|shape| shape.to_ascii_uppercase()),
            callbacks,
            hit_points,
            category,
            radar_signature,
            infrared_signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_shape_and_inert_callback() {
        let mut root = String::from("[brent's_relocatable_format]\n");
        for (kind, name) in schema::OBJECT {
            let value = match *name {
                "ot_names" => "ot_names",
                "shape" => "shape",
                "hitPoints" => "100",
                "utilProc" => "_ThingProc",
                _ if *kind == "ptr" => "0",
                _ if *kind == "symbol" => "0",
                _ => "0",
            };
            let output_kind = if *kind == "ptr" && value == "0" {
                "dword"
            } else {
                kind
            };
            root.push_str(&format!("{output_kind} {value}\n"));
        }
        root.push_str(":ot_names\nstring \"Tower\"\nstring \"Building\"\nstring \"TOWER.OT\"\n:shape\nstring \"tower.SH\"\nend\n");
        let d = Definition::parse(root.as_bytes()).unwrap();
        assert_eq!(d.main_shape.as_deref(), Some("TOWER.SH"));
        assert_eq!(d.callbacks, vec!["_ThingProc"]);
        assert_eq!(d.hit_points, Some(100));
    }
}
