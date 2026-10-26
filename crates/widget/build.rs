//! The Windows executable gets the port's icon (`res/icon.ico`,
//! `tools/make-icon.py`), when it's built on Windows.

fn main() {
    println!("cargo:rerun-if-changed=res/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("res/icon.ico");
        res.compile().expect("the icon resource");
    }
}
