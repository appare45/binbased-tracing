use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(value_name = "PID")]
    pid: i32,
}

fn main() {
    let args = Args::parse();
    println!("Target PID: {}", args.pid);
}
