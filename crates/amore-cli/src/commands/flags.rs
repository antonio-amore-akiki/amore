use amore_core::flags::{Flags, compile_time_features};
use clap::Args;
use serde_json::json;
use std::collections::HashMap;

#[derive(Args, Debug)]
pub struct FlagsArgs {
    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: FlagsArgs) -> anyhow::Result<()> {
    let runtime = Flags::list();
    let compile_time = compile_time_features();
    if args.json {
        let ct_map: HashMap<String, bool> =
            compile_time.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let rt_map: HashMap<String, bool> = runtime.iter().cloned().collect();
        let obj = json!({
            "compile_time": ct_map,
            "runtime": rt_map,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        println!("# Compile-time features (Cargo features)");
        for (k, v) in &compile_time {
            println!("  {} = {}", k, if *v { "on" } else { "off" });
        }
        println!("\n# Runtime flags (AMORE_FLAG_* env / $AMORE_FLAGS_FILE)");
        if runtime.is_empty() {
            println!("  (none set)");
        }
        for (k, v) in &runtime {
            println!("  {} = {}", k, if *v { "on" } else { "off" });
        }
    }
    Ok(())
}
