use anyhow::{Result, anyhow};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use std::{fs, path::PathBuf, sync::Arc};

use miden_client::{
    DebugMode,
    assembly::CodeBuilder,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    note::NoteScript,
    rpc::{Endpoint, GrpcClient},
};

const SERIALIZED_NOTE_FILE: &str = "note.serialized";

fn serialized_note_path() -> PathBuf {
    PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), SERIALIZED_NOTE_FILE])
}

fn try_load_note_script_from_serialized_file() -> Result<Option<NoteScript>> {
    let path = serialized_note_path();
    println!("Looking if serialized.note exists at {:?}.", path);
    if !path.is_file() {
        println!("IT does not.");
        return Ok(None);
    }
    let meta = fs::metadata(&path).map_err(|e| anyhow!("stat {:?}: {e:?}", path))?;
    if meta.len() == 0 {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|e| anyhow!("read {:?}: {e:?}", path))?;
    let script =
        NoteScript::from_bytes(&bytes).map_err(|e| anyhow!("deserialize {:?}: {e:?}", path))?;
    Ok(Some(script))
}

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

fn link_lib0(mut code_builder: CodeBuilder) -> Result<CodeBuilder> {
    let code = read_masm_file(&["lib", "lib0.masm"])?;
    code_builder.link_module("sandbox::lib0", &code)?;
    Ok(code_builder)
}

fn link_lib1(mut code_builder: CodeBuilder) -> Result<CodeBuilder> {
    let code = read_masm_file(&["lib", "lib1.masm"])?;
    code_builder.link_module("sandbox::lib1", &code)?;
    Ok(code_builder)
}

fn link_lib2(code_builder: CodeBuilder) -> Result<CodeBuilder> {
    let mut code_builder = link_lib0(code_builder)?;
    let code = read_masm_file(&["lib", "lib2.masm"])?;
    code_builder.link_module("sandbox::lib2", &code)?;
    Ok(code_builder)
}

fn link_acc0(code_builder: CodeBuilder) -> Result<CodeBuilder> {
    let mut code_builder = link_lib1(code_builder)?;
    code_builder = link_lib2(code_builder)?;
    let pool_code = read_masm_file(&["accounts", "acc0.masm"])?;
    code_builder.link_module("sandbox::acc0", &pool_code)?;
    Ok(code_builder)
}

fn link_all_libraries(code_builder: CodeBuilder) -> Result<CodeBuilder> {
    link_acc0(code_builder)
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

fn print_note_details(note_script: &NoteScript) {
    println!("\nNOTE ROOT: {}", note_script.root().to_hex());
    println!("NOTE DIGESTS:");
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
        .sqlite_store("store.sqlite3".into())
        .build()
        .await?;
    client.ensure_genesis_in_place().await?;
    client.sync_state().await?;

    if let Some(script) = try_load_note_script_from_serialized_file()? {
        println!("\n\n------------------\n\nSERIALIZED NOTE: web-sdk");
        print_note_details(&script);
    }

    println!("\n\n------------------\n\nNOTE BUILT WITH: miden-client");
    let code_builder = client.code_builder();
    let note_script = get_note_script(code_builder, "EXAMPLE_NOTE.masm")?;
    print_note_details(&note_script);
    println!("\n\n------------------\n\n");
    Ok(())
}
