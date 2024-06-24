use winit::monitor::MonitorHandle;

extern crate libc;

#[derive(Debug, Clone)]
pub(crate) struct Screenshot {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

pub(crate) fn screenshot_from_handle(monitor: MonitorHandle) -> Screenshot {
    let ss = crate::screenshot::ffi::get_screenshot_from_matching(monitor.position().into(), monitor.size().into()).unwrap();
    let ss_bytes = ss.as_ref().to_vec();

    Screenshot {
        width: ss.width(),
        height: ss.height(),
        bytes: ss_bytes
    }
}

// Tweaked from https://github.com/alexchandel/screenshot-rs/blob/master/src/lib.rs, only with Windows APIs for now

/// An image buffer containing the screenshot.
/// Pixels are stored as [ARGB](https://en.wikipedia.org/wiki/ARGB).
struct ScreenshotData {
	data: Vec<u8>,
	height: usize,
	width: usize
}

impl ScreenshotData {
	/// Height of image in pixels.
	#[inline]
	pub fn height(&self) -> usize { self.height }

	/// Width of image in pixels.
	#[inline]
	pub fn width(&self) -> usize { self.width }
}

impl AsRef<[u8]> for ScreenshotData {
	#[inline]
	fn as_ref<'a>(&'a self) -> &'a [u8] {
		self.data.as_slice()
	}
}

type ScreenResult = Result<ScreenshotData, &'static str>;

// This should definitely be converted to all use windows_sys... but it works.
#[cfg(target_os = "windows")]
mod ffi {
	#![allow(non_snake_case, dead_code)]
	use bytemuck::Contiguous;
use windows_sys::Win32::Graphics::Gdi as W32Gdi;

	use std::collections::VecDeque;

use libc::{c_int, c_uint, c_long, c_void};

	type PVOID = *mut c_void;
	type LPVOID = *mut c_void;
	type WORD = u16; // c_uint;
	type DWORD = u32; // c_ulong;
	type BOOL = c_int;
	type BYTE = u8;
	type UINT = c_uint;
	type LONG = c_long;
	type LPARAM = c_long;

	#[repr(C)]
	struct RECT {
		left: LONG,
		top: LONG,
		right: LONG, // immediately outside rect
		bottom: LONG, // immediately outside rect
	}
	type LPCRECT = *const RECT;
	type LPRECT = *mut RECT;

	type HANDLE = PVOID;
	type HMONITOR = HANDLE;
	type HWND = HANDLE;
	type HDC = HANDLE;
	#[repr(C)]
	struct MONITORINFO {
		cbSize: DWORD,
		rcMonitor: RECT,
		rcWork: RECT,
		dwFlags: DWORD,
	}
	type LPMONITORINFO = *mut MONITORINFO;
	type MONITORENUMPROC = fn(HMONITOR, HDC, LPRECT, LPARAM) -> BOOL;

	type HBITMAP = HANDLE;
	type HGDIOBJ = HANDLE;
	type LPBITMAPINFO = PVOID; // Hack

	const NULL: *mut c_void = 0usize as *mut c_void;
	const HGDI_ERROR: *mut c_void = -1isize as *mut c_void;
	const SM_CXSCREEN: c_int = 0;
	const SM_CYSCREEN: c_int = 1;

	/// Verify value
	const SRCCOPY: u32 = 0x00CC0020;
	const CAPTUREBLT: u32 = 0x40000000;
	const DIB_RGB_COLORS: UINT = 0;
	const BI_RGB: DWORD = 0;

	#[repr(C)]
	struct BITMAPINFOHEADER {
		biSize: DWORD,
		biWidth: LONG,
		biHeight: LONG,
		biPlanes: WORD,
		biBitCount: WORD,
		biCompression: DWORD,
		biSizeImage: DWORD,
		biXPelsPerMeter: LONG,
		biYPelsPerMeter: LONG,
		biClrUsed: DWORD,
		biClrImportant: DWORD,
	}

	#[repr(C)]
	struct RGBQUAD {
		rgbBlue: BYTE,
		rgbGreen: BYTE,
		rgbRed: BYTE,
		rgbReserved: BYTE,
	}

