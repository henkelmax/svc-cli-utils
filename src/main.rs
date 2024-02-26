mod port_checker;

use clap::{Parser};
use console::style;
use crate::Commands::PortChecker;
use crate::port_checker::port_command;

#[derive(Parser, Debug)]
#[clap(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser, Debug)]
enum Commands {
    #[clap(aliases = & ["test", "ping"])]
    PortChecker(PortArgs),
}

#[derive(Parser, Debug)]
struct PortArgs {
    url: String,
    #[arg(short, long)]
    attempts: Option<u32>,
}

fn main() {
    let opt = Cli::parse();

    if opt.command.is_none() {
        eprintln!("{}", style("No arguments provided").red());
        return;
    }

    match opt.command.unwrap() {
        PortChecker(args) => {
            port_command(args);
        }
    }
}