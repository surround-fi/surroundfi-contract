use anyhow::Result;
use clap::Parser;
use surroundfi_cli::Opts;

fn main() -> Result<()> {
    surroundfi_cli::entry(Opts::parse())
}