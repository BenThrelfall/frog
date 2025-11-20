use rmp_serde::{decode, encode};
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
};
use thiserror::Error;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    scenario::ScenarioIdentity,
    simulation::data_structs::{LogItem, Transmission},
};

#[derive(Debug, Error)]
pub enum SimFileError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
    #[error(transparent)]
    RMPWriteError(#[from] encode::Error),
    #[error(transparent)]
    RMPReadError(#[from] decode::Error),
}

pub fn load_output(path: PathBuf) -> Result<SimOutput, SimFileError> {
    use serde_json::error::Category;

    let file = File::open(&path)?;
    let buf_reader = BufReader::new(file);

    let json_result: Result<SimOutput, _> = serde_json::from_reader(buf_reader);

    json_result.or_else(|err| match err.classify() {
        Category::Io | Category::Eof => Err(err.into()),
        _ => {
            let file = File::open(path)?;
            let buf_reader = BufReader::new(file);
            let res: Result<SimOutput, _> = decode::from_read(buf_reader);
            res.map_err(|x| x.into())
        }
    })
}

pub fn write_output(path: PathBuf, output: SimOutput, use_rmp: bool) -> Result<(), SimFileError> {
    let file = File::create(path)?;
    let mut buf = BufWriter::new(file);

    if use_rmp {
        encode::write(&mut buf, &output)?;
    } else {
        serde_json::to_writer_pretty(buf, &output)?;
    }

    Ok(())
}

pub fn load_file<T>(path: PathBuf) -> Result<T, SimFileError>
where
    T: DeserializeOwned,
{
    use serde_json::error::Category;

    let file = File::open(&path)?;
    let buf_reader = BufReader::new(file);

    let json_result: Result<T, _> = serde_json::from_reader(buf_reader);

    json_result.or_else(|err| match err.classify() {
        Category::Io | Category::Eof => Err(err.into()),
        _ => {
            let file = File::open(path)?;
            let buf_reader = BufReader::new(file);
            let res: Result<T, _> = decode::from_read(buf_reader);
            res.map_err(|x| x.into())
        }
    })
}

pub fn write_file<T>(path: PathBuf, object: T, use_rmp: bool) -> Result<(), SimFileError>
where
    T: Serialize,
{
    let file = File::create(path)?;
    let mut buf = BufWriter::new(file);

    if use_rmp {
        encode::write(&mut buf, &object)?;
    } else {
        serde_json::to_writer_pretty(buf, &object)?;
    }

    Ok(())
}

/// Contains enough information to completely recreate the simulation run it describes.
/// Unless a custom (i.e. hand created) scenario was used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputIdentity {
    pub scenario_identity: ScenarioIdentity,
    pub model_id: String,
    pub simulation_seed: u64,
    pub sim_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimOutput {
    // Data values
    pub logs: Vec<LogItem>,
    pub transmissions: Vec<Transmission>,

    // Regeneration
    pub complete_identity: OutputIdentity,
}
