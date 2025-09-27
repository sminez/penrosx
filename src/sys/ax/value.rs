use crate::sys::ax::{ax_call, error::AXError};
use core_foundation::{base::CFRange, declare_TCFType, impl_CFTypeDescription, impl_TCFType};
use core_foundation_sys::base::CFTypeID;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use penrose::{Result, custom_error};
use std::{ffi::c_void, fmt, marker::PhantomData};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct AXValueType(pub u32);

impl AXValueType {
    pub const CG_POINT: Self = Self(1);
    pub const CG_SIZE: Self = Self(2);
    pub const CG_RECT: Self = Self(3);
    pub const CF_RANGE: Self = Self(4);
}

impl fmt::Display for AXValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            Self::CG_POINT => "CG point",
            Self::CG_SIZE => "CG size",
            Self::CG_RECT => "CG rect",
            Self::CF_RANGE => "CF range",
            _ => return write!(f, "unknown value type ({})", self.0),
        };

        write!(f, "{s}")
    }
}

#[derive(Debug)]
pub enum __AXValue {}
pub type AXValueRef = *mut __AXValue;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXValueGetTypeID() -> CFTypeID;
    pub fn AXValueCreate(theType: AXValueType, valuePtr: *const c_void) -> AXValueRef;
    pub fn AXValueGetType(value: AXValueRef) -> AXValueType;
    pub fn AXValueGetValue(value: AXValueRef, theType: AXValueType, valuePtr: *mut c_void) -> bool;
}

pub trait AXValueKind {
    const TYPE: AXValueType;
}

impl AXValueKind for CGPoint {
    const TYPE: AXValueType = AXValueType::CG_POINT;
}
impl AXValueKind for CGSize {
    const TYPE: AXValueType = AXValueType::CG_SIZE;
}
impl AXValueKind for CGRect {
    const TYPE: AXValueType = AXValueType::CG_RECT;
}
impl AXValueKind for CFRange {
    const TYPE: AXValueType = AXValueType::CF_RANGE;
}

declare_TCFType!(AXValue<T: AXValueKind>, AXValueRef);
impl_TCFType!(AXValue<T: AXValueKind>, AXValueRef, AXValueGetTypeID);
impl_CFTypeDescription!(AXValue<T: AXValueKind>);

impl<T: AXValueKind> AXValue<T> {
    pub fn new(val: &T) -> Result<Self> {
        let ptr = unsafe { AXValueCreate(T::TYPE, val as *const T as *const c_void) };
        assert!(!ptr.is_null());

        Ok(Self(ptr, PhantomData))
    }

    pub fn value(&self) -> Result<T> {
        unsafe {
            ax_call(
                |x: *mut T| match AXValueGetValue(self.0, T::TYPE, x as *mut _) {
                    true => AXError::SUCCESS,
                    false => AXError::FAILURE,
                },
            )
            .map_err(|_| {
                custom_error!(
                    "unexpected AX type: expected {} got {}",
                    T::TYPE,
                    AXValueGetType(self.0)
                )
            })
        }
    }
}
