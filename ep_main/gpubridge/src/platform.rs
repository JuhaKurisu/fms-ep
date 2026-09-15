//! AWT の描画面から wgpu の Surface を作る、OS 依存の部分。
//!
//! macOS: JAWT は CALayer 経路しか提供しないので、CAMetalLayer を作って
//! コンポーネントに差し込み、それを wgpu に渡す。
//! Windows / Linux: JAWT が返す HWND / X11 Window をそのまま渡す。

use crate::jawt::SurfaceLock;
use jni::objects::GlobalRef;
use jni::sys::{JNIEnv as RawEnv, jobject};
use jni::{JavaVM, JNIEnv};

/// 描画面を生かしておくために保持しておくもの（macOS の CAMetalLayer など）。
pub struct LayerKeepAlive {
    #[cfg(target_os = "macos")]
    _layer: objc2::rc::Retained<objc2_quartz_core::CAMetalLayer>,
}

// CAMetalLayer は Send/Sync ではないが、レイヤへの操作（attach / resize）は必ず
// AppKit のメインスレッドへ回してから行う。Objective-C の retain/release 自体は
// スレッドセーフなので、保持と drop に限れば他スレッドへ渡しても問題ない。
unsafe impl Send for LayerKeepAlive {}
unsafe impl Sync for LayerKeepAlive {}

/// attach を「その OS が UI 操作を要求するスレッド」で実行する。
///
/// macOS では CALayer の差し替えが AppKit のメインスレッドを要求する。Processing の
/// Animation Thread から直接呼ぶと AppKit 側が main queue を待ち、EDT との間で
/// 循環待ちになって JVM ごと固まるため、main queue に積んでから結果を受け取る。
///
/// `vm` と `component`（グローバル参照）を使うのは、実行スレッドが変わるため。
/// JNIEnv はスレッド固有なので他スレッドへ持ち込めず、ローカル参照も同様。
pub fn attach_on_ui_thread(
    vm: &JavaVM,
    component: &GlobalRef,
    scale: f32,
    px_w: u32,
    px_h: u32,
    instance: &wgpu::Instance,
) -> Result<(wgpu::Surface<'static>, u32, u32, LayerKeepAlive), String> {
    #[cfg(target_os = "macos")]
    {
        use dispatch2::DispatchQueue;

        if objc2::MainThreadMarker::new().is_some() {
            return attach_here(vm, component, scale, px_w, px_h, instance);
        }

        let mut result: Option<Result<(wgpu::Surface<'static>, u32, u32, LayerKeepAlive), String>> =
            None;
        let slot = &mut result;

        DispatchQueue::main().exec_sync(move || {
            *slot = Some(attach_here(vm, component, scale, px_w, px_h, instance));
        });

        result.unwrap_or_else(|| Err("main queue で attach が実行されなかった".into()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        attach_here(vm, component, scale, px_w, px_h, instance)
    }
}

/// 今のスレッドを JVM に紐づけてから attach する。
fn attach_here(
    vm: &JavaVM,
    component: &GlobalRef,
    scale: f32,
    px_w: u32,
    px_h: u32,
    instance: &wgpu::Instance,
) -> Result<(wgpu::Surface<'static>, u32, u32, LayerKeepAlive), String> {
    // AppKit のメインスレッドは JVM から見ると未知のスレッドのことがある。
    // 既に紐づいていれば既存の JNIEnv がそのまま返る。
    let mut guard = vm
        .attach_current_thread()
        .map_err(|e| format!("このスレッドを JVM に紐づけられない: {e}"))?;
    let env: &mut JNIEnv = &mut guard;
    unsafe { attach(env.get_raw(), component.as_raw(), scale, px_w, px_h, instance) }
}

// ---------------------------------------------------------------- macOS

#[cfg(target_os = "macos")]
unsafe fn attach(
    env: *mut RawEnv,
    component: jobject,
    scale: f32,
    px_w: u32,
    px_h: u32,
    instance: &wgpu::Instance,
) -> Result<(wgpu::Surface<'static>, u32, u32, LayerKeepAlive), String> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_quartz_core::CAMetalLayer;

    let lock = unsafe { SurfaceLock::acquire(env, component)? };
    let bounds = lock.bounds();
    if bounds.width <= 0 || bounds.height <= 0 {
        return Err(format!(
            "コンポーネントの大きさが不正: {}x{}（まだ表示されていない可能性）",
            bounds.width, bounds.height
        ));
    }

    let surface_layers = lock.platform_info() as *mut AnyObject;
    if surface_layers.is_null() {
        return Err("platformInfo が null（JAWT_MACOSX_USE_CALAYER が効いていない）".into());
    }

    // drawable は Java 側が指定した物理ピクセルにちょうど合わせる
    // （窓・テクスチャ・マウスを同一座標系にするため。丸め由来の 1px ズレも出さない）
    let scale = if scale > 0.0 { scale as f64 } else { 1.0 };
    let layer = CAMetalLayer::new();
    layer.setFrame(CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(bounds.width as f64, bounds.height as f64),
    ));
    layer.setDrawableSize(CGSize::new(px_w as f64, px_h as f64));
    layer.setContentsScale(scale);
    // compute で書いた結果を blit するだけなので読み戻しは不要
    layer.setFramebufferOnly(true);

    // JAWT_SurfaceLayers プロトコルの layer プロパティに差し込む
    let layer_ptr: *const CAMetalLayer = &*layer;
    unsafe {
        let _: () = msg_send![surface_layers, setLayer: layer_ptr];
    }

    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                layer_ptr as *mut std::ffi::c_void,
            ))
            .map_err(|e| format!("CAMetalLayer から surface を作れない: {e}"))?
    };

    // lock はここで解放される（drop）
    drop(lock);

    Ok((surface, px_w, px_h, LayerKeepAlive { _layer: layer }))
}

