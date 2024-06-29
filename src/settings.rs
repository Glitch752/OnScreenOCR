use std::sync::{Arc, Mutex};
use include_dir::{include_dir, Dir};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::INITIALIZATION_ERRORS;

static SETTINGS_FILE_NAME: &str = "settings.bin";
static TESSERACT_SETTNGS_FILE_NAME: &str = "tesseract_settings.toml";

static DEFAULT_CONFIG_FILES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/default_config_files");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OCRLanguage {
    pub code: String,
    pub name: String,
}

impl OCRLanguage {
    pub fn new(code: &str, name: &str) -> Self {
        Self {
            code: code.to_string(),
            name: name.to_string()
        }
    }
}

#[derive(Debug, Serialize, Clone, Copy, Deserialize)]
pub struct Keybind {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool, // Windows key on Windows, Command key on macOS
    pub key: char
}

impl Default for Keybind {
    fn default() -> Self {
        Self {
            ctrl: false,
            shift: true,
            alt: true,
            meta: false,
            key: 'z'
        }
    }
}

impl Keybind {
    pub fn to_string(&self) -> String {
        let mut string = String::new();
        if self.ctrl {
            string.push_str("Ctrl + ");
        }
        if self.shift {
            string.push_str("Shift + ");
        }
        if self.alt {
            string.push_str("Alt + ");
        }
        if self.meta {
            string.push_str("Meta + ");
        }
        string.push(self.key);

        string
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsManager {
    pub use_polygon: bool,
    
    pub maintain_newline: bool,
    pub reformat_and_correct: bool,
    pub background_blur_enabled: bool,
    pub add_pilcrow_in_preview: bool,
    pub close_on_copy: bool,
    pub auto_copy: bool,

    /// Intended to be read-only by other modules -- use `set_open_keybind` to change.
    /// This is the case because we also set `open_keybind_string` so we don't need to
    /// lock the mutex every time we render the settings.
    pub open_keybind: Arc<Mutex<Keybind>>,
    #[serde(skip, default)]
    pub open_keybind_string: String,

    // Don't seriaize with the other settings; it's loaded from a separate file
    #[serde(skip)]
    pub tesseract_settings: TesseractSettings,
    
    #[serde(skip, default="crate::settings::get_project_dirs")]
    project_dirs: ProjectDirs
}

#[derive(Debug, Serialize, Copy, Clone, Deserialize)]
pub enum TesseractExportMode {
    UTF8,
    HOCR,
    Alto,
    TSV
}

fn verify(settings: TesseractSettings) -> Result<TesseractSettings, String> {
    if settings.ocr_languages.is_empty() {
        return Err("No OCR languages are defined".to_string());
    }

    Ok(settings)
}

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct TesseractSettings {
    pub ocr_language_code: String,
    pub export_mode: TesseractExportMode,

    pub ocr_languages: Vec<OCRLanguage>,

    pub tesseract_parameters: toml::Table,

    #[serde(skip, default="crate::settings::get_project_dirs")]
    project_dirs: ProjectDirs
}

pub fn get_project_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "", "OnScreenOCR").expect("Unable to get project directories")
}

impl Default for TesseractSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl TesseractSettings {
    fn new() -> Self {
        let project_dirs = get_project_dirs();
        let tesseract_settings_path = project_dirs.config_dir().join(TESSERACT_SETTNGS_FILE_NAME);

        if let Ok(encoded) = std::fs::read(&tesseract_settings_path) {
            let toml_string = String::from_utf8(encoded);
            if toml_string.is_err() {
                eprintln!("Failed to decode Tesseract setting string");
                std::fs::remove_file(&tesseract_settings_path).unwrap();
                INITIALIZATION_ERRORS.lock().unwrap().push("Failed to decode Tesseract setting string".to_string());
                return Self::new();
            }

            let toml_result = toml::from_str(&toml_string.unwrap());
            if let Err(error) = toml_result {
                eprintln!("Failed to deserialize Tesseract settings: {}", error);
                std::fs::remove_file(&tesseract_settings_path).unwrap();
                INITIALIZATION_ERRORS.lock().unwrap().push(format!("Failed to deserialize Tesseract settings: {}", error));
                return Self::new();
            }

            let verify_result = self::verify(toml_result.unwrap());
            if let Err(error) = verify_result {
                eprintln!("Failed to verify Tesseract settings: {}", error);
                std::fs::remove_file(&tesseract_settings_path).unwrap();
                INITIALIZATION_ERRORS.lock().unwrap().push(format!("Failed to verify Tesseract settings: {}", error));
                return Self::new();
            }
            
            return verify_result.unwrap();
        }

        let settings = Self {
            ocr_language_code: "eng".to_string(),
            tesseract_parameters: toml::Table::new(),
            export_mode: TesseractExportMode::UTF8,

            ocr_languages: vec![
                OCRLanguage::new("eng", "English"),
                OCRLanguage::new("eng_slow", "English (slow)"),
                OCRLanguage::new("deu", "German  "), // Spaces after this are intentional to make the layout look better
            ],

            project_dirs
        };

        settings.save();

        settings
    }

