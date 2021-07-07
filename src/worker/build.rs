fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../server/proto/queue.proto")?;
    tonic_build::compile_protos("proto/reflection.proto")?;

    Ok(())
}