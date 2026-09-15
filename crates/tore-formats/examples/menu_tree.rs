//! Inspect bounded menu data from extracted user-owned modules.
use std::{env, fs::File, io::Read};
use tore_formats::ui::{MenuNode, menu_tree};

fn print_tree(nodes: &[MenuNode], depth: usize) {
    for node in nodes {
        println!("{}{} [{}]", "  ".repeat(depth), node.label, node.shortcut);
        print_tree(&node.children, depth + 1);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: menu_tree EXTRACTED.MNU ...".into());
    }
    for path in paths {
        let mut data = Vec::new();
        File::open(&path)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut data)?;
        println!("{}", std::path::Path::new(&path).display());
        print_tree(&menu_tree(&data)?, 0);
    }
    Ok(())
}
