use std::{
    fs::{read_dir, File},
    io::{self, Write},
    path::PathBuf,
};

use clap::{arg, command, Parser};
use frogcore::{
    analysis::{CompleteAnalysis, EmergencyResult},
    node::{parse_model, MODEL_LIST},
    scenario::{ScenarioIdentity, Scenario},
    sim_file::{load_file, load_output, SimOutput},
    simulation::run_simulation,
    units::{Unit, SECONDS},
};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// A file containing a list of scenarios that will be run.
    /// This overrides `results`.
    #[arg(long)]
    pack: Option<PathBuf>,

    /// Models to use if running a simpack `--pack`.
    /// Defaults to all models.
    #[arg(long)]
    models: Option<Vec<String>>,

    #[arg(long)]
    range_start: Option<usize>,

    #[arg(long)]
    range_end: Option<usize>,

    #[arg(long)]
    no_verify: bool,

    /// Results file or directory containing results files
    #[arg(short, long)]
    results: Option<PathBuf>,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    let maybe_pack_path = args.pack;
    let no_verify = args.no_verify;

    let results_path = args.results.unwrap_or("sim_output.json".into());
    let verbose = args.verbose;

    let model_list = match (args.range_start, args.range_end) {
        (None, None) => args
            .models
            .map(|list| {
                list.iter()
                    .map(|inner| parse_model(inner).unwrap())
                    .collect()
            })
            .unwrap_or_else(|| MODEL_LIST.to_vec()),
        (None, Some(end)) => MODEL_LIST.into_iter().take(end).collect(),
        (Some(start), None) => MODEL_LIST.into_iter().skip(start).collect(),
        (Some(start), Some(end)) => MODEL_LIST.into_iter().skip(start).take(end).collect(),
    };

    if let Some(pack_path) = maybe_pack_path {
        let mut model_tables: Vec<(_, Vec<TableEntry>)> = model_list
            .into_iter()
            .map(|model| (model, Vec::new()))
            .collect();

        let scenarios = load_file::<Vec<ScenarioIdentity>>(pack_path).unwrap();

        for (model, inner_table) in model_tables.iter_mut() {
            scenarios
                .clone()
                .into_par_iter()
                .map(|x| {
                    let results = run_simulation(123456, x.create(), (*model).into(), false);
                    make_table_entry(no_verify, verbose, results)
                })
                .collect_into_vec(inner_table);

            eprintln!("Finished {model:?}");
            let out_path = args
                .output
                .as_ref()
                .map(|x| x.join(format!("{model:?}.csv")));
            write_table(out_path, inner_table);
        }
    } else {
        let mut table = Vec::new();
        let simulations = load_result_files(results_path);

        simulations
            .into_iter()
            .for_each(|x| table.push(make_table_entry(no_verify, verbose, x)));

        write_table(args.output, &table);
    };
}

fn write_table(maybe_path: Option<PathBuf>, table: &Vec<TableEntry>) {
    let write = if let Some(out_path) = maybe_path {
        let file = File::create(out_path).unwrap();
        Box::new(file) as Box<dyn Write>
    } else {
        Box::new(io::stdout())
    };

    let mut writer = csv::Writer::from_writer(write);
    table.into_iter().for_each(|x| {
        writer.serialize(x).unwrap();
    });
    writer.flush().unwrap();
}

