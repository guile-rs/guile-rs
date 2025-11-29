/* Credit where credit's due:
 * This build.rs is derived from: https://github.com/ysimonson/guile-sys.
 * Thanks to Yusuf Simonson for his work on the build script we use.
 * Makes things much, much easier... */

extern crate bindgen;

use std::env;
use std::io::{self, Write, stdout};
use std::path::{self, Path, PathBuf};
use std::process::Command;
use std::str;

fn config_output(cmd: &str) -> Vec<u8> {
    Command::new("guile-config")
        .arg(cmd)
        .output()
        .expect("`guile-config` failed. Is guile installed?")
        .stdout
}

fn write_cargo_command<O>(command: &str, val: &[u8], out: &mut O) -> Result<(), io::Error>
where
    O: io::Write,
{
    write!(out, "cargo:{}=", command)
        .and_then(|_| out.write_all(val))
        .and_then(|_| out.write_all(b"\n"))
}

fn main() {

    #[cfg(feature = "dynamic")]
    println!("cargo:rustc-link-arg=-Wl,--export-dynamic");
    
    let mut stdout = stdout().lock();
    let linker_args = config_output("link");
    linker_args
        .split(u8::is_ascii_whitespace)
        .try_for_each(|arg| {
            if let Some(link_dir) = arg.strip_prefix(b"-L") {
                write_cargo_command("rustc-link-dir", link_dir, &mut stdout)
            } else if let Some(link_lib) = arg.strip_prefix(b"-l") {
                write_cargo_command("rustc-link-lib", link_lib, &mut stdout)
            } else {
                write_cargo_command("rustc-link-arg", arg, &mut stdout)
            }
        })
        /* my addition: Mabe build.rs rebuild on change */
        .and_then(|_| {
            stdout.write_all(
                b"cargo:rerun-if-changed=build.rs
cargo:rerun-if-changed=Cargo.lock\n",
            )
        })
        .and_then(|_| stdout.flush())
        .unwrap();

    let mut libguile = None;
    config_output("compile")
        .split(u8::is_ascii_whitespace)
        .flat_map(str::from_utf8)
        .fold(bindgen::Builder::default(), |bindgen, arg| {
            if let Some(mut include_dir) = arg.strip_prefix("-I").map(String::from) {
                if !include_dir.ends_with(path::MAIN_SEPARATOR) {
                    include_dir.push(path::MAIN_SEPARATOR);
                }
                include_dir.push_str("libguile.h");

                if Path::new(&include_dir).is_file() {
                    libguile = Some(include_dir);
                }
            }

            bindgen.clang_arg(arg)
        })
        .header(libguile.expect("failed to find `libguile.h`"))
        .generate()
        .expect("failed to generate bindings")
        .write_to_file(
            env::var_os("OUT_DIR")
                .map(PathBuf::from)
                .unwrap()
                .join("libguile.rs"),
        )
        .unwrap();
}
