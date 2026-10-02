//! Opinionated repairs to the reviewed base F14 mesh. Retail files stay intact.
//! Geometry and UVs come from loaded source faces, never bundled art.
use tore_formats::shape::{Face, Shape};

// Corresponding right and left outlet/collar faces. Mirroring the complete
// collar keeps the repaired outlet connected to its surrounding body skin.
const COLLARS: [(usize, usize); 14] = [
    (0x335e, 0x26ef),
    (0x35c7, 0x2902),
    (0x35ec, 0x2810),
    (0x386b, 0x28e9),
    (0x396a, 0x29ad),
    (0x397d, 0x29c0),
    (0x4334, 0x4115),
    (0x441d, 0x4143),
    (0x44b6, 0x408c),
    (0x478e, 0x47a5),
    (0x48a6, 0x48d5),
    (0x48fc, 0x491b),
    (0x499d, 0x4800),
    (0x4a57, 0x4a40),
];
type Vertex = (usize, usize);
// Each triangle closes a three-edge boundary. References select the existing
// position, palette color and UV at each corner, retaining surrounding artwork.
const SEAMS: [(usize, [Vertex; 3]); 6] = [
    (0x2615, [(0x2615, 2), (0x2615, 3), (0x2c25, 1)]),
    (0x2dcb, [(0x2dcb, 3), (0x2dcb, 2), (0x2d5e, 0)]),
    (0x3448, [(0x3448, 0), (0x3448, 1), (0x32c3, 1)]),
    (0x2d5e, [(0x2d5e, 1), (0x2d5e, 0), (0x3f26, 1)]),
    (0x26ef, [(0x26ef, 1), (0x26ef, 0), (0x29c0, 0)]),
    (0x335e, [(0x335e, 0), (0x335e, 1), (0x397d, 1)]),
];

fn face(faces: &[Face], address: usize) -> Result<&Face, String> {
    faces
        .iter()
        .find(|f| f.address == address)
        .ok_or_else(|| format!("unreviewed F14 repair: missing face {address:x}"))
}

fn mirrored(source: &Face, address: usize) -> Face {
    let mut f = source.clone();
    f.address = address;
    for p in &mut f.positions {
        p[0] = -p[0];
    }
    if let Some(n) = &mut f.normal {
        n[0] = -n[0];
    }
    f.positions.reverse();
    f.colors.reverse();
    f.uv.reverse();
    f
}

fn seam(
    faces: &[Face],
    donor: usize,
    vertices: [Vertex; 3],
    address: usize,
) -> Result<Face, String> {
    let mut patch = face(faces, donor)?.clone();
    let textured = !patch.uv.is_empty();
    patch.address = address;
    patch.positions.clear();
    patch.colors.clear();
    patch.uv.clear();
    for (source, index) in vertices {
        let f = face(faces, source)?;
        if f.texture != patch.texture
            || f.positions.len() <= index
            || f.colors.len() <= index
            || (textured && f.uv.len() <= index)
        {
            return Err("unreviewed F14 seam vertex/material".into());
        }
        patch.positions.push(f.positions[index]);
        patch.colors.push(f.colors[index]);
        if textured {
            patch.uv.push(f.uv[index]);
        }
    }
    let a: [f32; 3] = std::array::from_fn(|i| patch.positions[1][i] - patch.positions[0][i]);
    let b: [f32; 3] = std::array::from_fn(|i| patch.positions[2][i] - patch.positions[0][i]);
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length <= 1e-6 {
        return Err("degenerate F14 seam repair".into());
    }
    // Positions are right/forward/up; stored normals are right/up/forward Q15.
    let mut normal = [n[0], n[2], n[1]].map(|v| v / length * 32767.);
    if patch
        .normal
        .is_some_and(|old| old.iter().zip(normal).map(|(a, b)| a * b).sum::<f32>() < 0.)
    {
        normal = normal.map(|v| -v);
        patch.positions.reverse();
        patch.colors.reverse();
        patch.uv.reverse();
    }
    patch.normal = Some(normal);
    Ok(patch)
}

fn fit_plume(face: &mut Face, min: f32, max: f32) -> Result<(), String> {
    let lo = face
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min);
    let hi = face
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    if hi <= lo || max <= min {
        return Err("degenerate F14 plume width".into());
    }
    for p in &mut face.positions {
        p[0] = min + (p[0] - lo) * (max - min) / (hi - lo);
    }
    Ok(())
}

