use criterion::{criterion_group, criterion_main, Criterion};
use frogcore::{node::BasicFlood, scenario::Scenario, simulation::run_simulation};
use std::{hint::black_box, time::Duration};

const DATA: &str = include_str!("sim_file.sim");

pub fn criterion_benchmark(c: &mut Criterion) {
    let this: Scenario = serde_json::from_str(DATA).unwrap();

    let mut group = c.benchmark_group("main");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("Full Simulation", |b| {
        b.iter(|| {
            black_box(run_simulation(
                123456,
                this.clone(),
                BasicFlood::new().into(),
                false,
            ));
        })
    });

    group.bench_function("Full Simulation with Logs", |b| {
        b.iter(|| {
            black_box(run_simulation(
                123456,
                this.clone(),
                BasicFlood::new().into(),
                true,
            ));
        })
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
