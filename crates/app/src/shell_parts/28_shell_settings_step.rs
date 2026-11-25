impl Shell {
    /// Ajusta el intervalo de escaneo de agentes.
    fn settings_step(&mut self, field: StepField, delta: f32, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        match field {
            StepField::ScanInterval => settings.step_scan_interval(delta),
        }
        cx.notify();
    }

    /// Captura la tecla pulsada durante un rebind. Devuelve `true` si la consumió.
    fn settings_capture_keystroke(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(action) = self
            .settings
            .as_ref()
            .and_then(|settings| settings.capturing.clone())
        else {
            return false;
        };
        let key = keystroke.key.to_ascii_lowercase();
        if matches!(
            key.as_str(),
            "control"
                | "ctrl"
                | "shift"
                | "alt"
                | "meta"
                | "super"
                | "win"
                | "cmd"
                | "fn"
                | "function"
        ) {
            return false;
        }
        let result = if key == "escape" {
            if let Some(settings) = &mut self.settings {
                settings.cancel_capture();
            }
            Ok(())
        } else if key == "backspace" || key == "delete" {
            self.settings
                .as_mut()
                .map(|settings| settings.unbind_action(&action))
                .unwrap_or(Ok(()))
        } else {
            let chord = Self::chord_from_keystroke(keystroke);
            if chord.is_empty() {
                return false;
            }
            self.settings
                .as_mut()
                .map(|settings| settings.set_binding(&action, &chord))
                .unwrap_or(Ok(()))
        };
        match result {
            Ok(()) => {
                if let Some(settings) = &mut self.settings
                    && settings.capturing.is_none()
                    && settings.notice.is_none()
                {
                    settings.set_notice("Atajo actualizado en el borrador.".into(), false);
                }
            }
            Err(error) => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(error, true);
                }
            }
        }
        cx.notify();
        true
    }
}
