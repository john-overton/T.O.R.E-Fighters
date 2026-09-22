//! Give `tore-app.exe` its application icon on Windows.
//!
//! Explorer, the taskbar and the Alt-Tab switcher read an executable's icon
//! from its embedded `RT_GROUP_ICON` resource, so the icon has to be linked
//! into the binary rather than shipped beside it. The usual way to do that is
//! a crate such as `winres`, but the dependency budget in `AGENTS.md` is
//! `winit`, `wgpu`, `pollster` and `cpal`, so this script writes the resource
//! object itself.
//!
//! `link.exe` accepts a compiled `.res` file as an input on the command line,
//! which is exactly what `rc.exe` would have produced. The format is public
//! and small: a 32-byte null resource header, then one record per resource
//! made of a `RESOURCEHEADER` followed by the resource's bytes padded to a
//! four-byte boundary. Each image inside `assets/icon/tore.ico` becomes one
//! `RT_ICON`, and a `GRPICONDIR` naming them becomes the single
//! `RT_GROUP_ICON` that Windows actually looks up.
//!
//! The script does nothing at all on any target that is not `windows-msvc`,
//! so Linux and macOS builds are unaffected. `tools/package/build_icons.py
//! --verify-res PATH` parses a generated `.res` back and prints its entries,
//! which is how the layout below was checked without a Windows host.

use std::env;
use std::fs;
use std::path::PathBuf;

/// Path to the committed icon, relative to this crate's root.
const ICON_PATH: &str = "assets/icon/tore.ico";

/// Resource type ordinals from `winuser.h`.
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;

/// A `RESOURCEHEADER` with ordinal type and name is always 32 bytes.
const HEADER_SIZE: u32 = 32;
/// `MAKELANGID(LANG_ENGLISH, SUBLANG_ENGLISH_US)`.
const LANGUAGE_ID: u16 = 0x0409;
/// Memory flags are advisory in Win32; these are what `rc.exe` emits.
const MEMORY_MOVEABLE_DISCARDABLE: u16 = 0x1010;
const MEMORY_MOVEABLE_PURE_DISCARDABLE: u16 = 0x1030;

/// Sizes of the `.ico` structures, in bytes.
const ICONDIR_SIZE: usize = 6;
const ICONDIRENTRY_SIZE: usize = 16;
const GRPICONDIRENTRY_SIZE: usize = 14;

/// One image inside the `.ico`.
struct IconImage {
    width: u8,
    height: u8,
    colours: u8,
    planes: u16,
    bits_per_pixel: u16,
    data: Vec<u8>,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ICON_PATH}");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }

    let icon = match fs::read(ICON_PATH) {
        Ok(bytes) => bytes,
        Err(error) => {
            println!(
                "cargo:warning=Could not read {ICON_PATH}: {error}. Building without an icon."
            );
            return;
        }
    };
    let resource = match build_resource(&icon) {
        Ok(bytes) => bytes,
        Err(reason) => {
            println!("cargo:warning={ICON_PATH} is unusable: {reason}. Building without an icon.");
            return;
        }
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Cargo always sets OUT_DIR"));
    let path = out_dir.join("tore-icon.res");
    if let Err(error) = fs::write(&path, resource) {
        panic!("Could not write {}: {error}", path.display());
    }
    // link.exe takes a .res straight from the command line. Only the binaries
    // need it; tests and build scripts must not link a resource file.
    println!("cargo:rustc-link-arg-bins={}", path.display());
}

/// Turn the bytes of an `.ico` into the bytes of a Win32 `.res` file.
fn build_resource(icon: &[u8]) -> Result<Vec<u8>, String> {
    let images = parse_icon(icon)?;

    let mut out = Vec::new();
    // Every .res opens with a header describing a zero-length resource of type
    // 0 and name 0. Tools use it to tell a 32-bit .res from the 16-bit format.
    push_header(&mut out, 0, 0, 0, 0, 0);

    // Icon resource names are 1-based and must match the group's nId fields.
    for (index, image) in images.iter().enumerate() {
        let name = u16::try_from(index + 1).map_err(|_| "too many icon images".to_string())?;
        push_header(
            &mut out,
            u32::try_from(image.data.len()).map_err(|_| "icon image too large".to_string())?,
            RT_ICON,
            name,
            MEMORY_MOVEABLE_DISCARDABLE,
            LANGUAGE_ID,
        );
        push_padded(&mut out, &image.data);
    }

    let group = build_group(&images)?;
    push_header(
        &mut out,
        u32::try_from(group.len()).map_err(|_| "icon group too large".to_string())?,
        RT_GROUP_ICON,
        // Windows shows the lowest-numbered group icon as the application
        // icon, so this one is name 1.
        1,
        MEMORY_MOVEABLE_PURE_DISCARDABLE,
        LANGUAGE_ID,
    );
    push_padded(&mut out, &group);
    Ok(out)
}