	/// WARNING variable sized struct
	#[repr(C)]
	struct BITMAPINFO {
		bmiHeader: BITMAPINFOHEADER,
		bmiColors: [RGBQUAD; 1],
	}

	#[link(name = "user32")]
	extern "system" {
		fn GetSystemMetrics(m: c_int) -> c_int;
        #[allow(improper_ctypes)]
		fn EnumDisplayMonitors(hdc: HDC, lprcClip: LPCRECT,
							   lpfnEnum: MONITORENUMPROC, dwData: LPARAM) -> BOOL;
		fn GetMonitorInfo(hMonitor: HMONITOR, lpmi: LPMONITORINFO) -> BOOL;
		fn GetDesktopWindow() -> HWND;
		fn GetDC(hWnd: HWND) -> HDC;
	}

	#[link(name = "gdi32")]
	extern "system" {
		fn CreateCompatibleDC(hdc: HDC) -> HDC;
		fn CreateCompatibleBitmap(hdc: HDC, nWidth: c_int, nHeight: c_int) -> HBITMAP;
		fn SelectObject(hdc: HDC, hgdiobj: HGDIOBJ) -> HGDIOBJ;
		fn BitBlt(hdcDest: HDC, nXDest: c_int, nYDest: c_int, nWidth: c_int, nHeight: c_int,
                  hdcSrc: HDC, nXSrc: c_int, nYSrc: c_int, dwRop: DWORD) -> BOOL;
		fn GetDIBits(hdc: HDC, hbmp: HBITMAP, uStartScan: UINT, cScanLines: UINT,
					 lpvBits: LPVOID, lpbi: LPBITMAPINFO, uUsage: UINT) -> c_int;

		fn DeleteObject(hObject: HGDIOBJ) -> BOOL;
		fn ReleaseDC(hWnd: HWND, hDC: HDC) -> c_int;
		fn DeleteDC(hdc: HDC) -> BOOL;
	}

	/// Reorder rows in bitmap, last to first.
	fn flip_rows(data: Vec<u8>, height: usize, row_len: usize) -> Vec<u8> {
		let mut new_data = Vec::with_capacity(data.len());
		unsafe {new_data.set_len(data.len())};
		for row_i in 0..height {
			for byte_i in 0..row_len {
				let old_idx = (height-row_i-1)*row_len + byte_i;
				let new_idx = row_i*row_len + byte_i;
				new_data[new_idx] = data[old_idx];
			}
		}
		new_data
	}

	struct MonitorEnumerationResult {
		width: u32,
		height: u32,

		requested_position: (i32, i32),
		requested_size: (u32, u32),

		data: VecDeque<u8>
	}