    fn save(&self) {
        let encoded = toml::to_string(&self).unwrap();
        // Not a perfect solution, but the comment isn't a huge deal
        let encoded = format!(r#"# The lower-level Tesseract configuration.
# If this file is messed up in such a way it can't be loaded, it will be re-created,
# and the old file will be stored uner `{}.bak`.

{}"#, TESSERACT_SETTNGS_FILE_NAME, encoded);
        let encoded = encoded.replace("[tesseract_parameters]", r#"# Each entry should be a value for a parameter.
# There are some useful parameters here: https://tesseract-ocr.github.io/tessdoc/tess3/ControlParams.html
# This is a (old) list of all parameters: http://www.sk-spell.sk.cx/tesseract-ocr-parameters-in-302-version
[tesseract_parameters]"#);
        let encoded = encoded.replace("export_mode = ", r#"
# The export mode. Possible values:
# "UTF8" - Normal text
# "HOCR" - An OCR-specific HTML format with coordinates, layout information, etc.
#    More information available here: https://en.wikipedia.org/wiki/HOCR
# "Alto" - An XML format originally designed for OCR data
#    More information available here: https://en.wikipedia.org/wiki/Analyzed_Layout_and_Text_Object
# "TSV" - Tab-separated values
# Note that turning "preserve newlines" off and "Reformat and correct results" will only work with "UTF8"
# If you don't know what to choose, "UTF8" is probably what you expect.
export_mode = "#);
        let encoded = encoded.replacen("[[ocr_languages]]", r#"# Each entry should be a language, with a corresponding [name].traineddata file under /tessdata.
# Name is an arbitrary string shown in the UI, and code is the language code.
# To support automatic correction for other languages, add associated dictionary text files
# to correction_data, following the existing conventions. More documentation is to come.
[[ocr_languages]]"#, 1);

        ensure_settings_dir(&self.project_dirs);

        let tesseract_settings_path = self.project_dirs.config_dir().join(TESSERACT_SETTNGS_FILE_NAME);
        std::fs::write(tesseract_settings_path, encoded).unwrap();
    }

    pub fn absolute_path(&self) -> String {
        let tesseract_settings_path = self.project_dirs.config_dir().join(TESSERACT_SETTNGS_FILE_NAME);
        std::fs::canonicalize(tesseract_settings_path).unwrap().to_string_lossy().to_string()
    }

    /// Puts initialization errors in the global error list
    pub fn reload(&mut self) {
        let tesseract_settings_path = self.project_dirs.config_dir().join(TESSERACT_SETTNGS_FILE_NAME);
        if let Ok(encoded) = std::fs::read(&tesseract_settings_path) {
            let toml_string = String::from_utf8(encoded);
            if toml_string.is_err() {
                eprintln!("Failed to decode Tesseract setting string");
                std::fs::remove_file(&tesseract_settings_path).unwrap();
                INITIALIZATION_ERRORS.lock().unwrap().push("Failed to decode Tesseract setting string".to_string());
                *self = Self::new();
                return;
            }

            let toml_result = toml::from_str(&toml_string.unwrap());
            if let Err(error) = toml_result {
                eprintln!("Failed to deserialize Tesseract settings: {}", error);
                std::fs::remove_file(&tesseract_settings_path).unwrap();
                INITIALIZATION_ERRORS.lock().unwrap().push(format!("Failed to deserialize Tesseract settings: {}", error));
                *self = Self::new();
                return;
            }

            let verify_result = self::verify(toml_result.unwrap());
            if let Err(error) = verify_result {
                eprintln!("Failed to verify Tesseract settings: {}", error);
                std::fs::remove_file(&tesseract_settings_path).unwrap();
                INITIALIZATION_ERRORS.lock().unwrap().push(format!("Failed to verify Tesseract settings: {}", error));
                *self = Self::new();
                return;
            }
            
            *self = verify_result.unwrap();
            return;
        } else {
            eprintln!("Failed to read Tesseract settings, using default settings and overwriting the file");
            std::fs::rename(&tesseract_settings_path, tesseract_settings_path.with_extension(".toml.bak")).unwrap();
            *self = TesseractSettings::new();
            INITIALIZATION_ERRORS.lock().unwrap().push("Failed to read Tesseract settings, using default settings and overwriting the file".to_string());
            return;
        }
    }

    pub fn get_ocr_language_data(&self) -> OCRLanguage {
        self.ocr_languages.iter().find(|x| x.code == self.ocr_language_code).unwrap().clone()
    }

    pub fn ocr_language_increment(&mut self) {
        let current_language_index = self.ocr_languages.iter().position(|x| x.code == self.ocr_language_code).unwrap();
        self.ocr_language_code = self.ocr_languages[(current_language_index + 1) % self.ocr_languages.len()].code.to_string();
    }

    pub fn ocr_language_decrement(&mut self) {
        let current_language_index = self.ocr_languages.iter().position(|x| x.code == self.ocr_language_code).unwrap();
        self.ocr_language_code = self.ocr_languages[(current_language_index + self.ocr_languages.len() - 1) % self.ocr_languages.len()].code.to_string();
    }

    pub fn configure_tesseract(&self, api: &mut leptess::tesseract::TessApi) {
        for (k, v) in &self.tesseract_parameters {
            let k = std::ffi::CString::new(k.to_string()).unwrap();
            let value_string = match v {
                toml::Value::String(s) => s.clone(),
                toml::Value::Integer(i) => i.to_string(),
                toml::Value::Float(f) => f.to_string(),
                _ => continue
            };
            let v = std::ffi::CString::new(value_string).unwrap();

            let result = api.raw.set_variable(&k, &v);
            if result.is_err() {
                // Ignore, but warn the user
                eprintln!("Failed to set Tesseract variable: {:?}", result);
            }
        }
    }
}

impl SettingsManager {
    pub fn new() -> Self {
        let project_dirs = get_project_dirs();
        let settings_file_path = project_dirs.config_dir().join(SETTINGS_FILE_NAME);

        if let Ok(encoded) = std::fs::read(&settings_file_path) {
            let deserialized = bincode::deserialize(&encoded).map(|mut val: SettingsManager| {
                val.open_keybind_string = val.open_keybind.lock().unwrap().to_string();
                val
            });
            
            if let Err(error) = deserialized {
                eprintln!("Failed to deserialize settings, using default settings and overwriting the file");
                std::fs::remove_file(&settings_file_path).unwrap();
                INITIALIZATION_ERRORS.lock().unwrap().push(format!("Failed to deserialize settings; using defaults: {}", error.to_string()));
                return Self::new();
            }

            return deserialized.unwrap();
        }

        // Default settings
        Self {
            use_polygon: false,
            maintain_newline: true,
            reformat_and_correct: true,
            background_blur_enabled: true,
            add_pilcrow_in_preview: true,
            close_on_copy: false,
            auto_copy: false,

            tesseract_settings: TesseractSettings::new(),

            open_keybind: Arc::new(Mutex::new(Keybind::default())),
            open_keybind_string: "Shift + Alt + Z".to_string(),

            project_dirs
        }
    }

    pub fn set_open_keybind(&mut self, keybind: Keybind) {
        *self.open_keybind.lock().unwrap() = keybind;
        self.open_keybind_string = keybind.to_string();
    }

    pub fn save(&self) {
        ensure_settings_dir(&self.project_dirs);

        let settings_file_path = self.project_dirs.config_dir().join(SETTINGS_FILE_NAME);
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        std::fs::write(&settings_file_path, encoded).unwrap();

        self.tesseract_settings.save();
    }

    pub fn get_ocr_languages(&self) -> Vec<OCRLanguage> {
        self.tesseract_settings.ocr_languages.to_vec()
    }
}

fn ensure_settings_dir(project_dirs: &ProjectDirs) {
    let config_dir = project_dirs.config_dir();
    if !config_dir.exists() || std::fs::read_dir(config_dir).map(|dir| dir.count()).unwrap_or(0) == 0 {
        std::fs::create_dir_all(config_dir).unwrap();

        for entry in DEFAULT_CONFIG_FILES.find("**/*").unwrap() {
            if entry.as_dir().is_some() {
                continue;
            }

            let path = config_dir.join(entry.path());

            std::fs::create_dir_all(path.parent().expect("Failed to get parent directory")).unwrap();
            std::fs::write(path, entry.as_file().unwrap().contents()).unwrap();
        }

        println!("Default settings and configuration files have been created in {}", config_dir.to_string_lossy());
    }
}