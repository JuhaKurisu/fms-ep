//! JAWT (Java AWT Native Interface) の FFI 定義。
//!
//! AWT のコンポーネントからネイティブの描画面を取り出すための JDK 標準 API。
//! これを使うと、ウィンドウ管理は Java(AWT) に任せたまま、描画だけをネイティブ側が奪える。

#![allow(non_snake_case)]

use jni::sys::{JNIEnv as RawEnv, jboolean, jint, jobject};
use std::os::raw::c_void;

pub const JAWT_VERSION_1_4: jint = 0x0001_0004;
pub const JAWT_VERSION_1_7: jint = 0x0001_0007;
pub const JAWT_VERSION_9: jint = 0x0009_0000;
#[cfg(target_os = "macos")]
pub const JAWT_MACOSX_USE_CALAYER: jint = 0x8000_0000u32 as jint;
pub const JAWT_LOCK_ERROR: jint = 0x0000_0001;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JawtRectangle {
    pub x: jint,
    pub y: jint,
    pub width: jint,
    pub height: jint,
}

#[repr(C)]
pub struct JawtDrawingSurfaceInfo {
    /// OS ごとの実体。macOS では `id<JAWT_SurfaceLayers>`、Windows では
    /// `JAWT_Win32DrawingSurfaceInfo*`、X11 では `JAWT_X11DrawingSurfaceInfo*`。
    pub platformInfo: *mut c_void,
    pub ds: *mut JawtDrawingSurface,
    pub bounds: JawtRectangle,
    pub clipSize: jint,
    pub clip: *mut JawtRectangle,
}

#[repr(C)]
pub struct JawtDrawingSurface {
    pub env: *mut RawEnv,
    pub target: jobject,
    pub Lock: Option<unsafe extern "system" fn(*mut JawtDrawingSurface) -> jint>,
    pub GetDrawingSurfaceInfo:
        Option<unsafe extern "system" fn(*mut JawtDrawingSurface) -> *mut JawtDrawingSurfaceInfo>,
    pub FreeDrawingSurfaceInfo: Option<unsafe extern "system" fn(*mut JawtDrawingSurfaceInfo)>,
    pub Unlock: Option<unsafe extern "system" fn(*mut JawtDrawingSurface)>,
}

#[repr(C)]
pub struct Jawt {
    /// 呼び出し前に希望バージョンを入れる（in/out）
    pub version: jint,
    pub GetDrawingSurface:
        Option<unsafe extern "system" fn(*mut RawEnv, jobject) -> *mut JawtDrawingSurface>,
    pub FreeDrawingSurface: Option<unsafe extern "system" fn(*mut JawtDrawingSurface)>,
    pub Lock: Option<unsafe extern "system" fn(*mut RawEnv)>,
    pub Unlock: Option<unsafe extern "system" fn(*mut RawEnv)>,
    pub GetComponent: Option<unsafe extern "system" fn(*mut RawEnv, *mut c_void) -> jobject>,
    pub CreateEmbeddedFrame:
        Option<unsafe extern "system" fn(*mut RawEnv, *mut c_void) -> jobject>,
    pub SetBounds: Option<unsafe extern "system" fn(*mut RawEnv, jobject, jint, jint, jint, jint)>,
    pub SynthesizeWindowActivation:
        Option<unsafe extern "system" fn(*mut RawEnv, jobject, jboolean)>,
}

unsafe extern "system" {
    pub fn JAWT_GetAWT(env: *mut RawEnv, awt: *mut Jawt) -> jboolean;
}

/// 描画面を lock している間だけ有効なガード。Drop で確実に解放する。
pub struct SurfaceLock {
    awt: Jawt,
    ds: *mut JawtDrawingSurface,
    dsi: *mut JawtDrawingSurfaceInfo,
}

