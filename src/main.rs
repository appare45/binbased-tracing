use std::io::Read;

use clap::Parser;

mod conf;
mod elf;
mod error;
mod maps;
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
    let mut t = c.trace().unwrap();
    let mut buf = Vec::new();
    t.get_bin().read_to_end(&mut buf).unwrap();
    let _elf = elf::new(&buf).unwrap();
}
