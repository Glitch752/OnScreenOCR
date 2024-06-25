use serde::{Deserialize, Serialize};

static OCR_LANGUAGES: [OCRLanguage; 2] = [
    OCRLanguage { code: "eng", name: "English" },
    OCRLanguage { code: "deu", name: "German" },
];
static DEFAULT_OCR_LANGUAGE: OCRLanguage = OCR_LANGUAGES[0];

static SETTINGS_PATH: &str = "settings.bin";
    
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OCRLanguage {
    pub code: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsManager {
    pub maintain_newline: bool,
    pub reformat_and_correct: bool,
    pub ocr_language_code: String,
    pub background_blur_enabled: bool,
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
            ocr_language_code: DEFAULT_OCR_LANGUAGE.code.to_string(),
            background_blur_enabled: true,
        }
    }

    pub fn save(&self) {
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        std::fs::write(SETTINGS_PATH, encoded).unwrap();
    }

    pub fn get_ocr_languages(&self) -> Vec<OCRLanguage> {
        OCR_LANGUAGES.to_vec()
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
}