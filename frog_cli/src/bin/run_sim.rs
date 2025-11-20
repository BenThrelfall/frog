//! The simulator cli.

use std::{
    fs::{create_dir_all, read_dir},
    path::PathBuf,
    process::ExitCode,
    sync::atomic::AtomicU64,
    time::Instant,
};

use clap::Parser;
use frogcore::{
    node::{parse_model, ModelSelection, MODEL_LIST},
    scenario::ScenarioIdentity,
    sim_file::{self, load_file},
    simulation::run_simulation,
};
use rand::{rng, Rng};
use rayon::prelude::*;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    quiet: bool,

    /// Scenario file or directory containing scenario files
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// File name for output or folder to put simulation results into
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Seed for the rng. A random seed will be used if not specified
    #[arg(long)]
    seed: Option<u64>,

    #[arg(long)]
    model: Option<Vec<String>>,

    /// Show timing information
    #[arg(long)]
    time: bool,

    /// Overrides `--model` option if set.
    /// Will run with all models.
    #[arg(short, long)]
    all_models: bool,

    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let do_timing = args.time;
    let input_path = args.input.unwrap_or("sim_file.sim".into());

    let use_rmp = !args.json;

    let output_path = args.output.unwrap_or_else(|| {
        if !input_path.is_dir() {
            "sim_output.json".into()
        } else {
            create_dir_all("outputs").unwrap();
            let count = read_dir("outputs").unwrap().count();
            let out_name = format!("outputs/{count}");
            create_dir_all(out_name.clone()).unwrap();
            out_name.into()
        }
    });

    let quiet = args.quiet;

    let model_list = if args.all_models {
        MODEL_LIST.to_vec()
    } else {
        args.model
            .map(|x| x.into_iter().map(|s| parse_model(&s).unwrap()).collect())
            .unwrap_or(vec![ModelSelection::Meshtastic])
    };

    if !input_path.is_dir() {
        let timer = do_timing.then(|| Instant::now());
        let sim_count = model_list.len();

        for model in model_list {
            let random_seed = args.seed.unwrap_or_else(|| rng().random());

            let sim_file = sim_file::load_file(input_path.clone())
                .unwrap_or_else(|_| load_file::<ScenarioIdentity>(input_path.clone()).unwrap().create());

            let output = run_simulation(random_seed, sim_file.clone(), model.into(), true);

            let final_path = match (sim_count == 1, output_path.is_dir()) {
                (true, true) => output_path.join(format!("{model:?}.sim")),
                (true, false) => output_path.clone(),
                (false, true) => output_path.join(format!("{model:?}.rmp")),
                (false, false) => {
                    eprintln!(
                        "<Error> Output path must be a directory when using multiple node models"
                    );
                    return ExitCode::FAILURE;
                }
            };

            sim_file::write_output(final_path, output, use_rmp).unwrap();
        }

        if let Some(timer) = timer {
            let final_time = timer.elapsed().as_secs_f32();
            println!(
                "Ran {} sims in {:.4}s ({} sims / s)",
                sim_count,
                final_time,
                sim_count as f32 / final_time
            )
        }
        return ExitCode::SUCCESS;
    }

    let timer = do_timing.then(|| Instant::now());
    let count = AtomicU64::new(0);

    model_list.into_par_iter().for_each(|model| {
        for thing in read_dir(input_path.clone()).unwrap() {
            let file = match thing {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("<Error> {e}");
                    continue;
                }
            };

            let sim_file = match sim_file::load_file(file.path()) {
                Ok(loaded) => loaded,
                Err(e) => {
                    eprintln!("<Warning> {e}");
                    continue;
                }
            };

            // I think a new seed per scenario is best for now.
            let random_seed = args.seed.unwrap_or_else(|| rng().random());

            let file_name = file.file_name().into_string().unwrap();
            if !quiet {
                println!("<Message> Running simulation for {file_name}");
            }

            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let output = run_simulation(random_seed, sim_file, model.into(), true);

            let out_name = format!("output_{model:?}_{file_name}");
            let mut out = output_path.clone();
            out.push(out_name);

            if !quiet {
                println!("<Message> Writing output to {out:?}");
            }

            sim_file::write_output(out, output, use_rmp).unwrap();
        }
    });

    if let Some(timer) = timer {
        let final_count = count.load(std::sync::atomic::Ordering::Relaxed);
        let final_time = timer.elapsed().as_secs_f32();
        println!(
            "Ran {final_count} sims in {:.4}s ({} sims / s)",
            final_time,
            final_count as f32 / final_time
        )
    }

    return ExitCode::SUCCESS;
}
