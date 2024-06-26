# OnScreenOCR

A tool designed to mimic Microsoft PowerToys' "Text Extract feature", but with additional functionality:
- Multi-platform support: Windows, MacOS, and Linux (untested at the moment, some features TODO on other platforms)
- Live preview of the OCR result
- Take screenshot
- Support for multiple OCR languages
- Result fixing and reformatting
  - Reformat to remove hyphens from end of lines, moving the word to fit entirely on the line
  - More todo
- Ability to copy without newlines
- Ability to fine-tune Tesseract's parameters
- GPU-accelerated rendering of everything using wgpu

## TODO
- [ ] Add support for MacOS and Linux
- [ ] Feedback for copying to clipboard
- [ ] A setting to change the OCR keybind
- [ ] Allow exporting with tesseract's TSV mode (and maybe some others)
- [ ] Free-form selection of OCR region

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