use anyhow::Result;

use ea_skill_eval::load_traces;

fn main() -> Result<()> {
    let traces = load_traces("samples/traces.json")?;

    println!("Loaded {} trace(s)", traces.len());

    Ok(())
}
