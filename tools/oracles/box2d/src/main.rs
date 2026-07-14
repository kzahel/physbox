use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use physbox_box2d_oracle::{AdapterError, OracleLane, execute_request, unsupported_result};
use physbox_contracts::{ORACLE_SUCCESS_SENTINEL, OracleRequestV1, Validate, canonical_json};

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-box2d-oracle error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let request_bytes = fs::read(&arguments.request).map_err(|error| error.to_string())?;
    let request: OracleRequestV1 =
        serde_json::from_slice(&request_bytes).map_err(|error| error.to_string())?;
    request.validate().map_err(|error| error.to_string())?;
    let result = match execute_request(&request, &request_bytes, arguments.lane) {
        Ok(result) => result,
        Err(AdapterError::Unsupported(reason)) => {
            unsupported_result(&request, &request_bytes, arguments.lane, reason)
        }
        Err(error) => return Err(error.to_string()),
    };
    let result_bytes = canonical_json(&result).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments.result)
        .map_err(|error| error.to_string())?;
    file.write_all(&result_bytes)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    println!("{ORACLE_SUCCESS_SENTINEL}");
    Ok(())
}

struct Arguments {
    lane: OracleLane,
    request: PathBuf,
    result: PathBuf,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut lane = None;
        let mut request = None;
        let mut result = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--lane" => {
                    lane = Some(OracleLane::parse(&value).map_err(|error| error.to_string())?)
                }
                "--request" => request = Some(PathBuf::from(value)),
                "--result" => result = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            lane: lane.ok_or_else(|| "missing --lane world|narrowphase".to_owned())?,
            request: request.ok_or_else(|| "missing --request".to_owned())?,
            result: result.ok_or_else(|| "missing --result".to_owned())?,
        })
    }
}
