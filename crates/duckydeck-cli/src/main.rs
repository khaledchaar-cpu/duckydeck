//! `duckydeck` command line interface.

use anyhow::Result;

fn main() -> Result<()> {
    println!("duckydeck {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
