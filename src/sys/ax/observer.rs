use crate::sys::{
    Pid,
    ax::{
        error::AXError,
        ui_element::{AXUIElement, AXUIElementRef},
    },
};
use core_foundation::{
    base::TCFType,
    declare_TCFType, impl_CFTypeDescription, impl_TCFType,
    runloop::{CFRunLoopAddSource, CFRunLoopGetCurrent, kCFRunLoopCommonModes},
    string::CFString,
};
use core_foundation_sys::{base::CFTypeID, runloop::CFRunLoopSourceRef, string::CFStringRef};
use penrose::{Result, custom_error};
use std::{borrow::Cow, ffi::c_void, mem::ManuallyDrop, ptr};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXObserverCreate(
        application: Pid,
        callback: AXObserverCallback,
        outObserver: *mut AXObserverRef,
    ) -> AXError;
    pub fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFStringRef,
        refcon: *mut c_void,
    ) -> AXError;
    pub fn AXObserverGetTypeID() -> CFTypeID;
    pub fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFRunLoopSourceRef;
}

pub type AXObserverCallback = unsafe extern "C" fn(
    observer: AXObserverRef,
    element: AXUIElementRef,
    notification: CFStringRef,
    refcon: *mut c_void,
);

#[derive(Debug)]
pub enum __AXObserver {}
pub type AXObserverRef = *mut __AXObserver;

declare_TCFType!(AXObserver, AXObserverRef);
impl_TCFType!(AXObserver, AXObserverRef, AXObserverGetTypeID);
impl_CFTypeDescription!(AXObserver);

#[derive(Debug)]
pub struct Observer {
    callback: *mut (),
    destructor: fn(*mut ()),
    observer: ManuallyDrop<AXObserver>,
}

fn destruct<F>(ptr: *mut ()) {
    let _ = unsafe { Box::from_raw(ptr as *mut F) };
}

impl Drop for Observer {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.observer);
            (self.destructor)(self.callback);
        }
    }
}

impl Observer {
    pub fn try_new<F: Fn(&str) + 'static>(pid: Pid, callback: F) -> Result<Observer> {
        let mut observer: AXObserverRef = ptr::null_mut();
        unsafe {
            let res = AXObserverCreate(pid, observer_callback::<F>, &mut observer);
            if res.is_err() {
                return Err(custom_error!("failed to create observer: {}", res));
            }

            let source = AXObserverGetRunLoopSource(observer);
            CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
        }

        Ok(Observer {
            callback: Box::into_raw(Box::new(callback)) as *mut (),
            destructor: destruct::<F>,
            observer: ManuallyDrop::new(unsafe { AXObserver::wrap_under_create_rule(observer) }),
        })
    }

    pub fn add_notification(&self, elem: &AXUIElement, notif: &'static str) -> Result<()> {
        let res = unsafe {
            AXObserverAddNotification(
                self.observer.as_concrete_TypeRef(),
                elem.as_concrete_TypeRef(),
                CFString::from_static_string(notif).as_concrete_TypeRef(),
                self.callback as *mut c_void,
            )
        };
        if res.is_err() {
            return Err(custom_error!("failed to add notification: {}", res));
        }

        Ok(())
    }
}

unsafe extern "C" fn observer_callback<F: Fn(&str) + 'static>(
    _obs: AXObserverRef,
    _elem: AXUIElementRef,
    notif: CFStringRef,
    data: *mut c_void,
) {
    let callback = unsafe { &*(data as *const F) };
    let notif = unsafe { CFString::wrap_under_get_rule(notif) };
    let notif = Cow::<str>::from(&notif);

    callback(&notif);
}