// ---------------------------------------------------------------- Windows

#[cfg(target_os = "windows")]
#[repr(C)]
struct JawtWin32DrawingSurfaceInfo {
    /// hwnd / hbitmap / pbits の union の先頭
    handle: *mut std::ffi::c_void,
    hdc: *mut std::ffi::c_void,
    hpalette: *mut std::ffi::c_void,
}

/// 未検証。Windows 実機で確認していない。
#[cfg(target_os = "windows")]
unsafe fn attach(
    env: *mut RawEnv,
    component: jobject,
    _scale: f32,
    px_w: u32,
    px_h: u32,
    instance: &wgpu::Instance,
) -> Result<(wgpu::Surface<'static>, u32, u32, LayerKeepAlive), String> {
    use raw_window_handle::{
        RawDisplayHandle, RawWindowHandle, WindowsDisplayHandle, Win32WindowHandle,
    };
    use std::num::NonZeroIsize;

    let lock = unsafe { SurfaceLock::acquire(env, component)? };
    let bounds = lock.bounds();
    if bounds.width <= 0 || bounds.height <= 0 {
        return Err(format!(
            "コンポーネントの大きさが不正: {}x{}",
            bounds.width, bounds.height
        ));
    }

    let info = lock.platform_info() as *const JawtWin32DrawingSurfaceInfo;
    if info.is_null() {
        return Err("platformInfo が null".into());
    }
    let hwnd = unsafe { (*info).handle } as isize;
    let hwnd = NonZeroIsize::new(hwnd).ok_or("HWND が null")?;

    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(RawDisplayHandle::Windows(WindowsDisplayHandle::new())),
                raw_window_handle: RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
            })
            .map_err(|e| format!("HWND から surface を作れない: {e}"))?
    };
    drop(lock);
    Ok((surface, px_w, px_h, LayerKeepAlive {}))
}

// ---------------------------------------------------------------- Linux (X11)

#[cfg(target_os = "linux")]
#[repr(C)]
struct JawtX11DrawingSurfaceInfo {
    drawable: std::os::raw::c_ulong,
    display: *mut std::ffi::c_void,
    visualID: std::os::raw::c_ulong,
    colormapID: std::os::raw::c_ulong,
    depth: std::os::raw::c_int,
}

/// 未検証。Linux 実機で確認していない。X11 のみ（Wayland では JAWT が X11 情報を返さない）。
#[cfg(target_os = "linux")]
unsafe fn attach(
    env: *mut RawEnv,
    component: jobject,
    _scale: f32,
    px_w: u32,
    px_h: u32,
    instance: &wgpu::Instance,
) -> Result<(wgpu::Surface<'static>, u32, u32, LayerKeepAlive), String> {
    use raw_window_handle::{RawDisplayHandle, RawWindowHandle, XlibDisplayHandle, XlibWindowHandle};
    use std::ptr::NonNull;

    let lock = unsafe { SurfaceLock::acquire(env, component)? };
    let bounds = lock.bounds();
    if bounds.width <= 0 || bounds.height <= 0 {
        return Err(format!(
            "コンポーネントの大きさが不正: {}x{}",
            bounds.width, bounds.height
        ));
    }

    let info = lock.platform_info() as *const JawtX11DrawingSurfaceInfo;
    if info.is_null() {
        return Err("platformInfo が null".into());
    }
    let (drawable, display, visual) =
        unsafe { ((*info).drawable, (*info).display, (*info).visualID) };
    if drawable == 0 {
        return Err("X11 drawable が 0".into());
    }

    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                    NonNull::new(display),
                    0,
                )),
                raw_window_handle: RawWindowHandle::Xlib(XlibWindowHandle::new(
                    drawable,
                    visual as u32 as _,
                )),
            })
            .map_err(|e| format!("X11 drawable から surface を作れない: {e}"))?
    };
    drop(lock);
    Ok((surface, px_w, px_h, LayerKeepAlive {}))
}

/// ウィンドウリサイズ時に CAMetalLayer の大きさを追従させる（macOS のみ実処理）。
///
/// attach と同じく AppKit のメインスレッドで実行する。呼び出し元は
/// Animation Thread であること（EDT から呼ぶと循環待ちの危険がある）。
#[cfg(target_os = "macos")]
pub fn resize_layer(keep: &LayerKeepAlive, scale: f32, px_w: u32, px_h: u32) {
    use dispatch2::DispatchQueue;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};

    let scale = if scale > 0.0 { scale as f64 } else { 1.0 };
    // Retained は Send ではないのでアドレスで渡す。keep が生きている間 layer は有効で、
    // exec_sync は完了を待つのでライフタイム上の問題はない。
    let layer_addr = (&*keep._layer) as *const objc2_quartz_core::CAMetalLayer as usize;

    let work = move || {
        let layer = unsafe { &*(layer_addr as *const objc2_quartz_core::CAMetalLayer) };
        layer.setFrame(CGRect::new(
            CGPoint::new(0.0, 0.0),
            CGSize::new(px_w as f64 / scale, px_h as f64 / scale),
        ));
        layer.setDrawableSize(CGSize::new(px_w as f64, px_h as f64));
    };
    if objc2::MainThreadMarker::new().is_some() {
        work();
    } else {
        DispatchQueue::main().exec_sync(work);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn resize_layer(_keep: &LayerKeepAlive, _scale: f32, _px_w: u32, _px_h: u32) {}
