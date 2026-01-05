fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile protocol buffer definitions
    // Output goes to OUT_DIR by default, which is where tonic::include_proto! looks
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/broker.proto",
                "proto/market_data.proto",
                "proto/order.proto",
                "proto/portfolio.proto",
                "proto/strategy.proto",
            ],
            &["proto"],
        )?;

    // Re-run build if proto files change
    println!("cargo:rerun-if-changed=proto/broker.proto");
    println!("cargo:rerun-if-changed=proto/market_data.proto");
    println!("cargo:rerun-if-changed=proto/order.proto");
    println!("cargo:rerun-if-changed=proto/portfolio.proto");
    println!("cargo:rerun-if-changed=proto/strategy.proto");

    Ok(())
}
