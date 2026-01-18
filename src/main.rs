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

const TARGET_SYMBOL: &str = "net/http.serverHandler.ServeHTTP";

fn main() {
    let args = Args::parse();
    println!("Target PID: {}", args.pid);

    let c = conf::new(args.pid);
    let mut proc = c.trace().unwrap();
    let mut buf = Vec::new();
    proc.get_bin().read_to_end(&mut buf).unwrap();
    let elf = elf::new(&buf).unwrap();
    let exec_base = proc
        .get_maps()
        .find(|m| m.executable)
        .map(|m| m.address.0)
        .unwrap();
    elf.funcs
        .get(TARGET_SYMBOL)
        .unwrap()
        .get_real_address(exec_base);
}
