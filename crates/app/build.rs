//! Metadatos e icono de Windows embebidos en el ejecutable (M9).
//!
//! `winresource` usa el `rc.exe` del Windows SDK. Solo se ejecuta al compilar
//! para Windows; el resto de plataformas no arrastra la dependencia.

fn main() {
    println!("cargo:rerun-if-changed=assets/axfterm.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/axfterm.ico");
        resource.set("ProductName", "Axfterm");
        resource.set("FileDescription", "Axfterm — terminal workspace");
        resource.set("OriginalFilename", "axfterm.exe");
        resource.set("InternalName", "axfterm");
        resource.set("LegalCopyright", "© 2026 Juan Andrés");
        resource.set("CompanyName", "Axfterm");
        resource.set("FileVersion", env!("CARGO_PKG_VERSION"));
        resource.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        if let Err(error) = resource.compile() {
            // Un fallo del compilador de recursos no debe romper una build de
            // desarrollo sin SDK; se avisa y el binario sigue sin metadatos.
            println!("cargo:warning=no se pudieron embeber los metadatos de Windows: {error}");
        }
    }
}
