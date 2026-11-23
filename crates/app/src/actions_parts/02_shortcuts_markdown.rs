#[cfg(test)]
pub(crate) fn shortcuts_markdown(
    effective: &std::collections::BTreeMap<String, Vec<String>>,
) -> String {
    let mut text = String::from(
        "# Atajos de teclado\n\n\
         <!-- Generado desde el registro de acciones; no editar a mano. -->\n\n\
         Todos los atajos se pueden cambiar en la sección `[keymap]` de\n\
         `config.toml`. Un atajo por defecto se desactiva con `unbind`.\n\n",
    );
    for category in ActionCategory::ALL {
        let actions: Vec<&ActionMeta> = all()
            .filter(|action| action.category == *category && action.enabled)
            .collect();
        if actions.is_empty() {
            continue;
        }
        text.push_str(&format!("## {}\n\n", category.label()));
        text.push_str("| Acción | Atajo | Id |\n| --- | --- | --- |\n");
        for action in actions {
            let chords = effective
                .get(action.id)
                .map(|chords| chords.join(", "))
                .filter(|chords| !chords.is_empty())
                .unwrap_or_else(|| "—".to_owned());
            text.push_str(&format!(
                "| {} | `{}` | `{}` |\n",
                action.label,
                chords.replace('`', ""),
                action.id
            ));
        }
        text.push('\n');
    }
    text.push_str(
        "## Configurar atajos\n\n\
         ```toml\n\
         [keymap]\n\
         # Añade un atajo alternativo; el de por defecto sigue activo.\n\
         \"ctrl-alt-shift-n\" = \"tab.new\"\n\
         # Quita todos los atajos por defecto de una acción.\n\
         unbind = [\"app.quit\"]\n\n\
         [keymap.terminal]\n\
         \"ctrl-shift-c\" = \"terminal.copy\"\n\
         ```\n\n\
         Los contextos disponibles son `workspace`, `sidebar` y `terminal`.\n\
         Los atajos de app requieren modificador; `Ctrl+C`, `Ctrl+D`, `Ctrl+Z`,\n\
         `Tab`, `Esc` y las flechas quedan siempre para la terminal.\n",
    );
    text
}
pub(crate) fn category_icon(category: ActionCategory) -> &'static str {
    match category {
        ActionCategory::App => axf_ui::icons::SETTINGS,
        ActionCategory::Project => axf_ui::icons::FOLDER,
        ActionCategory::Pane => axf_ui::icons::SPLIT_HORIZONTAL,
        ActionCategory::Tab => axf_ui::icons::TERMINAL,
        ActionCategory::Profile => axf_ui::icons::TERMINAL,
        ActionCategory::Theme => axf_ui::icons::PALETTE,
        ActionCategory::Agent => axf_ui::icons::BRAND_OPENCODE,
        ActionCategory::Help => axf_ui::icons::KEYBOARD,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_action_builds_a_gpui_action() {
        for action in all() {
            assert!(
                action_for(action.id).is_some(),
                "la acción `{}` no tiene constructor GPUI",
                action.id
            );
        }
    }

    #[test]
    fn command_palette_hides_self_references_and_duplicates() {
        let visible: Vec<&str> = palette_actions().map(|action| action.id).collect();
        for hidden in [
            "palette.open",
            "palette.search",
            "tab.next",
            "tab.previous",
            "pane.focus_next",
            "pane.focus_previous",
            "theme.toggle_variant",
            "theme.next",
            "theme.previous",
            "theme.reload",
            "theme.open_folder",
            "theme.import",
            "theme.export",
        ] {
            assert!(
                !visible.contains(&hidden),
                "`{hidden}` no debe listarse en la paleta de comandos"
            );
            assert!(
                by_id(hidden).is_some_and(|action| action.enabled),
                "`{hidden}` sigue existiendo para atajos y ayuda"
            );
        }
        for essential in [
            "tab.new",
            "tab.close",
            "pane.split_horizontal",
            "pane.close",
            "project.open",
            "settings.open",
            "help.shortcuts",
        ] {
            assert!(
                visible.contains(&essential),
                "`{essential}` debe seguir en la paleta de comandos"
            );
        }
    }
}