impl SurfaceLock {
    /// AWT コンポーネントの描画面を lock する。
    ///
    /// # Safety
    /// `env` と `component` が有効で、component が表示可能な状態であること。
    pub unsafe fn acquire(env: *mut RawEnv, component: jobject) -> Result<Self, String> {
        unsafe {
            let mut awt: Jawt = std::mem::zeroed();

            // JDK やプラットフォームによって受け付けるバージョンが違うので、新しい順に試す。
            // 実測: JDK 17 / macOS 26 では 9 も 1.7 も拒否され、1.4 だけが通った。
            let mut ok = false;
            let mut tried = Vec::new();
            for v in candidate_versions() {
                awt = std::mem::zeroed();
                awt.version = v;
                if JAWT_GetAWT(env, &mut awt) != 0 {
                    ok = true;
                    break;
                }
                tried.push(format!("{v:#010x}"));
            }
            if !ok {
                return Err(format!(
                    "JAWT_GetAWT がどのバージョンでも失敗した（試した値: {}）。\
                     AWT が初期化されていないか、libjawt が実体に繋がっていない可能性",
                    tried.join(", ")
                ));
            }
            let get_ds = awt
                .GetDrawingSurface
                .ok_or("GetDrawingSurface が null")?;
            let ds = get_ds(env, component);
            if ds.is_null() {
                return Err("GetDrawingSurface が null を返した".into());
            }

            let lock_fn = match (*ds).Lock {
                Some(f) => f,
                None => {
                    free_ds(&awt, ds);
                    return Err("Lock が null".into());
                }
            };
            let lock = lock_fn(ds);
            if lock & JAWT_LOCK_ERROR != 0 {
                free_ds(&awt, ds);
                return Err(format!("描画面の lock に失敗 (code={lock})"));
            }

            let get_dsi = match (*ds).GetDrawingSurfaceInfo {
                Some(f) => f,
                None => {
                    unlock_and_free(&awt, ds);
                    return Err("GetDrawingSurfaceInfo が null".into());
                }
            };
            let dsi = get_dsi(ds);
            if dsi.is_null() {
                unlock_and_free(&awt, ds);
                return Err("GetDrawingSurfaceInfo が null を返した".into());
            }

            Ok(SurfaceLock { awt, ds, dsi })
        }
    }

    /// OS ごとの描画面情報。
    pub fn platform_info(&self) -> *mut c_void {
        unsafe { (*self.dsi).platformInfo }
    }

    /// コンポーネントの現在の大きさ。
    pub fn bounds(&self) -> JawtRectangle {
        unsafe { (*self.dsi).bounds }
    }
}

impl Drop for SurfaceLock {
    fn drop(&mut self) {
        unsafe {
            if let Some(free_dsi) = (*self.ds).FreeDrawingSurfaceInfo {
                free_dsi(self.dsi);
            }
            unlock_and_free(&self.awt, self.ds);
        }
    }
}

fn candidate_versions() -> Vec<jint> {
    #[cfg(target_os = "macos")]
    {
        // macOS は CALayer 経路が必須。実際に通るのは 1.4 だが、将来の JDK で
        // 新しい版が通る可能性があるので新しい順に試す。
        vec![
            JAWT_VERSION_9 | JAWT_MACOSX_USE_CALAYER,
            JAWT_VERSION_1_7 | JAWT_MACOSX_USE_CALAYER,
            JAWT_VERSION_1_4 | JAWT_MACOSX_USE_CALAYER,
            JAWT_VERSION_9,
        ]
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![JAWT_VERSION_9, JAWT_VERSION_1_7, JAWT_VERSION_1_4]
    }
}

unsafe fn unlock_and_free(awt: &Jawt, ds: *mut JawtDrawingSurface) {
    unsafe {
        if let Some(unlock) = (*ds).Unlock {
            unlock(ds);
        }
        free_ds(awt, ds);
    }
}

unsafe fn free_ds(awt: &Jawt, ds: *mut JawtDrawingSurface) {
    unsafe {
        if let Some(free) = awt.FreeDrawingSurface {
            free(ds);
        }
    }
}
