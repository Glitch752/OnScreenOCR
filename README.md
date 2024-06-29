# OnScreenOCR

A tool designed to mimic Microsoft PowerToys' "Text Extract feature", but with additional functionality:
- Fully GPU-accelerated rendering using wgpu
- Multi-platform support: Windows, MacOS, and Linux (untested at the moment, some features TODO on other platforms)
- Live preview of the OCR result
- Support for taking screenshots
- Support for multiple OCR languages
- Result fixing and reformatting
  - Reformat to remove hyphens from end of lines, moving the word to fit entirely on the line
  - More to come in the future! If you have any suggestions or find common formatting failure points/annoyances, please open an issue!
- Ability to copy without newlines
- Ability to fine-tune Tesseract's parameters
  - Ability to export in other Tesseract formats (TSV, Alto, HOCR)
- Support for non-rectangular selections
- Support for multiple monitors
- Keybinds for common actions (Ctrl+C to copy, Ctrl+Z to undo, arrows to move selection, etc.)
- Full undo/redo history
- Stays in system tray when closed
- Numerous intuitive selection-related interactions, including drawing outlines, shifting edges/vertices, removing edges/vertices, and more.

## Configuration
Configuration files are stored under your user's configuration directory.  
On Windows, this is `{FOLDERID_RoamingAppData}\OnScreenOCR` (usually `C:\Users\<username>\AppData\Roaming\OnScreenOCR`).
On MacOS, this is `~/Library/Application Support/OnScreenOCR`.  
On Linux, this is `$XDG_CONFIG_HOME/OnScreenOCR` or `$HOME/.config/OnScreenOCR`.  

Configuration files include:
- `settings.bin`: A binary file containing the application's simple settings, configurable from the application. (e.g. preserve newlines or background blur enabled)
- `tesseract_settings.toml`: A TOML file containing the Tesseract settings. When changing this file, the "reload" button must be clicked in the application. (e.g. OCR language, OCR parameters)
- `tessdata`: A directory containing the Tesseract data files. This directory is created when the application is run for the first time with a few default languages. To add more languages, simply copy the `.traineddata` files into this directory and add the language to the `tesseract_settings.toml` file. Configuration is documented in the file.
- `correction_data`: A directory containing the correcton data files. There is one subdirectory per required dictionary for correction. If no file exists for a language, the correction will not be applied.
  - Currently, there's only one type of correction -- the "hyphenated" correction. For every language you add that requires end-of-line hyphen correction, you should add a dictionary of hyphenated words to `correction_data/hyphenated/{language code}.txt`, with one word per line.

## TODO
- [ ] Add support for MacOS and Linux
- [ ] Optionally automatically start on boot
- [ ] Proper installer and uninstaller
- [ ] Better documentation on how the interaction system, configuration, and probably other parts of the application work

## Installation
For now, the way to install is by cloning the repository, building the project, and running it manually.  
However, I plan to release standalone binaries once I get all the packaging working.

```bash
git clone --recurse-submodules https://github.com/Glitch752/OnScreenOCR.git
cd OnScreenOCR
cargo build --release
# You can now run target/release/BetterOCRTool.exe from the project root
./target/release/OnScreenOCR.exe
```

## Development
Since the OCR dependency used ([Leptess](https://github.com/houqp/leptess)) relies on vcpkg dependencies, you need to run the following (and clone with submodules!):
```bash
# To install the LLVM (clang)
winget install LLVM

# To install vcpkg dependencies
.\vcpkg\bootstrap-vcpkg.bat
.\vcpkg\vcpkg integrate install
.\vcpkg\vcpkg install tesseract:x64-windows-static-md
.\vcpkg\vcpkg install leptonica:x64-windows-static-md
```
NOTE: When installing, vcpkg can't be in a directory with spaces in the path!

On platforms other than Windows, follow the instructons in Leptess' README [here](https://github.com/houqp/leptess?tab=readme-ov-file#build-dependencies).

Finally, you can run the project:
```bash
cargo run
```