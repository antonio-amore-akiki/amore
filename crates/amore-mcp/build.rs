// build.rs -- amore-mcp gRPC code generation (Phase H.6, ADR 0009).
//
// Resolves proto/amore.proto with a local-first search so the published
// crate tarball (which lives at target/package/amore-mcp-X.Y.Z/ without
// access to the workspace's ../../proto/) can find its bundled copy.
//
// Search order:
//   1. <CARGO_MANIFEST_DIR>/proto/amore.proto         — published crate path
//   2. <CARGO_MANIFEST_DIR>/../../proto/amore.proto   — workspace dev path
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let manifest = std::path::Path::new(&manifest_dir);

    let local = manifest.join("proto").join("amore.proto");
    let workspace = manifest.join("..").join("..").join("proto").join("amore.proto");

    let proto_path = if local.exists() {
        local
    } else if workspace.exists() {
        workspace
    } else {
        return Err(format!(
            "amore.proto not found at {} or {}",
            local.display(),
            workspace.display()
        )
        .into());
    };

    let proto_dir = proto_path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "proto dir not found")
    })?;

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[&proto_path], &[proto_dir])?;
    println!("cargo:rerun-if-changed={}", proto_path.display());
    Ok(())
}