	unsafe extern "system" fn monitor_enum_proc(
		hmonitor: W32Gdi::HMONITOR,
		_hdc: W32Gdi::HDC,
		_place: *mut windows_sys::Win32::Foundation::RECT,
		data: windows_sys::Win32::Foundation::LPARAM,
	) -> BOOL {
		// There are probably so many memory safety issues hidden here...

		let data: *mut MonitorEnumerationResult = data as *mut MonitorEnumerationResult;

		let requested_position = (*data).requested_position;
		let requested_size = (*data).requested_size;
		
		let info = get_monitor_info(hmonitor).unwrap();
		let position = (info.monitorInfo.rcMonitor.left, info.monitorInfo.rcMonitor.top);
		let size = (info.monitorInfo.rcMonitor.right - info.monitorInfo.rcMonitor.left, info.monitorInfo.rcMonitor.bottom - info.monitorInfo.rcMonitor.top);
		println!("Monitor: {:?} ({:?}) {:?} ({:?})", position, requested_position, size, requested_size);
		if position == requested_position && size == (requested_size.0 as i32, requested_size.1 as i32) {
			// We found the monitor!
			let desktop_window = GetDesktopWindow();
			let h_wnd_screen = desktop_window;
			let h_dc_screen = _hdc as HDC;

			// Create a Windows Bitmap, and copy the bits into it
			let h_dc = CreateCompatibleDC(h_dc_screen);
			if h_dc == NULL { return false.into(); } // Err("Can't get a Windows display.");

			let h_bmp = CreateCompatibleBitmap(h_dc_screen, size.0, size.1);
			if h_bmp == NULL { return false.into(); } // Err("Can't create a Windows buffer");

			let res = SelectObject(h_dc, h_bmp);
			if res == NULL || res == HGDI_ERROR {
				return false.into(); // Err("Can't select Windows buffer.");
			}

			let res = BitBlt(h_dc, 0, 0, size.0, size.1, h_dc_screen, 0, 0, SRCCOPY|CAPTUREBLT);
			if res == 0 { return false.into(); } // Err("Failed to copy screen to Windows buffer");

			// Get image info
			let pixel_width: usize = 4;
			let mut bmi = BITMAPINFO {
				bmiHeader: BITMAPINFOHEADER {
					biSize: size_of::<BITMAPINFOHEADER>() as DWORD,
					biWidth: size.0 as LONG,
					biHeight: size.1 as LONG,
					biPlanes: 1,
					biBitCount: 8*pixel_width as WORD,
					biCompression: BI_RGB,
					biSizeImage: (size.0 * size.1 * pixel_width as c_int) as DWORD,
					biXPelsPerMeter: 0,
					biYPelsPerMeter: 0,
					biClrUsed: 0,
					biClrImportant: 0,
				},
				bmiColors: [RGBQUAD {
					rgbBlue: 0,
					rgbGreen: 0,
					rgbRed: 0,
					rgbReserved: 0
				}],
			};

			// Create a Vec for image
			let size_bytes: usize = (size.0*size.1) as usize * pixel_width;
			let mut tmp_data: Vec<u8> = Vec::with_capacity(size_bytes);
			tmp_data.set_len(size_bytes);

			// copy bits into Vec
			GetDIBits(h_dc, h_bmp, 0, size.1 as DWORD,
				&mut tmp_data[0] as *mut u8 as *mut c_void,
				&mut bmi as *mut BITMAPINFO as *mut c_void,
				DIB_RGB_COLORS);

			// Release native image buffers
			ReleaseDC(h_wnd_screen, h_dc_screen); // don't need screen anymore
			DeleteDC(h_dc);
			DeleteObject(h_bmp);

			let tmp_data = flip_rows(tmp_data, size.0 as usize, size.1 as usize*pixel_width);

			// Copy image data into the data VecDeque
			let result = &mut *data;
			result.data.extend(tmp_data);
			result.width = size.0 as u32;
			result.height = size.1 as u32;

			return false.into(); // Stop enumeration
		}
		
		return true.into(); // Continue enumeration
	}	

	fn enumerate_monitors(requested_position: (i32, i32), requested_size: (u32, u32)) -> MonitorEnumerationResult {
		let mut data: MonitorEnumerationResult = MonitorEnumerationResult {
			width: 0,
			height: 0,

			requested_position,
			requested_size,

			data: VecDeque::new()
		};
		unsafe {
			W32Gdi::EnumDisplayMonitors(
				0,
				std::ptr::null(),
				Some(monitor_enum_proc),
				&mut data as *mut _ as windows_sys::Win32::Foundation::LPARAM,
			);
		}
		data.into()
	}

	fn get_monitor_info(hmonitor: W32Gdi::HMONITOR) -> Result<W32Gdi::MONITORINFOEXW, std::io::Error> {
		let mut monitor_info: W32Gdi::MONITORINFOEXW = unsafe { std::mem::zeroed() };
		monitor_info.monitorInfo.cbSize = std::mem::size_of::<W32Gdi::MONITORINFOEXW>() as u32;
		let status = unsafe {
			W32Gdi::GetMonitorInfoW(hmonitor, &mut monitor_info as *mut W32Gdi::MONITORINFOEXW as *mut W32Gdi::MONITORINFO)
		};
		if status == false.into() {
			Err(std::io::Error::last_os_error())
		} else {
			Ok(monitor_info)
		}
	}

	/// This may never happen, given the horrific quality of Win32 APIs
	pub fn get_screenshot_from_matching(position: (i32, i32), size: (u32, u32)) -> crate::screenshot::ScreenResult {
		let result: MonitorEnumerationResult = enumerate_monitors(position, size);

		Ok(crate::screenshot::ScreenshotData {
			data: result.data.into(),
			height: result.height as usize,
			width: result.width as usize
		})
	}
}