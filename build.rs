use std::fs;
use std::path::Path;

fn main() {
    let out_dir = "src/proto_gen";

    // Remove old output
    if Path::new(out_dir).exists() {
        fs::remove_dir_all(out_dir).expect("Failed to remove old proto output");
    }

    // Create output dir
    fs::create_dir_all(out_dir).expect("Failed to create proto output dir");

    // Compile proto
    tonic_build::configure()
        .out_dir(out_dir)
        .build_client(true)
        .build_server(false)
        .compile_protos(&["proto/chat.proto"], &["proto"])
        .expect("Failed to compile proto");
}