/// Append a `RESOURCEHEADER` with an ordinal type and an ordinal name.
fn push_header(
    out: &mut Vec<u8>,
    data_size: u32,
    kind: u16,
    name: u16,
    memory: u16,
    language: u16,
) {
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    // An ordinal is the marker 0xFFFF followed by the number itself.
    out.extend_from_slice(&0xFFFF_u16.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&0xFFFF_u16.to_le_bytes());
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&memory.to_le_bytes()); // MemoryFlags
    out.extend_from_slice(&language.to_le_bytes()); // LanguageId
    out.extend_from_slice(&0_u32.to_le_bytes()); // Version
    out.extend_from_slice(&0_u32.to_le_bytes()); // Characteristics
}

/// Append resource data, padded with zeros to the next four-byte boundary.
fn push_padded(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(data);
    out.resize(out.len().next_multiple_of(4), 0);
}

/// Build the `GRPICONDIR` that names every `RT_ICON` written above.
fn build_group(images: &[IconImage]) -> Result<Vec<u8>, String> {
    let count = u16::try_from(images.len()).map_err(|_| "too many icon images".to_string())?;
    let mut group = Vec::with_capacity(ICONDIR_SIZE + GRPICONDIRENTRY_SIZE * images.len());
    group.extend_from_slice(&0_u16.to_le_bytes()); // idReserved
    group.extend_from_slice(&1_u16.to_le_bytes()); // idType, 1 for icons
    group.extend_from_slice(&count.to_le_bytes()); // idCount
    for (index, image) in images.iter().enumerate() {
        let id = u16::try_from(index + 1).map_err(|_| "too many icon images".to_string())?;
        let size =
            u32::try_from(image.data.len()).map_err(|_| "icon image too large".to_string())?;
        group.push(image.width);
        group.push(image.height);
        group.push(image.colours);
        group.push(0); // bReserved
        group.extend_from_slice(&image.planes.to_le_bytes());
        group.extend_from_slice(&image.bits_per_pixel.to_le_bytes());
        group.extend_from_slice(&size.to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    Ok(group)
}

/// Read the directory of an `.ico` and copy out each image.
fn parse_icon(icon: &[u8]) -> Result<Vec<IconImage>, String> {
    if icon.len() < ICONDIR_SIZE {
        return Err("shorter than an icon directory".to_string());
    }
    if read_u16(icon, 0) != 0 || read_u16(icon, 2) != 1 {
        return Err("not an icon file".to_string());
    }
    let count = usize::from(read_u16(icon, 4));
    if count == 0 {
        return Err("holds no images".to_string());
    }
    let directory_end = ICONDIR_SIZE + ICONDIRENTRY_SIZE * count;
    if icon.len() < directory_end {
        return Err("directory runs past the end of the file".to_string());
    }

    let mut images = Vec::with_capacity(count);
    for index in 0..count {
        let entry = ICONDIR_SIZE + ICONDIRENTRY_SIZE * index;
        let size = read_u32(icon, entry + 8) as usize;
        let offset = read_u32(icon, entry + 12) as usize;
        let end = offset
            .checked_add(size)
            .ok_or_else(|| format!("image {index} has an impossible extent"))?;
        if offset < directory_end || end > icon.len() {
            return Err(format!("image {index} lies outside the file"));
        }
        // A zero plane or depth means the writer left it to the image data.
        // Everything this project produces is a 32-bit single-plane image.
        let planes = match read_u16(icon, entry + 4) {
            0 => 1,
            value => value,
        };
        let bits_per_pixel = match read_u16(icon, entry + 6) {
            0 => 32,
            value => value,
        };
        images.push(IconImage {
            width: icon[entry],
            height: icon[entry + 1],
            colours: icon[entry + 2],
            planes,
            bits_per_pixel,
            data: icon[offset..end].to_vec(),
        });
    }
    Ok(images)
}

fn read_u16(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([data[at], data[at + 1]])
}

fn read_u32(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}
