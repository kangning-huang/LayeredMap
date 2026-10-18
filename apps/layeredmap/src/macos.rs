//! Native macOS hooks (Architecture: the one place `unsafe` is allowed, with a SAFETY comment on
//! each block and a second review).
//!
//! Open With and drops on the Dock icon (WP2.1 Import coordinator): macOS sends
//! `application:openURLs:` to the application delegate, which winit owns (`WinitApplicationDelegate`)
//! and which does not implement that method. Before the event loop starts we watch for
//! `NSApplicationWillFinishLaunchingNotification`; by then winit has installed its delegate, and
//! AppKit has not yet delivered the files a launch was asked to open. We add
//! `application:openURLs:` to the delegate's class, and it queues file paths here. The app drains
//! the queue every frame into `Command::Open`, the same pipeline as the Open button, so files that
//! arrive before the window exists are opened once the shell is up.

use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::{Mutex, OnceLock};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
use objc2::{ffi, sel};
use objc2_app_kit::NSApplication;
use objc2_foundation::{MainThreadMarker, NSArray, NSNotification, NSNotificationCenter, NSURL};

/// Paths macOS asked us to open that the app has not taken yet.
static PENDING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// The egui context, once the shell exists, to wake the app when files arrive.
static CONTEXT: OnceLock<egui::Context> = OnceLock::new();

/// Call once on the main thread, before `eframe::run_native`.
pub fn install_open_handler() {
    let Some(_mtm) = MainThreadMarker::new() else {
        log::warn!("open handler: not on the main thread; Open With is unavailable");
        return;
    };
    let block = RcBlock::new(|_note: NonNull<NSNotification>| add_open_urls_method());
    // SAFETY: called on the main thread (checked above). `NSApplicationWillFinishLaunchingNotification`
    // is an immutable AppKit constant that lives for the whole process. The block is `'static` and
    // takes the `NSNotification` pointer AppKit passes, which it ignores. No queue is given, so it
    // runs synchronously on the posting (main) thread. The returned observer token is leaked on
    // purpose: the observation must last until the process exits.
    unsafe {
        let center = NSNotificationCenter::defaultCenter();
        let token = center.addObserverForName_object_queue_usingBlock(
            Some(objc2_app_kit::NSApplicationWillFinishLaunchingNotification),
            None,
            None,
            &block,
        );
        std::mem::forget(token);
    }
}

/// Lets the queue wake the app; call once the shell exists.
pub fn set_context(ctx: &egui::Context) {
    let _ = CONTEXT.set(ctx.clone());
}

/// Files macOS asked us to open since the last call.
pub fn take_opened() -> Vec<PathBuf> {
    std::mem::take(&mut *PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// Adds `application:openURLs:` to the class of the current application delegate (winit's).
fn add_open_urls_method() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: `delegate` is a plain getter, called on the main thread.
    let Some(delegate) = (unsafe { app.delegate() }) else {
        log::warn!("open handler: no application delegate; Open With is unavailable");
        return;
    };
    // SAFETY: every protocol object is an Objective-C object; viewing it as `AnyObject` only
    // erases the protocol (`ProtocolObject` is `#[repr(C)]` over `AnyObject`).
    let object: Retained<AnyObject> = unsafe { Retained::cast(delegate) };
    let class: &AnyClass = object.class();
    let imp: unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut NSArray<NSURL>) = open_urls;
    // SAFETY: `class` is a registered, live Objective-C class. The function pointer has the C ABI
    // and the exact signature that the type encoding "v@:@@" describes (void return; self, _cmd,
    // and two object arguments), which is the signature of
    // `-[NSApplicationDelegate application:openURLs:]`. Casting it to the untyped `IMP` is how the
    // runtime stores every method; it is called back with the original signature. The encoding is a
    // NUL-terminated static string. `class_addMethod` leaves an existing method untouched and
    // returns NO, so winit's own methods are never replaced.
    let added = unsafe {
        let imp: ffi::IMP = Some(std::mem::transmute::<
            unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut NSArray<NSURL>),
            unsafe extern "C" fn(),
        >(imp));
        ffi::class_addMethod(class as *const AnyClass as *mut ffi::objc_class, sel!(application:openURLs:).as_ptr(), imp, c"v@:@@".as_ptr())
    };
    if Bool::from_raw(added).as_bool() {
        log::debug!("open handler: installed on {}", class.name());
    } else {
        log::warn!("open handler: {} already implements application:openURLs:", class.name());
    }
}

/// `-[delegate application:openURLs:]`: queues local file paths and wakes the app.
unsafe extern "C" fn open_urls(_this: *mut AnyObject, _cmd: Sel, _app: *mut AnyObject, urls: *mut NSArray<NSURL>) {
    // SAFETY: AppKit passes a valid (possibly empty) `NSArray<NSURL *>` that outlives this call;
    // `as_ref` handles a null pointer. `isFileURL` and `path` are plain getters.
    let paths: Vec<PathBuf> = unsafe {
        let Some(urls) = urls.as_ref() else { return };
        urls.iter().filter(|u| u.isFileURL()).filter_map(|u| u.path()).map(|p| PathBuf::from(p.to_string())).collect()
    };
    if paths.is_empty() {
        return;
    }
    PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend(paths);
    if let Some(ctx) = CONTEXT.get() {
        ctx.request_repaint();
    }
}

/// Starts macOS Dictation into the focused text field: the same action as Edit › Start
/// Dictation… (`startDictation:`), sent up the responder chain. The recognised text arrives
/// through the window's text input like typing (the Ask box, WP7.1).
pub fn start_dictation() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: on the main thread (checked above). `sendAction:to:from:` is NSApplication's public
    // method; `startDictation:` is AppKit's standard action selector; a nil target sends it to the
    // first responder (then the application), and a nil sender is allowed. It returns whether
    // some responder handled it, which is only logged.
    let handled: Bool = unsafe {
        objc2::msg_send![&app, sendAction: sel!(startDictation:), to: std::ptr::null::<AnyObject>(), from: std::ptr::null::<AnyObject>()]
    };
    if !handled.as_bool() {
        log::info!("dictation: no responder took startDictation:");
    }
}
