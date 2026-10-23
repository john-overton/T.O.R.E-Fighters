//! The surface data the import keeps for Quick Mission ground targets (slice
//! IM1), and the lookup the game reads it through.
//!
//! The import keeps all 129 ground target templates (`~Q*.M`), every record
//! they, the executable's equipment lists and the base layouts can name (NT
//! surface units, OT static objects, PT aircraft parked as targets), the
//! weapon and sensor records of those, and every shape, damaged shape and
//! texture they use. What is kept is worked out from the data by
//! [`tore_formats::surface_set::select`]; this module holds the marker that says
//! a pack has it and the lookups on a loaded pack.
//!
//! Everything is stored under its retail name, so a loader that reads a pack
//! reads these records the way it reads any other. Behaviour:
//! `docs/spec/import-cache.md`.
use crate::{ImportResult, Resources};
use tore_formats::surface_set::{self, Missing};

/// The marker the import writes with the surface data. A pack without it
/// predates the data and must be re-imported ([`crate::check_markers`] asks).
pub const MARKER: &str = "TORE_SURFACE_V1";
/// What the marker holds.
pub const MARKER_VALUE: &[u8] = b"SURF1";

/// Whether the pack was imported with the surface data.
pub fn present(resources: &Resources) -> bool {
    resources.get(MARKER).map(Vec::as_slice) == Some(MARKER_VALUE)
}

/// The resource name of a template: `QUCOL` and `~QUCOL.M` both give
/// `~QUCOL.M`.
pub fn template_resource(stem: &str) -> String {
    let upper = stem.to_ascii_uppercase();
    if upper.starts_with('~') && upper.ends_with(".M") {
        upper
    } else {
        format!("~{upper}.M")
    }
}

/// One template's bytes by its stem (`QUCOL`) or resource name.
pub fn template<'a>(resources: &'a Resources, stem: &str) -> Option<&'a [u8]> {
    resources.get(&template_resource(stem)).map(Vec::as_slice)
}

/// Every template in the pack as `(resource name, bytes)`, in name order.
pub fn templates(resources: &Resources) -> impl Iterator<Item = (&str, &[u8])> {
    resources
        .iter()
        .filter(|(name, _)| name.starts_with("~Q") && name.ends_with(".M"))
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
}

/// What the surface round needs that the pack lacks, worked out from the pack's
/// own templates, the executable's equipment lists, the base layouts and the
/// records they name. Empty when the pack is complete. This reads about 1,500
/// records, so it belongs in tests, tools and diagnostics, not on every start.
pub fn missing(resources: &Resources) -> ImportResult<Vec<Missing>> {
    let selection = surface_set::select(resources)?;
    let mut missing: Vec<Missing> = selection.missing.into_iter().collect();
    // A template the pack holds but the data cannot read is a gap too.
    for (name, error) in selection.unread {
        missing.push(Missing {
            name,
            needed_by: format!("unreadable: {error}"),
        });
    }
    // Every texture the shape reader names in a kept shape must be kept.
    for name in &selection.resources {
        if name.ends_with(".SH")
            && let Some(bytes) = resources.get(name)
        {
            for texture in surface_set::shape_textures(bytes) {
                if !resources.contains_key(&texture) {
                    missing.push(Missing {
                        name: texture,
                        needed_by: name.clone(),
                    });
                }
            }
        }
    }
    missing.sort();
    missing.dedup();
    Ok(missing)
}

/// [`missing`] as an error that names the first few gaps and says to re-import.
pub fn check(resources: &Resources) -> ImportResult<()> {
    let missing = missing(resources)?;
    if missing.is_empty() {
        return Ok(());
    }
    let shown: Vec<String> = missing
        .iter()
        .take(5)
        .map(|m| format!("{} ({})", m.name, m.needed_by))
        .collect();
    Err(format!(
        "cache lacks {} surface resources, such as {}; re-import media",
        missing.len(),
        shown.join(", ")
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_names_accept_a_stem_or_a_resource() {
        assert_eq!(template_resource("qucol"), "~QUCOL.M");
        assert_eq!(template_resource("~QUCOL.M"), "~QUCOL.M");
        let mut resources = Resources::new();
        resources.insert("~QUCOL.M".into(), vec![1]);
        resources.insert("~UKR1.MM".into(), vec![2]);
        assert_eq!(template(&resources, "QUCOL"), Some(&[1u8][..]));
        assert_eq!(template(&resources, "QNOPE"), None);
        assert_eq!(templates(&resources).count(), 1);
        assert!(!present(&resources));
        resources.insert(MARKER.into(), MARKER_VALUE.to_vec());
        assert!(present(&resources));
    }

    #[test]
    fn the_marker_name_fits_the_pack() {
        assert!((1..=32).contains(&MARKER.len()));
    }

    #[test]
    fn an_empty_pack_lacks_every_template() {
        let error = check(&Resources::new()).unwrap_err().to_string();
        assert!(error.contains("re-import media"), "{error}");
        assert!(error.contains("lacks"), "{error}");
        let gaps = missing(&Resources::new()).unwrap();
        assert!(gaps.iter().any(|gap| gap.name == "~QUCOL.M"));
    }
}
