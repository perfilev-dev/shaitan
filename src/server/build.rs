use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/helloworld.proto")?;

    let descriptor_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("helloworld_descriptor.bin");
    tonic_build::configure()
        .file_descriptor_set_path(&descriptor_path)
        .format(true)
        .compile(&["proto/helloworld.proto"], &["proto/"])?;

    Ok(())
}