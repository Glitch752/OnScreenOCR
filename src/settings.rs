use serde::{Deserialize, Serialize};

static OCR_LANGUAGES: [OCRLanguage; 2] = [
    OCRLanguage { code: "eng", name: "English" },
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

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct TesseractSettings {
    pub ocr_language_code: String,

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
            tesseract_parameters: toml::Table::new()
        }
    }
}

impl TesseractSettings {
    fn save(&self) {
        let encoded = toml::to_string(&self).unwrap();
        // Not a perfect solution, but the comment isn't a huge deal
        let encoded = encoded.replace("[tesseract_parameters]", r#"# Each entry should be a value for a parameter.
# There are some useful parameters here: https://tesseract-ocr.github.io/tessdoc/tess3/ControlParams.html
# This is a (old) list of all parameters: http://www.sk-spell.sk.cx/tesseract-ocr-parameters-in-302-version
[tesseract_parameters]"#);
        std::fs::write(TESSERACT_SETTNGS_PATH, encoded).unwrap();
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