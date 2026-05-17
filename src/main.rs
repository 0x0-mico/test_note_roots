use anyhow::{Result, anyhow};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use std::{path::PathBuf, sync::Arc};

use miden_client::{
    DebugMode,
    assembly::CodeBuilder,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    note::NoteScript,
    rpc::{Endpoint, GrpcClient},
};

fn read_masm_file(path_steps: &[&str]) -> Result<String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from_iter(
        [manifest_dir, "masm"]
            .into_iter()
            .chain(path_steps.iter().copied()),
    );
    std::fs::read_to_string(&path)
        .map_err(|e| anyhow!("Error reading MASM file at path {path:?}: {e:?}"))
}

fn link_math(mut code_builder: CodeBuilder) -> Result<CodeBuilder> {
    let math_code = read_masm_file(&["lib", "math.masm"])?;
    code_builder.link_module("zoro_miden::lib::math", &math_code)?;
    Ok(code_builder)
}

fn link_storage_utils(code_builder: CodeBuilder) -> Result<CodeBuilder> {
    let mut code_builder = link_math(code_builder)?;
    let storage_utils_code = read_masm_file(&["lib", "storage_utils.masm"])?;
    code_builder.link_module("zoro_miden::lib::storage_utils", &storage_utils_code)?;
    Ok(code_builder)
}

fn link_asset_utils(mut code_builder: CodeBuilder) -> Result<CodeBuilder> {
    let asset_utils_code = read_masm_file(&["lib", "asset_utils.masm"])?;
    code_builder.link_module("zoro_miden::lib::asset_utils", &asset_utils_code)?;
    Ok(code_builder)
}

fn link_zoropool(code_builder: CodeBuilder) -> Result<CodeBuilder> {
    let code_builder = link_asset_utils(code_builder)?;
    let mut code_builder = link_storage_utils(code_builder)?;

    let pool_code = read_masm_file(&["accounts", "zoropool.masm"])?;
    code_builder.link_module("zoroswap::zoropool", &pool_code)?;
    Ok(code_builder)
}

fn link_all_libraries(code_builder: CodeBuilder) -> Result<CodeBuilder> {
    link_zoropool(code_builder)
}

fn get_note_script(code_builder: CodeBuilder, note_file_name: &str) -> Result<NoteScript> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let note_path = PathBuf::from_iter(&[manifest_dir, "masm", "notes", note_file_name]);
    let note_code = std::fs::read_to_string(&note_path)
        .map_err(|e| anyhow!("Error parsing note code at path {note_path:?}: {e:?}"))?;
    let code_builder = link_all_libraries(code_builder.clone())?;
    code_builder
        .compile_note_script(note_code)
        .map_err(|e| anyhow!("Failed to compile note script: {}", e))
}

fn print_note_details(note_script: NoteScript) {
    println!("NOTE ROOT: {}", note_script.root().to_hex());
    println!("\nNOTE DIGESTS:");
    for digest in note_script.mast().procedure_digests() {
        println!("{}", digest.to_hex());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_api = Arc::new(GrpcClient::new(&Endpoint::testnet(), 30000));
    let keystore = FilesystemKeyStore::new("keystore".into())?;
    let mut client = ClientBuilder::new()
        .rpc(rpc_api.clone())
        .authenticator(keystore.into())
        .in_debug_mode(DebugMode::Enabled)
        .filesystem_keystore("keystore")?
        .sqlite_store("store.sqlite3".into())
        .build()
        .await?;
    client.ensure_genesis_in_place().await?;
    client.sync_state().await?;

    let code_builder = client.code_builder();
    let note_script = get_note_script(code_builder, "DEPOSIT.masm")?;
    print_note_details(note_script);

    Ok(())
}
