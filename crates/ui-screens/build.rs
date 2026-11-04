use std::path::PathBuf;

fn main() {
    let source = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("../../assets/branding/client-badge.png");
    println!("cargo:rerun-if-changed={}", source.display());
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("output dir"));
    let mut image = image::open(&source)
        .unwrap_or_else(|error| panic!("read Umbra badge {}: {error}", source.display()))
        .into_rgba8();
    for pixel in image.pixels_mut() {
        let [red, green, blue, alpha] = pixel.0;
        let gray = ((u32::from(red) * 2_126
            + u32::from(green) * 7_152
            + u32::from(blue) * 722
            + 5_000)
            / 10_000) as u8;
        *pixel = image::Rgba([gray, gray, gray, alpha]);
    }
    image
        .save(out.join("umbra-badge-gray.png"))
        .expect("write grayscale Umbra badge");
}
