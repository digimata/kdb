// ---------------
// build.rs
//
// fn main()    L7
// ---------------

fn main() {
    let target = std::env::var("TARGET").unwrap();
    println!("cargo:rustc-env=TARGET={target}");
}
