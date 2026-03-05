fn main() {
    let proto_file = "../../proto/agent.proto";

    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&[proto_file], &["../../proto"])
        .expect("failed to compile proto contracts");
}
