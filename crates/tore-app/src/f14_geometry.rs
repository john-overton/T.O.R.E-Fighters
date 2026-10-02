//! Opinionated repairs to the reviewed base F14 mesh. Retail files stay intact.
//! Geometry and UVs come from the loaded neighboring faces, never bundled art.
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
}
