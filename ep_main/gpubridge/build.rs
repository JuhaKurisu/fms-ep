use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=JAVA_HOME");

    let jdk = find_jdk().expect(
        "JDK が見つかりません。JAVA_HOME を設定するか Processing をインストールしてください",
    );
    let libdir = jdk.join("lib");

    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=dylib=jawt");

    // 実行時に libjawt を解決できるようにする（JVM 内から読まれるため）。
    // lib/server は libjawt が依存する libjvm の場所。JVM 外で走る cargo test の
    // テストバイナリが起動できるようにするために必要。
    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", libdir.display());
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,{}",
            libdir.join("server").display()
        );
    }
}

fn find_jdk() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(home);
        if p.join("lib").exists() {
            return Some(p);
        }
    }
    // Processing 同梱の JDK
    for c in [
        "/Applications/Processing.app/Contents/app/resources/jdk",
        "C:/Program Files/Processing/app/resources/jdk",
    ] {
        let p = Path::new(c);
        if p.join("lib").exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}
