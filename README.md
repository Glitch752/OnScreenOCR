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
- Keybinds for common actions (Ctrl+C to copy, Ctrl+Z to undo, arrows to move selection, etc.)
- Full undo/redo history
- Stays in system tray when closed

## TODO
- [ ] Add support for MacOS and Linux
- [ ] Feedback for copying to clipboard
- [ ] Ability to drag polygon path with CTRL held
- [ ] Optionally automatically start on boot
- [ ] Proper installer and uninstaller

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