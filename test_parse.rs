use pulsiora_parser::parse_pulsefile;

fn main() {
    let pulsefile_content = r#"
pipeline {
  name: "test-pipeline";
  version: "1.0";
  
  triggers {
    git {
      on_push: true;
      on_pull_request: true;
      branches: ["main", "feature/*"];
    }
  }
  
  steps {
    step "build" {
      run: """
        echo "Building..."
        cargo build
      """;
    }
    
    step "test" {
      run: """
        echo "Testing..."
        cargo test
      """;
    }
  }
}"#;

    match parse_pulsefile(pulsefile_content) {
        Ok(pipeline) => {
            println!("✅ Pulsefile parsed successfully!");
            println!("Name: {}", pipeline.name);
            println!("Version: {}", pipeline.version);
            println!("Steps: {}", pipeline.steps.len());
            println!("on_push: {}", pipeline.triggers.git.on_push);
            println!("Branches: {:?}", pipeline.triggers.git.branches);
        }
        Err(e) => {
            println!("❌ Parse error: {}", e);
        }
    }
}