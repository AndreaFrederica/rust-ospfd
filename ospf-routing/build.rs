fn main() {
    println!("cargo:rerun-if-changed=src/routing.c");
    cc::Build::new().file("src/routing.c").compile("routing");
}
