fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let protoc_include = protoc_bin_vendored::include_path()?;
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptor_path = out_dir.join("proph_descriptor.bin");

    let proto_files = [
        "../../packages/proto/v1/common.proto",
        "../../packages/proto/v1/errors.proto",
        "../../packages/proto/v1/engine.proto",
    ];
    let includes = ["../../packages/proto"];

    for proto in proto_files {
        println!("cargo:rerun-if-changed={proto}");
    }

    let status = std::process::Command::new(protoc)
        .arg(format!(
            "--descriptor_set_out={}",
            descriptor_path.to_string_lossy()
        ))
        .arg("--include_imports")
        .arg(format!("-I{}", includes[0]))
        .arg(format!("-I{}", protoc_include.to_string_lossy()))
        .args(proto_files)
        .status()?;

    if !status.success() {
        return Err(format!("protoc failed with status {status}").into());
    }

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .extern_path(".google.protobuf.Struct", "::prost_types::Struct")
        .extern_path(".google.protobuf.Timestamp", "::prost_types::Timestamp")
        .file_descriptor_set_path(&descriptor_path)
        .skip_protoc_run()
        .compile(&proto_files, &includes)?;

    Ok(())
}
