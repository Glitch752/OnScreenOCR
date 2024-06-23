# OnScreenOCR

A tool designed to mimic Microsoft PowerToys' "Text Extract feature", but with additional functionality:
- Multi-platform support: Windows, MacOS, and Linux (untested at the moment, some features TODO on other platforms)
- Optional blurred background
- Live preview of the OCR result
...todo

## Development
Since the OCR dependency used ([Leptess](https://github.com/houqp/leptess)) relies on vcpkg dependencies, you need to run the following (and clone with submodules!):
```bash
.\vcpkg\bootstrap-vcpkg.bat
.\vcpkg install tesseract:x64-windows
```
On platforms other than Windows, follow the instructons in Leptess' README [here](https://github.com/houqp/leptess?tab=readme-ov-file#build-dependencies).

Finally, you can run the project:
```bash
cargo run
```