/// Run after validating the original layout and collecting its device branches.
pub(crate) fn repair(shape: &mut Shape) -> Result<(), String> {
    let patches = SEAMS
        .iter()
        .enumerate()
        .map(|(i, &(donor, vertices))| seam(&shape.faces, donor, vertices, 0x10_0000 + i))
        .collect::<Result<Vec<_>, _>>()?;
    let copies = COLLARS
        .iter()
        .map(|&(right, left)| {
            face(&shape.faces, left)?;
            Ok((left, mirrored(face(&shape.faces, right)?, left)))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let outlet = face(&shape.faces, 0x48a6)?;
    let min = outlet
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min);
    let max = outlet
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    for address in [0x5963, 0x598a] {
        face(&shape.faces, address)?;
    }
    for (address, copy) in copies {
        *shape
            .faces
            .iter_mut()
            .find(|f| f.address == address)
            .unwrap() = copy;
    }
    for f in &mut shape.faces {
        if matches!(f.address, 0x5963 | 0x598a) {
            fit_plume(f, min, max)?;
        }
    }
    shape.faces.extend(patches);
    Ok(())
}

/// User-requested F-22N hook art, fitted to the F-14's existing root and tip.
/// Both shapes must have passed their own rig layout validation first.
pub(crate) fn replace_hook(shape: &mut Shape, donor: &Shape) -> Result<(), String> {
    let old = face(&shape.faces, 0x5836)?;
    let blade = face(&donor.faces, 0x40a1)?;
    if old.positions.len() != 3
        || old.positions[1] != old.positions[2]
        || blade.positions.len() != 4
    {
        return Err("unreviewed F14/F22N hook attachment".into());
    }
    let root = old.positions[1];
    let tip = old.positions[0];
    let midpoint = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| (a[i] + b[i]) * 0.5);
    let donor_root: [f32; 3] = midpoint(blade.positions[0], blade.positions[1]);
    let donor_tip: [f32; 3] = midpoint(blade.positions[2], blade.positions[3]);
    let vector = |a: [f32; 3], b: [f32; 3]| [b[1] - a[1], b[2] - a[2]];
    let from = vector(donor_root, donor_tip);
    let to = vector(root, tip);
    let length = |v: [f32; 2]| v[0].hypot(v[1]);
    if length(from) <= 1e-6 || length(to) <= 1e-6 {
        return Err("degenerate hook span".into());
    }
    let scale = length(to) / length(from);
    let angle = (to[1] as f64).atan2(to[0] as f64) - (from[1] as f64).atan2(from[0] as f64);
    let mut copies = Vec::new();
    for (source, address) in [(0x40a1, 0x5836), (0x40c0, 0x584b)] {
        face(&shape.faces, address)?;
        let mut f = face(&donor.faces, source)?.clone();
        if f.positions.len() != 4 || f.uv.len() != 4 || f.subtype != 0x6c {
            return Err("unreviewed textured F22N hook blade".into());
        }
        crate::additional_animation::turn(&mut f, donor_root, [1., 0., 0.], angle);
        for p in &mut f.positions {
            *p = std::array::from_fn(|i| root[i] + (p[i] - donor_root[i]) * scale);
        }
        // Keep the destination's switch/animation identity, and the donor's
        // material, UVs, transparent silhouette and opposite face normals.
        f.address = address;
        copies.push(f);
    }
    for f in copies {
        let address = f.address;
        *shape
            .faces
            .iter_mut()
            .find(|old| old.address == address)
            .unwrap() = f;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn panel(address: usize) -> Face {
        Face {
            positions: vec![[3., -14., 1.], [5., -14., 1.], [5., -14., -1.]],
            colors: vec![1, 2, 3],
            uv: vec![[0., 0.], [1., 0.], [1., 1.]],
            texture: "SYNTHETIC.PIC".into(),
            subtype: 0xfe,
            normal: Some([0., 0., -32767.]),
            address,
            fog: Default::default(),
        }
    }
    #[test]
    fn outlet_mirroring_keeps_materials_and_reverses_winding() {
        let source = panel(1);
        let left = mirrored(&source, 2);
        assert_eq!(
            left.positions,
            vec![[-5., -14., -1.], [-5., -14., 1.], [-3., -14., 1.]]
        );
        assert_eq!(left.uv, source.uv.iter().rev().copied().collect::<Vec<_>>());
        assert_eq!(left.colors, [3, 2, 1]);
        let roundtrip = mirrored(&left, 1);
        assert_eq!(roundtrip.positions, source.positions);
        assert_eq!(roundtrip.normal, source.normal);
        assert_eq!(roundtrip.uv, source.uv);
        assert_eq!(roundtrip.texture, source.texture);
        assert_eq!(roundtrip.colors, source.colors);
        assert_eq!(roundtrip.address, source.address);
    }
    #[test]
    fn a_seam_reuses_neighbor_positions_uvs_and_q15_normal_direction() {
        let mut a = panel(1);
        a.positions = vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]];
        a.normal = Some([0., -32767., 0.]);
        let mut b = a.clone();
        b.address = 2;
        b.positions[2] = [0., 1., 0.];
        b.uv[2] = [0., 1.];
        let p = seam(&[a.clone(), b.clone()], 1, [(1, 0), (1, 1), (2, 2)], 99).unwrap();
        assert!(p.normal.unwrap()[1] < -32766.);
        for (position, uv) in p.positions.iter().zip(&p.uv) {
            assert!([&a, &b].iter().any(|f| {
                f.positions
                    .iter()
                    .zip(&f.uv)
                    .any(|pair| pair == (position, uv))
            }));
        }
        assert!(seam(&[a], 1, [(1, 0), (1, 0), (1, 1)], 99).is_err());
    }
    #[test]
    fn plume_width_matches_outlet_without_changing_length_or_uv() {
        let mut f = panel(1);
        f.positions = vec![[2., -24., 0.], [5., -24., 0.], [5., -14., 0.]];
        let uv = f.uv.clone();
        fit_plume(&mut f, 3., 5.).unwrap();
        assert_eq!(
            f.positions,
            vec![[3., -24., 0.], [5., -24., 0.], [5., -14., 0.]]
        );
        assert_eq!(f.uv, uv);
    }
    #[test]
    fn donated_hook_keeps_its_art_and_fits_both_f14_endpoints() {
        // Synthetic blade and collapsed recipient, no retail mesh or texture.
        let mut blade = panel(0x40a1);
        blade.positions = vec![[0., -1., 0.], [0., 1., 0.], [0., -4., -4.], [0., -6., -4.]];
        blade.uv = vec![[1., 2.], [1., 4.], [8., 4.], [8., 2.]];
        blade.colors = vec![1; 4];
        blade.subtype = 0x6c;
        blade.normal = Some([32767., 0., 0.]);
        let back = mirrored(&blade, 0x40c0);
        let donor = Shape {
            faces: vec![blade.clone(), back],
            lines: vec![],
            state_words: Default::default(),
        };
        let root = [0., -5., -2.];
        let tip = [0., -13., -5.];
        let mut old = panel(0x5836);
        old.positions = vec![tip, root, root];
        let mut reverse = old.clone();
        reverse.address = 0x584b;
        let mut shape = Shape {
            faces: vec![old, reverse],
            lines: vec![],
            state_words: Default::default(),
        };
        replace_hook(&mut shape, &donor).unwrap();
        assert_eq!(shape.faces.len(), 2);
        let fitted = &shape.faces[0];
        assert_eq!(fitted.address, 0x5836);
        assert_eq!(fitted.uv, blade.uv);
        assert_eq!(fitted.texture, blade.texture);
        assert_eq!(
            fitted.subtype, 0x6c,
            "keep the texture's transparent silhouette"
        );
        for (indices, expected) in [([0, 1], root), ([2, 3], tip)] {
            for (axis, value) in expected.iter().enumerate() {
                assert!(
                    ((fitted.positions[indices[0]][axis] + fitted.positions[indices[1]][axis])
                        * 0.5
                        - value)
                        .abs()
                        < 1e-5
                );
            }
        }
        assert_eq!(
            fitted.normal.unwrap()[0],
            -shape.faces[1].normal.unwrap()[0]
        );
        assert_eq!(
            fitted.positions.iter().rev().copied().collect::<Vec<_>>(),
            shape.faces[1].positions
        );
        // A uniform fit preserves the donor's outline proportions.
        let distance = |a: [f32; 3], b: [f32; 3]| {
            a.iter()
                .zip(b)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>()
                .sqrt()
        };
        let scale = distance(root, tip) / 41f32.sqrt();
        for i in 0..4 {
            let j = (i + 1) % 4;
            assert!(
                (distance(fitted.positions[i], fitted.positions[j])
                    - scale * distance(blade.positions[i], blade.positions[j]))
                .abs()
                    < 1e-5
            );
        }
        let rig = crate::additional_animation::Rig::synthetic(
            tore_formats::aircraft::AircraftId::F14,
            &[],
            &[],
            &[],
            &[0x5836, 0x584b],
        );
        let mut state =
            crate::flight::State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        state.hook = 0.;
        assert!(shape.faces.iter().all(|f| rig.animate(f, &state).is_none()));
        state.hook = 1.;
        for (p, q) in rig
            .animate(fitted, &state)
            .unwrap()
            .positions
            .iter()
            .zip(&fitted.positions)
        {
            assert!(distance(*p, *q) < 1e-5);
        }
        state.hook = 0.5;
        let moving = rig.animate(fitted, &state).unwrap();
        assert_eq!(moving.uv, fitted.uv);
        assert_eq!(moving.texture, fitted.texture);
        assert_eq!(moving.subtype, fitted.subtype);
        // The production hinge is fixed; all donor vertices rotate rigidly
        // about it, with no leftover per-vertex triangle widening.
        let hinge = [0., -5., -2.];
        for (p, q) in fitted.positions.iter().zip(&moving.positions) {
            assert!((distance(*p, hinge) - distance(*q, hinge)).abs() < 1e-5);
        }
    }
}
