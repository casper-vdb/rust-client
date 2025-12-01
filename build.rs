fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use a vendored `protoc` binary so users don't need it installed system-wide.
    let protoc_path = protoc_bin_vendored::protoc_bin_path()?;

    // `set_var` is unsafe in this toolchain, wrap it explicitly.
    unsafe {
        std::env::set_var("PROTOC", protoc_path);
    }

    tonic_build::configure()
        .build_server(false)
        .compile_protos(
            &["proto/matrix_service.proto"],
            &["proto"],
        )?;

    Ok(())
}

