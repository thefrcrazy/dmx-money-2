//! Lecture bornée des fichiers importés, hors du fil GTK.
use crate::store::{FormRequest, Store};
use std::io::Read;
use std::path::Path;
use std::rc::Rc;

pub const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

/// UTF-8 strict, puis Windows-1252 (les caractères 0x80–0x9f diffèrent de Latin-1).
pub fn decode_text(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text.trim_start_matches('\u{feff}').to_string(),
        Err(error) => {
            const EXTENDED: [char; 32] = [
                '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
                '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
            ];
            error
                .into_bytes()
                .into_iter()
                .map(|byte| {
                    if (0x80..=0x9f).contains(&byte) {
                        EXTENDED[(byte - 0x80) as usize]
                    } else {
                        char::from(byte)
                    }
                })
                .collect()
        }
    }
}

pub fn read_text(path: &Path) -> Result<String, String> {
    let backup = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dmx") || extension.eq_ignore_ascii_case("json"));
    let max_bytes = if backup { MAX_IMPORT_BYTES } else { 16 * 1024 * 1024 };
    let too_large = || format!("Le fichier dépasse la limite de {} Mio.", max_bytes / (1024 * 1024));
    let file = std::fs::File::open(path).map_err(|error| format!("Le fichier n'a pas pu être lu. ({error})"))?;
    if file.metadata().map_err(|error| error.to_string())?.len() > max_bytes {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > max_bytes {
        return Err(too_large());
    }
    Ok(decode_text(bytes))
}

pub fn open(store: &Rc<Store>, file: gtk::gio::File) {
    use gtk::gio::prelude::*;
    let Some(path) = file.path() else {
        store.show_error("Sélectionnez un fichier local.");
        return;
    };
    let store = store.clone();
    gtk::glib::spawn_future_local(async move {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "import".to_string());
        let backup = path.extension().is_some_and(|extension| {
            extension.to_str().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("dmx") || extension.eq_ignore_ascii_case("json")
            })
        });
        match gtk::gio::spawn_blocking(move || read_text(&path)).await {
            Ok(Ok(content)) => store.present(if backup {
                FormRequest::RestoreBackup { content, file_name }
            } else {
                FormRequest::StatementImport { content, file_name }
            }),
            Ok(Err(message)) => store.show_error(&message),
            Err(_) => store.show_error("La lecture du fichier a été interrompue."),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_1252_preserves_bank_punctuation_and_euro() {
        assert_eq!(
            decode_text(vec![
                0x80, 0x20, 0x93, b'C', 0x9c, b'u', b'r', 0x94, 0x20, 0x96, 0x20, 0xe9
            ]),
            "€ “Cœur” – é"
        );
    }
    #[test]
    fn utf8_and_bom_are_preserved() {
        assert_eq!(decode_text("\u{feff}Cœur €".as_bytes().to_vec()), "Cœur €");
    }
    #[test]
    fn oversized_file_is_rejected_before_reading() {
        let path = std::env::temp_dir().join(format!("dmx-import-limit-{}.dmx", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_IMPORT_BYTES + 1).unwrap();
        assert!(read_text(&path).unwrap_err().contains("64 Mio"));
        std::fs::remove_file(path).unwrap();
    }
}
