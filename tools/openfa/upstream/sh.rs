// This file is part of OpenFA.
//
// OpenFA is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// OpenFA is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with OpenFA.  If not, see <http://www.gnu.org/licenses/>.
use anyhow::Result;
use catalog::{AssetCatalog, FileSystem, Search};
use clap::Args;
use installations::Installations;
use sh::{DrawSelection, ShCode};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

/// SH format slicing and discovery tooling
#[derive(Debug, Args)]
pub struct ShCommand {
    /// Show the min and max coordinates
    #[arg(short = 'e', long = "extents")]
    show_extents: bool,

    /// Write instructions to a YAML file
    #[arg(long, name = "out.yaml")]
    yaml: Option<String>,

    #[arg(long)]
    custom: bool,

    /// Shape files to display
    #[arg()]
    inputs: Vec<String>,
}

pub fn handle_sh_command(
    cmd: &ShCommand,
    _installs: &Installations,
    catalog: &AssetCatalog,
) -> Result<()> {
    for input in &cmd.inputs {
        for info in catalog.search(Search::for_input("SH", input))? {
            let long_name = info.to_string();
            println!("{long_name}");
            println!("{}", "=".repeat(long_name.len()));
            show_sh(info.name(), &info.data()?, cmd)?;
        }
    }
    Ok(())
}

pub fn sh_to_yaml(path: &Path) -> Result<()> {
    let outpath = path.with_extension("SH.yaml");
    let data = fs::read(path)?;
    let shape = ShCode::from_bytes(&data, None)?;
    let yaml = shape.to_yaml()?;
    fs::write(outpath, yaml)?;
    Ok(())
}

pub fn yaml_to_sh(path: &Path) -> Result<()> {
    let yaml = fs::read_to_string(path)?;
    let code = ShCode::from_yaml(&yaml)?;
    let pe_data = code.compile_pe()?;
    let outpath = path.with_extension("");
    fs::write(outpath, pe_data)?;
    Ok(())
}

fn show_sh(_name: &str, data: &[u8], cmd: &ShCommand) -> Result<()> {
    if let Some(filename) = cmd.yaml.as_ref() {
        let shape = ShCode::from_bytes(data, None)?;
        let content = shape.to_yaml()?;
        if filename == "-" {
            println!("{content}");
        } else {
            let mut fp = File::create(filename)?;
            fp.write_all(content.as_bytes())?;
        }
    } else if cmd.show_extents {
        let shape = ShCode::from_bytes(data, None)?;
        if let Ok(analysis) = shape.analysis(DrawSelection::NormalModel) {
            println!("Full: {:#?}", analysis.extent().aabb_full());
            println!("Body: {:#?}", analysis.extent().aabb_body());
        }
        if let Ok(analysis) = shape.analysis(DrawSelection::DamageModel) {
            println!("Damage Full: {:#?}", analysis.extent().aabb_full());
        }
    } else if cmd.custom {
        println!("unused");
    } else {
        println!("Showing all instructions...");
        let shape = ShCode::from_bytes(data, None)?;
        println!("{}", shape.display_content()?);
    }
    Ok(())
}