fn make_table_entry(no_verify: bool, verbose: bool, results: SimOutput) -> TableEntry {
    let frogcore::sim_file::OutputIdentity {
        scenario_identity: scenario,
        model_id,
        simulation_seed: random_seed,
        sim_version,
    } = &results.complete_identity;

    let scenario_file = scenario.create();

    let analysis = CompleteAnalysis::new(results.clone(), scenario_file.clone());

    let first_message = scenario_file
        .messages
        .iter()
        .map(|x| x.generate_time)
        .min_by(|x, y| x.partial_cmp(&y).unwrap());

    let last_message = scenario_file
        .messages
        .iter()
        .map(|x| x.generate_time)
        .max_by(|x, y| x.partial_cmp(&y).unwrap());

    let messaging_time =
        last_message.unwrap_or(0.0 * SECONDS) - first_message.unwrap_or(0.0 * SECONDS);
    let messaging_time = messaging_time.seconds();

    let pathloss_param = {
        use frogcore::simulation::models::PairWiseCaptureEffect;
        use frogcore::simulation::models::PathlossModel::*;
        use frogcore::simulation::models::TransmissionModel::*;

        let path_loss = match scenario_file.model {
            PairWiseNormal(PairWiseCaptureEffect { ref path_loss, .. })
            | PairWiseNone(PairWiseCaptureEffect { ref path_loss, .. })
            | PairWiseUniform(PairWiseCaptureEffect { ref path_loss, .. }) => path_loss,
        };

        match path_loss {
            NoPathloss(_) => "None".to_owned(),
            AdjustedFreeSpacePathLoss(free_space_path_loss, ..) => {
                format!("Log {:.6}", free_space_path_loss.distance_exponent)
            }
            LinearPathLoss(linear_path_loss) => {
                format!("Linear {:.6}", linear_path_loss.loss_rate.inner())
            }
        }
    };

    let entry = TableEntry {
        scenario_identity: serde_json::to_string(scenario).unwrap(),
        model_identity: model_id.clone(),
        sim_version: sim_version.clone(),
        seed: *random_seed,
        avg_reception: analysis.reception_analysis.average_reception_rate,
        min_reception: analysis.reception_analysis.min_reception_rate,
        max_reception: analysis.reception_analysis.max_reception_rate,
        total_transmissions: analysis.transmissions.len(),
        total_airtime: analysis.total_airtime,
        end_time: analysis.end_time,
        avg_avg_latency: analysis.reception_analysis.avg_avg_latency.seconds(),
        min_avg_latency: analysis.reception_analysis.min_avg_latency.seconds(),
        max_avg_latency: analysis.reception_analysis.max_avg_latency.seconds(),
        generated_messages: scenario_file.messages.len(),
        messaging_time,
        pathloss_param,
        l120_score: analysis.reception_analysis.l120_score.seconds(),
        l600_score: analysis.reception_analysis.l600_score.seconds(),
        l6000_score: analysis.reception_analysis.l6000_score.seconds(),
        all_packet_uniqueness: analysis.reception_analysis.all_packet_uniqueness,
        message_packet_uniqueness: analysis.reception_analysis.message_packet_uniqueness,
        phantom_uniqueness: analysis.reception_analysis.phantom_uniqueness,
        message_reception_directness: analysis.reception_analysis.message_reception_directness,
        reception_directness: analysis.reception_analysis.reception_directness,
        message_reception_unique_directness: analysis
            .reception_analysis
            .message_reception_unique_directness,
        reception_unique_directness: analysis.reception_analysis.reception_unique_directness,
        message_transmission_directness: analysis
            .reception_analysis
            .message_transmission_directness,
        transmission_directness: analysis.reception_analysis.transmission_directness,
        message_transmission_unique_directness: analysis
            .reception_analysis
            .message_transmission_unique_directness,
        transmission_unique_directness: analysis.reception_analysis.transmission_unique_directness,
        emergency_result: analysis.reception_analysis.emergency_result,
        transmission_sent_events: analysis.transmission_sent_events,
        transmission_received_events: analysis.transmission_received_events,
        transmission_blocked_events: analysis.transmission_blocked_events,
        global_latency: analysis.reception_analysis.global_latency.seconds(),
        global_reception_rate: analysis.reception_analysis.global_reception_rate,
        t120_reception: analysis.reception_analysis.t120_reception,
        t600_reception: analysis.reception_analysis.t600_reception,
        t1800_reception: analysis.reception_analysis.t1800_reception,
        t6000_reception: analysis.reception_analysis.t6000_reception,
        gateway_latency: analysis.reception_analysis.gateway_latency.seconds(),
        gateway_reception: analysis.reception_analysis.gateway_reception,
    };

    if verbose {
        printout(scenario_file.clone(), results);
    }

    if !no_verify && !frogcore::verification::verify_all(&analysis) {
        eprintln!(
            "<Error> Verification failed for {:#?}",
            analysis.complete_identity
        );
    }

    entry
}

fn load_result_files(results_path: PathBuf) -> Vec<SimOutput> {
    let mut sim_results: Vec<SimOutput> = Vec::new();

    if results_path.is_file() {
        match load_output(results_path) {
            Ok(loaded) => sim_results.push(loaded),
            Err(e) => {
                eprintln!("<Error> {e}");
            }
        }
    } else {
        for thing in read_dir(results_path).unwrap() {
            let file = match thing {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("<Error> {e}");
                    continue;
                }
            };

            match load_output(file.path()) {
                Ok(loaded) => {
                    sim_results.push(loaded);
                }
                Err(e) => {
                    eprintln!("<Warning> {e}");
                    continue;
                }
            };
        }
    }
    sim_results
}

#[derive(Debug, Clone, Serialize)]
struct TableEntry {
    scenario_identity: String,
    model_identity: String,
    sim_version: String,
    seed: u64,
    pathloss_param: String,
    generated_messages: usize,
    messaging_time: f64,
    avg_reception: f64,
    min_reception: f64,
    max_reception: f64,
    avg_avg_latency: f64,
    min_avg_latency: f64,
    max_avg_latency: f64,
    total_transmissions: usize,
    total_airtime: f64,
    end_time: f64,
    l120_score: f64,
    l600_score: f64,
    l6000_score: f64,
    all_packet_uniqueness: f64,
    message_packet_uniqueness: f64,
    phantom_uniqueness: f64,

    global_latency: f64,
    global_reception_rate: f64,

    t120_reception: f64,
    t600_reception: f64,
    t1800_reception: f64,
    t6000_reception: f64,

    message_reception_directness: f64,
    reception_directness: f64,

    message_reception_unique_directness: f64,
    reception_unique_directness: f64,

    message_transmission_directness: f64,
    transmission_directness: f64,

    message_transmission_unique_directness: f64,
    transmission_unique_directness: f64,

    emergency_result: EmergencyResult,

    transmission_sent_events: usize,
    transmission_received_events: usize,
    transmission_blocked_events: usize,

    gateway_latency: f64,
    gateway_reception: f64,
}

fn printout(scenario: Scenario, results: SimOutput) {
    let frogcore::sim_file::OutputIdentity {
        scenario_identity: scenario_id,
        model_id,
        simulation_seed: random_seed,
        sim_version,
    } = &results.complete_identity;

    let analysis = CompleteAnalysis::new(results.clone(), scenario.clone());

    println!();
    println!("{scenario_id:?} with node model {model_id}");
    println!("random seed: {random_seed}   simulation version: {sim_version}");
    println!(
        "Reception Rate: avg({:.4})  min({:.4})  max({:.4})",
        analysis.reception_analysis.average_reception_rate,
        analysis.reception_analysis.min_reception_rate,
        analysis.reception_analysis.max_reception_rate
    );
    println!(
        "Total Transmissions: {}  Total Airtime: {:.4} (Simulation End Time: {:.4})",
        analysis.transmissions.len(),
        analysis.total_airtime,
        analysis.end_time,
    );
}
