use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=resources/resources.gresource.xml");
    println!("cargo:rerun-if-changed=resources/styles/shadhin.css");
    println!("cargo:rerun-if-changed=resources/icons/shadhin-logo.svg");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let target = format!("{out_dir}/shadhin.gresource");

    let status = Command::new("glib-compile-resources")
        .args([
            "--sourcedir=resources",
            &format!("--target={target}"),
            "resources/resources.gresource.xml",
        ])
        .status()
        .expect("Failed to execute glib-compile-resources");

    assert!(
        status.success(),
        "glib-compile-resources failed with non-zero exit status"
    );
}
