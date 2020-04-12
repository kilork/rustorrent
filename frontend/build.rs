fn main() {
    #[cfg(feature = "ui")]
    {
        use actix_web_static_files;
        use std::{env, path::Path};

        let out_dir = env::var("OUT_DIR").unwrap();

        let generated_files = Path::new(&out_dir).join("generated_www.rs");
        actix_web_static_files::NpmBuild::new("./www")
            .install()
            .unwrap()
            .run("build")
            .unwrap()
            .target("./www/target/classes/static")
            .to_resource_dir()
            .with_generated_filename(&generated_files)
            .with_generated_fn("generate_files")
            .build()
            .unwrap();
    }
}
