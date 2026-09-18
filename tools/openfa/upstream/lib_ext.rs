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

// Unpack lib files.
use anyhow::Result;
use catalog::{AssetCatalog, FileSystem, Search};
use clap::{Args, Subcommand};
use humansize::{format_size, BINARY};
use installations::Installations;
use lib::{LibProvider, LibWriter};
use std::{
    fs::{create_dir_all, remove_file},
    path::{Path, PathBuf},
};

#[derive(Debug, Args)]
pub struct LibCommand {
    #[command(subcommand)]
    command: LibCommands,
}

/// Pack and Unpack Janes game LIB files
#[derive(Debug, Subcommand)]
enum LibCommands {
    /// List the contents of a lib
    #[command(name = "ls")]
    List {
        /// The lib files to list
        inputs: Vec<PathBuf>,
    },

    /// Unpack the given lib file(s)
    #[command()]
    Unpack {
        /// Output unpacked files into this directory
        #[arg(short = 'o', long = "output")]
        output_path: Option<PathBuf>,

        /// The lib files to unpack
        inputs: Vec<PathBuf>,
    },

    /// Pack the given files into a LIB
    #[command()]
    Pack {
        /// Output unpacked files into this directory
        #[arg(short = 'o', long = "output")]
        output_lib: PathBuf,

        /// The files to pack
        inputs: Vec<PathBuf>,
    },
}

pub fn handle_lib_command(
    cmd: &LibCommand,
    _installs: &Installations,
    _catalog: &AssetCatalog,
) -> Result<()> {
    match &cmd.command {
        LibCommands::List { inputs } => handle_ls(inputs),
        LibCommands::Unpack {
            inputs,
            output_path,
        } => unpack_lib(inputs, output_path),
        LibCommands::Pack { output_lib, inputs } => pack_lib(
            &inputs.iter().map(|v| v.as_path()).collect::<Vec<_>>(),
            output_lib,
        ),
    }
}

fn handle_ls(inputs: &[PathBuf]) -> Result<()> {
    let multi_input = inputs.len() > 1;
    for (i, input) in inputs.iter().enumerate() {
        let mut catalog = AssetCatalog::default();
        catalog.add_collection("ofa-tools", 0)?;
        catalog
            .collection_mut("ofa-tools")?
            .add_provider("lib-ls", 0, LibProvider::new(input)?)?;
        if multi_input {
            if i != 0 {
                println!();
            }
            println!("{}:", input.to_string_lossy());
        }
        for info in catalog.search(Search::for_glob("*"))? {
            let meta = info.stat()?;
            let mut psize = format_size(meta.compressed_size(), BINARY);
            if psize.ends_with(" B") {
                psize += "  ";
            }
            let mut asize = format_size(meta.size(), BINARY);
            if asize.ends_with(" B") {
                asize += "  ";
            }
            let ratio = if meta.compressed_size() == meta.size() && meta.size() > 0 {
                "~".to_owned()
            } else {
                format!(
                    "{:0.3}x",
                    meta.compressed_size() as f64 / meta.size() as f64
                )
            };

            println!(
                "{:15} {:<8} {:>12} {:>12}  {}",
                info.name(),
                meta.compression_type(),
                psize,
                asize,
                ratio
            );
        }
    }

    Ok(())
}

pub fn unpack_lib(inputs: &[PathBuf], output_path: &Option<PathBuf>) -> Result<()> {
    for input in inputs {
        let outdir = if let Some(p) = &output_path {
            p.to_owned()
        } else {
            let mut parent = if let Some(p) = input.parent() {
                p.to_owned()
            } else {
                PathBuf::from(".")
            };
            let name = input
                .file_name()
                .expect("no filename in input")
                .to_string_lossy();
            parent.push(&name[..name.len() - 4]);
            parent
        };
        let mut catalog = AssetCatalog::default();
        catalog.add_collection("ofa-tools", 0)?;
        catalog
            .collection_mut("ofa-tools")?
            .add_provider("lib-ls", 0, LibProvider::new(input)?)?;
        if !outdir.exists() {
            create_dir_all(&outdir)?;
        }
        for info in catalog.search(Search::for_glob("*"))? {
            let name = info.name();
            let outfilename = outdir.join(name);
            println!(
                "{}:{} -> {}",
                input.to_string_lossy(),
                name,
                outfilename.to_string_lossy()
            );
            let content = info.data()?;
            if outfilename.exists() {
                remove_file(&outfilename)?;
            }
            std::fs::write(outfilename, content)?;
        }
    }

    Ok(())
}

/// Takes a list of PathBuf to compress into a new lib output.
pub fn pack_lib(inputs: &[&Path], output_path: &Path) -> Result<()> {
    let mut writer = LibWriter::new(output_path, u16::try_from(inputs.len())?)?;
    for input in inputs {
        println!("Adding {:?}...", input);
        writer.add_file(input)?;
    }
    writer.finish()?;
    Ok(())
}
