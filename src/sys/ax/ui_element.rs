use crate::{
    Pid,
    sys::ax::{attribute::AXAttribute, ax_call, ax_call_void, error::AXError},
};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType, TCFTypeRef},
    declare_TCFType, impl_CFTypeDescription, impl_TCFType,
    string::CFString,
};
use core_foundation_sys::{
    array::CFArrayRef,
    base::{CFTypeID, CFTypeRef},
    runloop::CFRunLoopSourceRef,
    string::CFStringRef,
};
use objc2_app_kit::NSRunningApplication;
use objc2_foundation::NSString;
use penrose::{Result, custom_error};
use std::{
    ffi::{c_uchar, c_void},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum __AXUIElement {}
pub type AXUIElementRef = *mut __AXUIElement;

#[derive(Debug)]
pub enum __AXObserver {}
pub type AXObserverRef = *mut __AXObserver;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub static kAXTrustedCheckOptionPrompt: *const c_void;

    pub fn AXIsProcessTrusted() -> bool;
    pub fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    pub fn AXUIElementGetTypeID() -> CFTypeID;
    pub fn AXUIElementCopyAttributeNames(
        element: AXUIElementRef,
        names: *mut CFArrayRef,
    ) -> AXError;
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut c_uchar,
    ) -> AXError;
    pub fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementCopyActionNames(element: AXUIElementRef, names: *mut CFArrayRef) -> AXError;
    pub fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;
    pub fn AXUIElementCreateApplication(pid: Pid) -> AXUIElementRef;
    pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub fn AXUIElementSetMessagingTimeout(
        element: AXUIElementRef,
        timeoutInSeconds: f32,
    ) -> AXError;
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
    pub fn AXObserverRemoveNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFStringRef,
    ) -> AXError;
    pub fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFRunLoopSourceRef;
}

pub type AXObserverCallback = unsafe extern "C" fn(
    observer: AXObserverRef,
    element: AXUIElementRef,
    notification: CFStringRef,
    refcon: *mut c_void,
);

declare_TCFType!(AXUIElement, AXUIElementRef);
impl_TCFType!(AXUIElement, AXUIElementRef, AXUIElementGetTypeID);
impl_CFTypeDescription!(AXUIElement);

impl AXUIElement {
    pub fn system_wide() -> Self {
        unsafe { Self::wrap_under_create_rule(AXUIElementCreateSystemWide()) }
    }

    pub fn application(pid: Pid) -> Self {
        unsafe { Self::wrap_under_create_rule(AXUIElementCreateApplication(pid)) }
    }

    pub fn application_with_bundle(bundle_id: &str) -> Result<Self> {
        unsafe {
            let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(
                &NSString::from_str(bundle_id),
            );

            if let Some(app) = apps.iter().next() {
                Ok(Self::wrap_under_create_rule(AXUIElementCreateApplication(
                    app.processIdentifier(),
                )))
            } else {
                Err(custom_error!("not found"))
            }
        }
    }

    pub fn application_with_bundle_timeout(bundle_id: &str, timeout: Duration) -> Result<Self> {
        let deadline = Instant::now() + timeout;

        loop {
            match Self::application_with_bundle(bundle_id) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let now = Instant::now();

                    if now >= deadline {
                        return Err(e);
                    } else {
                        let time_left = deadline.saturating_duration_since(now);
                        thread::sleep(std::cmp::min(time_left, Duration::from_millis(250)));
                    }
                }
            }
        }
    }

    pub fn attribute_names(&self) -> Result<CFArray<CFString>> {
        unsafe {
            Ok(CFArray::wrap_under_create_rule(ax_call(|x| {
                AXUIElementCopyAttributeNames(self.0, x)
            })?))
        }
    }

    pub fn attribute<T: TCFType>(&self, attribute: &AXAttribute<T>) -> Result<T> {
        let res = unsafe {
            Ok(T::wrap_under_create_rule(T::Ref::from_void_ptr(ax_call(
                |x| {
                    AXUIElementCopyAttributeValue(
                        self.0,
                        attribute.as_cf_string().as_concrete_TypeRef(),
                        x,
                    )
                },
            )?)))
        };

        if let Ok(val) = &res
            && T::type_id() != CFType::type_id()
            && !val.instance_of::<T>()
        {
            return Err(custom_error!(
                "unexpected AX type (expected {} got {})",
                T::type_id(),
                val.type_of()
            ));
        }

        res
    }

    pub fn set_attribute<T: TCFType>(
        &self,
        attribute: &AXAttribute<T>,
        value: impl Into<T>,
    ) -> Result<()> {
        let value = value.into();

        unsafe {
            ax_call_void(|| {
                AXUIElementSetAttributeValue(
                    self.0,
                    attribute.as_cf_string().as_concrete_TypeRef(),
                    value.as_CFTypeRef(),
                )
            })
        }
    }

    pub fn is_settable<T: TCFType>(&self, attribute: &AXAttribute<T>) -> Result<bool> {
        let settable: c_uchar = unsafe {
            ax_call(|x| {
                AXUIElementIsAttributeSettable(
                    self.0,
                    attribute.as_cf_string().as_concrete_TypeRef(),
                    x,
                )
            })?
        };

        Ok(settable != 0)
    }

    pub fn action_names(&self) -> Result<CFArray<CFString>> {
        unsafe {
            Ok(CFArray::wrap_under_create_rule(ax_call(|x| {
                AXUIElementCopyActionNames(self.0, x)
            })?))
        }
    }

    pub fn perform_action(&self, name: &str) -> Result<()> {
        unsafe {
            ax_call_void(|| {
                AXUIElementPerformAction(self.0, CFString::new(name).as_concrete_TypeRef())
            })
        }
    }

    pub fn set_messaging_timeout(&self, timeout: f32) -> Result<()> {
        unsafe { ax_call_void(|| AXUIElementSetMessagingTimeout(self.0, timeout))? };

        Ok(())
    }
}
