//! Compile the bundled icon GResource so the app ships its own symbolic icons
//! and never shows broken/missing glyphs when the system icon theme lacks them.

fn main() {
    glib_build_tools::compile_resources(
        &["icons"],
        "icons/bigtube.gresource.xml",
        "bigtube.gresource",
    );
}
