use std::path::Path;
use std::process::Command;

// trampoline-blob (aarch64-unknown-none, no_std) は workspace 全体の target には
// できないため、cargo の per-crate target 機能の欠如（rust-lang/cargo#7004）の
// 回避策として、build.rsからcargoを子プロセスとして明示的に呼び出す。
// target・linkerスクリプトの指定はtrampoline-blob/.cargo/config.tomlに記述している。

fn main() {
    // binbased-tracing/ はworkspaceルート直下のメンバークレートなので、
    // trampoline-blob/ は一つ上の階層（workspaceルート）にある。
    let cargo = std::env::var("CARGO").unwrap();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let blob_crate_dir = Path::new(&manifest_dir).join("../trampoline-blob");

    // rerun-if-changedを一切出力しない場合、cargoは自身のクレートのソースツリーのみを
    // 暗黙的に監視するため、別ディレクトリのtrampoline-blobの変更は検知できない。
    // 参考: https://github.com/rust-lang/cargo/issues/8091
    println!("cargo::rerun-if-changed={}", blob_crate_dir.display());

    let run = |args: &[&str]| {
        let status = Command::new(&cargo)
            .current_dir(&blob_crate_dir)
            // 親プロセスのCARGO_ENCODED_RUSTFLAGS（メインクレート用）を子に継承させると
            // trampoline-blob/.cargo/config.tomlのrustflagsより優先されてしまう。
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("RUSTFLAGS")
            .args(args)
            .status()
            .unwrap_or_else(|e| panic!("failed to spawn cargo {args:?}: {e}"));
        assert!(
            status.success(),
            "cargo {args:?} failed for trampoline-blob"
        );
    };

    run(&["build", "--release"]);

    let bin_path = Path::new(&out_dir).join("trampoline.bin");
    run(&[
        "objcopy",
        "--release",
        "--",
        "-O",
        "binary",
        "--only-section=.text",
        bin_path.to_str().unwrap(),
    ]);
}
