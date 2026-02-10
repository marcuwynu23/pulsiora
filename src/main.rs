use pulsiora_parser::parse_pulsefile;
use std::fs;

fn main() {
    // Read the Pulsefile
    let pulsefile_content = fs::read_to_string("Pulsefile")
        .expect("Failed to read Pulsefile");
    
    // Parse the Pulsefile
    match parse_pulsefile(&pulsefile_content) {
        Ok(pipeline) => {
            println!("✅ Pulsefile parsed successfully!");
            println!("Pipeline name: {}", pipeline.name);
            println!("Pipeline version: {}", pipeline.version);
            println!("Number of steps: {}", pipeline.steps.len());
            
            // Print trigger information
            let git = &pipeline.triggers.git;
            println!("\n📋 Git Triggers:");
            println!("  • on_push: {}", git.on_push);
            println!("  • on_pull_request: {}", git.on_pull_request);
            println!("  • Branches: {:?}", git.branches);
            
            // Print step information
            println!("\n🚀 Steps:");
            for step in &pipeline.steps {
                println!("  • {} (allow_failure: {})", step.name, step.allow_failure);
                println!("    Command: {}", step.run.lines().next().unwrap_or(""));
            }
        }
        Err(e) => {
            println!("❌ Failed to parse Pulsefile: {}", e);
        }
    }
}