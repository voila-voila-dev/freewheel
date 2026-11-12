//! `hot-paths <benchmark> <packets>`: runs one benchmark, prints its checksum.
//! `hot-paths --list`: prints benchmark names. Measured from outside by `scripts/bench`.

use benchmarks::ALL;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--list" => {
            for benchmark in ALL {
                println!("{}", benchmark.name);
            }
            ExitCode::SUCCESS
        }
        [name, packets] => {
            let (Some(benchmark), Ok(packets)) =
                (ALL.iter().find(|b| b.name == name), packets.parse())
            else {
                eprintln!("unknown benchmark or bad packet count: {name} {packets}");
                return ExitCode::FAILURE;
            };
            println!("{}", (benchmark.run)(packets));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: hot-paths --list | hot-paths <benchmark> <packets>");
            ExitCode::FAILURE
        }
    }
}
