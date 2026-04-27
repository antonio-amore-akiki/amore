// build.rs -- amore-mcp gRPC code generation (Phase H.6, ADR 0009).
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let proto_path = std::path::Path::new(&manifest_dir)
        .join("..").join("..").join("proto").join("amore.proto");
    let proto_dir = proto_path.parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "proto dir not found"))?;
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[&proto_path], &[proto_dir])?;
    println!("cargo:rerun-if-changed={}", proto_path.display());
    Ok(())
}