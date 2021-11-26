//! This crate parses a PDB file

use std::env;
use std::fs::File;
use std::io::{BufReader};
use std::path::{Path, PathBuf};

mod msf;
mod pdb;

#[derive(Debug)]
///Errors for the entire crate.
enum ReaderError {
    /// Cold not open supplied file.
    Open(PathBuf, std::io::Error),
    /// MSF file could not be parsed
    NotMsfFile(PathBuf, msf::Error),
    /// PDB file could not be parsed
    NotPDBFile(PathBuf, pdb::Error),
}

fn main() -> Result<(), ReaderError> {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        println!("Usage: pdb <file.pdb>");
        return Ok(());
    }
    println!("Reading file {:?}", args[1]);
    let file = &args[1];
    let mut reader = BufReader::new(
        File::open(file).map_err(|x| ReaderError::Open(Path::new(file).to_path_buf(), x))?,
    );
    let mut msf = msf::MSF::load(&mut reader)
        .map_err(|x| ReaderError::NotMsfFile(Path::new(file).to_path_buf(), x))?;

    let mut _pdb = pdb::PDB::pdb_stream(&mut reader, &mut msf)
        .map_err(|x| ReaderError::NotPDBFile(Path::new(file).to_path_buf(), x))?;

    Ok(())
}
