use pulsiora_parser::parse_pulsefile;
use std::fs;

fn main() {
    // Read our Pulsefile
    let pulsefile_content = fs::read_to_string("test-repo/Pulsefile")
        .expect("Failed to read Pulsefile");
    
    println!("Testing Pulsefile parsing...");
    
    match parse_pulsefile(&pulsefile_content) {
        Ok(pipeline) => {
            println!("✅ Pulsefile parsed successfully!");
            println!("Pipeline name: {}", pipeline.name);
            println!("Pipeline version: {}", pipeline.version);
            println!("Number of steps: {}", pipeline.steps.len());
            println!("Git triggers - on_push: {}", pipeline.triggers.git.on_push);
            println!("Git triggers - branches: {:?}", pipeline.triggers.git.branches);
        }
        Err(e) => {
            println!("❌ Failed to parse Pulsefile: {}", e);
        }
    }
}