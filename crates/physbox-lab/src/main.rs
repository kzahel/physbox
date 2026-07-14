use physbox_contracts::{Validate, canonical_json};
use physbox_engine::{authored_chain_scenario, authored_one_contact_scenario};

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-lab error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let scenario = match arguments.as_slice() {
        [command, name, seed] if command == "scenario" && name == "one-contact" => {
            authored_one_contact_scenario(parse(seed, "seed")?)
        }
        [command, name, count, seed] if command == "scenario" && name == "chain" => {
            authored_chain_scenario(parse(count, "body count")?, parse(seed, "seed")?)
                .map_err(|error| error.to_string())?
        }
        _ => {
            return Err(
                "usage: physbox-lab scenario one-contact SEED | scenario chain COUNT SEED"
                    .to_owned(),
            );
        }
    };
    scenario.validate().map_err(|error| error.to_string())?;
    let bytes = canonical_json(&scenario).map_err(|error| error.to_string())?;
    println!("{}", String::from_utf8(bytes).unwrap());
    Ok(())
}

fn parse<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {name}: {value}"))
}
