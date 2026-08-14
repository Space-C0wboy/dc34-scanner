#[cfg(feature = "board-baosec")]
fn main() {
    dc34_scanner::run_on_hardware();
}

#[cfg(not(feature = "board-baosec"))]
fn main() {
    println!("dc34-scanner boot");
}
