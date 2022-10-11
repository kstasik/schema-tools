use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
};
use walkdir::{DirEntry, WalkDir};

use digest::{Digest, Output};

use crate::error::Error;

pub fn calculate<D: Digest>(path: &Path) -> Result<Output<D>, Error> {
    let metadata = fs::metadata(path).map_err(Error::HashCalculationError)?;

    let mut hash = D::new();

    if metadata.is_file() {
        calculate_file(path, &mut hash)?;
    } else if metadata.is_dir() {
        calculate_dir(path, &mut hash)?;
    } else {
        return Err(Error::HashCalculationError(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid path type",
        )));
    }

    Ok(hash.finalize())
}

fn calculate_file<D: Digest>(path: &Path, hash: &mut D) -> Result<(), Error> {
    let mut file = File::open(path).map_err(Error::HashCalculationError)?;

    let metadata = fs::metadata(path).map_err(Error::HashCalculationError)?;

    log::debug!("calculating: {:?}", path);

    let mut buffer = vec![0; metadata.len() as usize];
    file.read(&mut buffer)
        .map_err(Error::HashCalculationError)?;

    hash.update(buffer);

    Ok(())
}

fn calculate_dir<D: Digest>(path: &Path, hash: &mut D) -> Result<(), Error> {
    let list = WalkDir::new(path).sort_by(|a, b| a.path().cmp(b.path()));

    for entry in list.into_iter().filter_entry(|e| !is_hidden(e, path)) {
        let entry = entry.map_err(Error::HashCalculationDirError)?;
        let path = entry.path();
        if path.is_file() {
            calculate_file(path, hash)?;
        }
    }

    Ok(())
}

fn is_hidden(entry: &DirEntry, path: &Path) -> bool {
    if entry
        .path()
        .strip_prefix(path)
        .map(|p| p.starts_with("tests"))
        .unwrap_or(false)
    {
        return true;
    }

    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}
