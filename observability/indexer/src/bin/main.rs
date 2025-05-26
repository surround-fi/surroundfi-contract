use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    surroundfi_indexer::entrypoint::entry(surroundfi_indexer::entrypoint::Opts::parse())
}
