# OnScreenOCR

A tool designed to mimic Microsoft PowerToys' "Text Extract feature", but with additional functionality:
- Fully GPU-accelerated rendering using wgpu
- Multi-platform support: Windows, MacOS, and Linux (untested at the moment, some features TODO on other platforms)
- Live preview of the OCR result
- Support for taking screenshots
- Support for multiple OCR languages
- Result fixing and reformatting
  - Reformat to remove hyphens from end of lines, moving the word to fit entirely on the line
  - More todo
- Ability to copy without newlines
- Ability to fine-tune Tesseract's parameters
  - Ability to export in other Tesseract formats (TSV, Alto, HOCR)
- Support for non-rectangular selections
- Support for multiple monitors

## TODO
- [ ] Add support for MacOS and Linux
- [ ] Feedback for copying to clipboard
- [ ] A setting to change the OCR keybind
- [ ] Undo/redo for selection edits
- [ ] Add OCR language list to tesseract settings file
- [ ] Handle "preserve newlines" being off for Alto and HOCR with an XML parser
- [ ] Ctrl-A to select entire screen
- [ ] Ability to drag polygon path with CTRL held
- [ ] Stay in system tray when closed

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