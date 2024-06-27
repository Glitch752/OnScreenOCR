use serde::{Deserialize, Serialize};

static OCR_LANGUAGES: [OCRLanguage; 3] = [
    OCRLanguage { code: "eng", name: "English" },
    OCRLanguage { code: "eng_slow", name: "English (slow)" },
    OCRLanguage { code: "deu", name: "German  " }, // Spaces after this are intentional to make the layout look better
];
static DEFAULT_OCR_LANGUAGE: OCRLanguage = OCR_LANGUAGES[0];

static SETTINGS_PATH: &str = "settings.bin";
static TESSERACT_SETTNGS_PATH: &str = "tesseract_settings.toml";
    
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OCRLanguage {
    pub code: &'static str,
    pub name: &'static str,
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

    // Don't seriaize with the other settings; it's loaded from a separate file
    #[serde(skip)]
    pub tesseract_settings: TesseractSettings,
}

#[derive(Debug, Serialize, Copy, Clone, Deserialize)]
pub enum TesseractExportMode {
    UTF8,
    HOCR,
    Alto,
    TSV
}

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct TesseractSettings {
    pub ocr_language_code: String,
    pub export_mode: TesseractExportMode,

    pub tesseract_parameters: toml::Table
}

impl Default for TesseractSettings {
    fn default() -> Self {
        if let Ok(encoded) = std::fs::read(TESSERACT_SETTNGS_PATH) {
            let toml_string = String::from_utf8(encoded);
            if toml_string.is_err() {
                eprintln!("Failed to decode Tesseract setting string, using default settings and overwriting the file");
                std::fs::remove_file(TESSERACT_SETTNGS_PATH).unwrap();
                return Self::default();
            }

            return toml::from_str(&toml_string.unwrap()).unwrap_or_else(|_| {
                eprintln!("Failed to deserialize Tesseract settings, using default settings and overwriting the file");
                std::fs::remove_file(TESSERACT_SETTNGS_PATH).unwrap();
                Self::default()
            })
        }

        Self {
            ocr_language_code: DEFAULT_OCR_LANGUAGE.code.to_string(),
            tesseract_parameters: toml::Table::new(),
            export_mode: TesseractExportMode::UTF8
        }
    }
}

impl TesseractSettings {
    fn save(&self) {
        let encoded = toml::to_string(&self).unwrap();
        // Not a perfect solution, but the comment isn't a huge deal
        let encoded = format!(r#"# The lower-level Tesseract configuration.
# If this file is messed up in such a way it can't be loaded, it will be re-created,
# and the old file will be stored uner `{}.bak`.

{}"#, TESSERACT_SETTNGS_PATH, encoded);
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
        std::fs::write(TESSERACT_SETTNGS_PATH, encoded).unwrap();
    }

    pub fn absolute_path(&self) -> String {
        std::fs::canonicalize(TESSERACT_SETTNGS_PATH).unwrap().to_string_lossy().to_string()
    }

    pub fn reload(&mut self) {
        if let Ok(encoded) = std::fs::read(TESSERACT_SETTNGS_PATH) {
            let toml_string = String::from_utf8(encoded);
            if toml_string.is_err() {
                eprintln!("Failed to decode Tesseract setting string, using default settings and overwriting the file");
                std::fs::rename(TESSERACT_SETTNGS_PATH, format!("{}.bak", TESSERACT_SETTNGS_PATH)).unwrap();
                *self = TesseractSettings::default();
                return;
            }

            *self = toml::from_str(&toml_string.unwrap()).unwrap_or_else(|_| {
                eprintln!("Failed to deserialize Tesseract settings, using default settings and overwriting the file");
                std::fs::rename(TESSERACT_SETTNGS_PATH, format!("{}.bak", TESSERACT_SETTNGS_PATH)).unwrap();
                TesseractSettings::default()
            });
        }
    }

    pub fn get_ocr_language_data(&self) -> OCRLanguage {
        OCR_LANGUAGES.iter().find(|x| x.code == self.ocr_language_code).unwrap().clone()
    }

    pub fn ocr_language_increment(&mut self) {
        self.ocr_language_code = OCR_LANGUAGES[(OCR_LANGUAGES.iter().position(|&x| x.code == self.ocr_language_code).unwrap() + 1) % OCR_LANGUAGES.len()].code.to_string();
    }

    pub fn ocr_language_decrement(&mut self) {
        self.ocr_language_code = OCR_LANGUAGES[(OCR_LANGUAGES.iter().position(|&x| x.code == self.ocr_language_code).unwrap() + OCR_LANGUAGES.len() - 1) % OCR_LANGUAGES.len()].code.to_string();
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

impl Default for SettingsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsManager {
    pub fn new() -> Self {
        if let Ok(encoded) = std::fs::read(SETTINGS_PATH) {
            return bincode::deserialize(&encoded).unwrap_or_else(|_| {
                eprintln!("Failed to deserialize settings, using default settings and overwriting the file");
                std::fs::remove_file(SETTINGS_PATH).unwrap();
                Self::default()
            })
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

            tesseract_settings: TesseractSettings::default()
        }
    }

    pub fn save(&self) {
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        std::fs::write(SETTINGS_PATH, encoded).unwrap();

        self.tesseract_settings.save();
    }

    pub fn get_ocr_languages(&self) -> Vec<OCRLanguage> {
        OCR_LANGUAGES.to_vec()
    }
}