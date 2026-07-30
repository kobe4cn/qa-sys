use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir: PathBuf = "../../proto".into();
    let mut file_list: Vec<PathBuf> = Vec::new();
    let lists = proto_dir.read_dir()?;
    for entry in lists.flatten() {
        if entry.path().is_file() {
            file_list.push(entry.path());
        }
    }
    let out_dir = Path::new("../pb/src");
    let descriptor_file = Path::new("../qa-svc").join("rpc_descriptor.bin");
    tonic_prost_build::configure()
        .out_dir(out_dir)
        .file_descriptor_set_path(&descriptor_file)
        .build_client(true)
        .build_server(true)
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&file_list, &[proto_dir])?;
    Ok(())
}
