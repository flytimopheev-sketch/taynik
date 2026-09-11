fn main() {
    // Каталог syslibs (создаётся в spec-файле RPM из системных GTK-библиотек,
    // без libc — libc предоставляет zig c целевым glibc 2.17).
    println!("cargo:rustc-link-search=native=syslibs");
    println!("cargo:rustc-link-arg=-Wl,--allow-shlib-undefined");
}
