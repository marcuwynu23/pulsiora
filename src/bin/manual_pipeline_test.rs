use pulsiora_parser::parse_pulsefile;
use pulsiora_runner::PipelineExecutor;
use pulsiora_core::{GitEvent, GitEventType, Repository};
use std::fs;

fn main() {
    println!("🚀 Manual CI/CD Pipeline Test");
    println!("================================");
    
    // 1. Read and parse the Pulsefile
    println!("\n📋 Step 1: Parsing Pulsefile...");
    let pulsefile_content = fs::read_to_string("../test-repo/Pulsefile")
        .expect("Failed to read Pulsefile");
    
    match parse_pulsefile(&pulsefile_content) {
        Ok(pipeline) => {
            println!("✅ Pulsefile parsed successfully!");
            println!("   Pipeline: {} v{}", pipeline.name, pipeline.version);
            println!("   Steps: {}", pipeline.steps.len());
            println!("   Git triggers - push: {}, branches: {:?}", 
                pipeline.triggers.git.on_push, 
                pipeline.triggers.git.branches);
            
            // 2. Simulate git push event
            println!("\n🎯 Step 2: Simulating Git Push Event...");
            let git_event = GitEvent {
                event_type: GitEventType::Push,
                repository: Repository {
                    full_name: "test-owner/test-repo".to_string(),
                    default_branch: "main".to_string(),
                    url: "https://github.com/test-owner/test-repo".to_string(),
                },
                branch: "main".to_string(),
                commit: "abc123".to_string(),
            };
            
            println!("✅ Git event created for branch: {}", git_event.branch);
            
            // 3. Check if pipeline should trigger
            println!("\n🔍 Step 3: Checking if pipeline should trigger...");
            if pipeline.should_trigger(&git_event) {
                println!("✅ Pipeline should trigger on this event!");
                
                // 4. Execute the pipeline
                println!("\n⚡ Step 4: Executing Pipeline Steps...");
                let executor = PipelineExecutor::new();
                
                for (i, step) in pipeline.steps.iter().enumerate() {
                    println!("\n   Step {}: {}", i + 1, step.name);
                    println!("   Command: {}", step.run);
                    println!("   Allow failure: {}", step.allow_failure);
                    
                    // In a real scenario, this would execute the actual commands
                    println!("   ✅ Would execute: {}", step.run);
                }
                
                println!("\n🎉 CI/CD Pipeline would execute successfully!");
                println!("\nThe pipeline includes:");
                println!("   - Environment setup");
                println!("   - Application build");
                println!("   - Test execution");
                println!("   - Deployment preparation");
                println!("   - Success notification");
                
            } else {
                println!("❌ Pipeline would not trigger for this event");
            }
        }
        Err(e) => {
            println!("❌ Failed to parse Pulsefile: {}", e);
        }
    }
}