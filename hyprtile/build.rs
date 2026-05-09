use std::env;
#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::process::Command;

fn main() {
    println!("cargo:rustc-env=VERSION={}", env!("CARGO_PKG_VERSION"));
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        compile_windows_resources(&out_dir);
    }
}

#[cfg(windows)]
fn compile_windows_resources(out_dir: &Path) {
    let rc_file = PathBuf::from("resources/hyprtile.rc");
    if !rc_file.exists() {
        println!(
            "cargo:warning=Resource file not found: {}",
            rc_file.display()
        );
        return;
    }

    println!("cargo:rerun-if-changed={}", rc_file.display());

    let target = env::var("TARGET").unwrap_or_default();
    let res_file = out_dir.join("hyprtile.res");

    if target.contains("gnu") {
        // MinGW / GNU toolchain — use windres
        let windres = env::var("WINDRES").unwrap_or_else(|_| "windres".to_string());

        let status = Command::new(&windres)
            .arg("-O")
            .arg("coff")
            .arg("-i")
            .arg(&rc_file)
            .arg("-o")
            .arg(&res_file)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:rustc-link-arg={}", res_file.display());
            }
            Ok(s) => {
                println!(
                    "cargo:warning=windres failed with exit code {:?}, skipping resource compilation",
                    s.code()
                );
            }
            Err(e) => {
                println!(
                    "cargo:warning=Failed to run windres ({}), skipping resource compilation",
                    e
                );
            }
        }
    } else {
        // MSVC toolchain — use rc.exe
        let rc = env::var("RC").unwrap_or_else(|_| "rc.exe".to_string());

        let status = Command::new(&rc)
            .arg("/fo")
            .arg(&res_file)
            .arg(&rc_file)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:rustc-link-arg={}", res_file.display());
            }
            Ok(s) => {
                println!(
                    "cargo:warning=rc.exe failed with exit code {:?}, skipping resource compilation",
                    s.code()
                );
            }
            Err(e) => {
                println!(
                    "cargo:warning=Failed to run rc.exe ({}), skipping resource compilation",
                    e
                );
            }
        }
    }
}
