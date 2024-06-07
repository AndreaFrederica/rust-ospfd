use std::process::Command;

fn main() {
    rsc_compile("ospfd/handler/ack");
    rsc_compile("ospfd/handler/dd");
    rsc_compile("ospfd/handler/hello");
    rsc_compile("ospfd/handler/lsr");
    rsc_compile("ospfd/handler/lsu");
}

fn rsc_compile(path: &str) {
    println!("cargo::rerun-if-changed={}.rsc", path);
    let rsc = &format!("{}.rsc", path);
    let c = &format!("{}.c", path);
    let rs = &format!("{}.rs", path);
    // Copy the file
    Command::new("cp").arg(rsc).arg(c).status().unwrap();
    // Replace all '@' with '#' in the file
    Command::new("sed")
        .arg("-i")
        .arg(r#"s/\/\/ @/#/g"#)
        .arg(c)
        .status()
        .unwrap();
    // Compile with gcc -E
    Command::new("gcc")
        .arg("-E")
        .arg("-P")
        .arg(c)
        .arg("-o")
        .arg(rs)
        .status()
        .unwrap();
    // Remove the temporary file
    Command::new("rm").arg(c).status().unwrap();
}
