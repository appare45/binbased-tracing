use clap::Parser;

mod conf;
mod error;
mod proc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]

struct Args {
    #[arg(value_name = "PID")]
    pid: i32,
}

fn main() {
    let args = Args::parse();
    println!("Target PID: {}", args.pid);

    let c = conf::new(args.pid);
    let _ = c.trace().unwrap();
}
