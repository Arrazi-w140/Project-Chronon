use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

const FONT_DIRECTORY: &str = "fonts";
const FONT_LIBRARY_FILE: &str = "font-library.json";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedFont {
    pub id: String,
    pub name: String,
    pub family: String,
    pub path: String,
    pub format: String,
}

pub fn initialize(app: &AppHandle) -> Result<(), String> {
    font_directory(app).map(|_| ())
}

fn font_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to locate the application data directory: {err}"))?
        .join(FONT_DIRECTORY);
    fs::create_dir_all(&directory)
        .map_err(|err| format!("Failed to create the font directory: {err}"))?;
    Ok(directory)
}

fn library_path(directory: &Path) -> PathBuf {
    directory.join(FONT_LIBRARY_FILE)
}

fn read_library(directory: &Path) -> Result<Vec<ImportedFont>, String> {
    let path = library_path(directory);
    match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents)
            .map_err(|err| format!("Failed to read the font library: {err}")),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(format!("Failed to read the font library: {err}")),
    }
}

fn write_library(directory: &Path, fonts: &[ImportedFont]) -> Result<(), String> {
    let contents = serde_json::to_string_pretty(fonts)
        .map_err(|err| format!("Failed to save the font library: {err}"))?;
    fs::write(library_path(directory), contents)
        .map_err(|err| format!("Failed to save the font library: {err}"))
}

fn supported_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "ttf" | "otf" | "woff" | "woff2" => Some(extension),
        _ => None,
    }
}

fn css_format(extension: &str) -> &'static str {
    match extension {
        "ttf" => "truetype",
        "otf" => "opentype",
        "woff" => "woff",
        "woff2" => "woff2",
        _ => "truetype",
    }
}

fn display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| "Imported Font".to_string())
}

fn identifier_part(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "font".to_string()
    } else {
        trimmed.to_string()
    }
}

fn next_font_id(directory: &Path, name: &str, extension: &str) -> (String, PathBuf) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let base = identifier_part(name);
    let mut counter = 0_u32;

    loop {
        let id = if counter == 0 {
            format!("{base}-{timestamp}")
        } else {
            format!("{base}-{timestamp}-{counter}")
        };
        let destination = directory.join(format!("{id}.{extension}"));
        if !destination.exists() {
            return (id, destination);
        }
        counter += 1;
    }
}

fn list_fonts(app: &AppHandle) -> Result<Vec<ImportedFont>, String> {
    let directory = font_directory(app)?;
    let mut fonts = read_library(&directory)?;
    let initial_count = fonts.len();
    fonts.retain(|font| {
        let path = Path::new(&font.path);
        path.starts_with(&directory) && path.is_file()
    });
    if fonts.len() != initial_count {
        write_library(&directory, &fonts)?;
    }
    Ok(fonts)
}

#[tauri::command]
pub fn list_imported_fonts(app: AppHandle) -> Result<Vec<ImportedFont>, String> {
    list_fonts(&app)
}

#[tauri::command]
pub fn import_fonts(app: AppHandle) -> Result<Vec<ImportedFont>, String> {
    let selected = rfd::FileDialog::new()
        .set_title("Load Fonts")
        .add_filter("Font files", &["ttf", "otf", "woff", "woff2"])
        .pick_files()
        .unwrap_or_default();
    if selected.is_empty() {
        return Ok(Vec::new());
    }

    let directory = font_directory(&app)?;
    let mut library = list_fonts(&app)?;
    let mut added = Vec::new();

    for source in selected {
        let extension = supported_extension(&source)
            .ok_or_else(|| "Only .ttf, .otf, .woff, and .woff2 files are supported.".to_string())?;
        if !source.is_file() {
            return Err("The selected font file could not be found.".to_string());
        }

        let name = display_name(&source);
        let (id, destination) = next_font_id(&directory, &name, &extension);
        fs::copy(&source, &destination)
            .map_err(|err| format!("Failed to import {}: {err}", name))?;

        let font = ImportedFont {
            family: format!("ChrononImported-{id}"),
            path: destination.to_string_lossy().into_owned(),
            format: css_format(&extension).to_string(),
            id,
            name,
        };
        library.push(font.clone());
        added.push(font);
    }

    write_library(&directory, &library)?;
    Ok(added)
}

#[tauri::command]
pub fn delete_imported_font(app: AppHandle, id: String) -> Result<(), String> {
    let directory = font_directory(&app)?;
    let mut library = list_fonts(&app)?;
    let index = library
        .iter()
        .position(|font| font.id == id)
        .ok_or_else(|| "Imported font not found.".to_string())?;
    let font = library.remove(index);
    let path = PathBuf::from(font.path);
    if !path.starts_with(&directory) {
        return Err("The requested font is outside the application font library.".to_string());
    }
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("Failed to delete the imported font: {err}"))?;
    }
    write_library(&directory, &library)
}
