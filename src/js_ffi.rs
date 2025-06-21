pub static EDITABLE_LEVEL: std::sync::Mutex<Option<vmf_forge::VmfFile>> =
    std::sync::Mutex::new(None);

#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    /// Send a message to the client.
    pub fn tfbe_ffi_alert(s: &str);
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn tfbe_ffi_load_file(file_contents: &str) {
    tfbe_ffi_alert(&format!("Loading file with {} bytes", file_contents.len()));

    let parsed_file: Result<vmf_forge::VmfFile, _> = vmf_forge::VmfFile::parse(file_contents);

    match parsed_file {
        Ok(parsed_file) => {
            *EDITABLE_LEVEL.lock().unwrap() = Some(parsed_file);
            tfbe_ffi_alert("Loaded file!");
        }
        Err(err) => {
            tfbe_ffi_alert(&format!("Failed to parse file: {err}"));
        }
    }
}
