fn validate_wsl_distro(distro: &str) -> Result<(), WorkspaceError> {
    if distro.trim().is_empty() {
        return Err(WorkspaceError::InvalidHost {
            message: "la distribución WSL no puede estar vacía".into(),
        });
    }
    if distro.chars().any(char::is_control) {
        return Err(WorkspaceError::InvalidHost {
            message: "el nombre de la distribución WSL contiene caracteres de control".into(),
        });
    }
    Ok(())
}

/// Normaliza una ruta POSIX sin tocar el sistema de archivos.
///
/// Colapsa barras repetidas y quita la barra final, salvo en la raíz. La
/// canonicalización real (symlinks, `.`/`..`) la hace el adaptador WSL antes de
/// registrar el proyecto.
fn normalize_posix(path: &Path) -> Result<PathBuf, WorkspaceError> {
    let text = path.to_string_lossy();
    if !text.starts_with('/') {
        return Err(WorkspaceError::InvalidHost {
            message: format!(
                "la ruta WSL debe ser absoluta (empezar por /): {}",
                path.display()
            ),
        });
    }
    if text.contains('\0') {
        return Err(WorkspaceError::InvalidHost {
            message: "la ruta WSL contiene un carácter nulo".into(),
        });
    }
    let mut normalized = String::with_capacity(text.len());
    let mut previous_slash = false;
    for character in text.chars() {
        if character == '/' {
            if previous_slash {
                continue;
            }
            previous_slash = true;
        } else {
            previous_slash = false;
        }
        normalized.push(character);
    }
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    Ok(PathBuf::from(normalized))
}
