//! Rust-to-Wasm tests helper library.

// Coding conventions
// #![deny(unsafe_code)]
#![deny(non_upper_case_globals)]
#![deny(non_camel_case_types)]
#![deny(non_snake_case)]
#![deny(unused_mut)]
// #![deny(dead_code)]
#![deny(unused_imports)]
// #![deny(missing_docs)]
// Clippy exclusions
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::wildcard_enum_match_arm)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
// #![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::panic)]

extern crate c2zk_rust_wasm_tests_add;
extern crate c2zk_rust_wasm_tests_fib;

#[allow(clippy::type_complexity)]
pub fn wrap_main_with_io(
    main_func: &'static dyn Fn(),
) -> Box<dyn Fn(Vec<u64>, Vec<u64>) -> Vec<u64>> {
    Box::new(|input: Vec<u64>, secret_input: Vec<u64>| {
        c2zk_stdlib::io_native::init_io(input, secret_input);
        main_func();
        c2zk_stdlib::io_native::get_pub_output()
    })
}

#[allow(clippy::unwrap_used)]
pub fn compile_rust_wasm_tests(bundle_name: &str, bin_name: &str) -> Vec<u8> {
    // TODO: make it relative to this crate (not the one it is called from)
    let manifest_path = format!("../rust-wasm-tests/{}/Cargo.toml", bundle_name);
    // let pwd = std::process::Command::new("pwd").output().unwrap();
    // dbg!(&pwd);
    let target_dir = format!("/tmp/c2zk-rust-wasm-tests/{}", bundle_name);
    let comp_status = std::process::Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--release")
        // .arg("--bin")
        // .arg(bin_name)
        .arg("--bins")
        .arg("--target=wasm32-unknown-unknown")
        .arg("--target-dir")
        .arg(target_dir.clone())
        .status()
        .unwrap();
    dbg!(&comp_status);
    assert!(comp_status.success());
    let target_bin_file_path = std::path::Path::new(&target_dir)
        .join("wasm32-unknown-unknown")
        .join("release")
        .join(bin_name)
        .with_extension("wasm");
    let mut target_bin_file = std::fs::File::open(target_bin_file_path).unwrap();
    let mut wasm_bytes = vec![];
    std::io::Read::read_to_end(&mut target_bin_file, &mut wasm_bytes).unwrap();
    wasm_bytes
}
