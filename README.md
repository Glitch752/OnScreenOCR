# OnScreenOCR

A tool designed to mimic Microsoft PowerToys' "Text Extract feature", but with additional functionality:
- Multi-platform support: Windows, MacOS, and Linux (untested at the moment, some features TODO on other platforms)
- Optional blurred background
- Live preview of the OCR result

## TODO
- [ ] Ability to capture screenshots (since we effectively do that anyway)
- [ ] Change cursor to crosshair when selecting region
- [ ] Test and add support for MacOS and Linux
- [ ] Tooltips when hovering over buttons
- [ ] Feedback for copying to clipboard
- [ ] A setting to change the OCR keybind
- [ ] Allow for configuring other tesseract options (e.g. blacklisted characters, etc.)
  - [ ] Allow exporting with tesseract's TSV mode (and maybe some others)

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