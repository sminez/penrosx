use std::{error, fmt};

/// An OSX accessibility API error result
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct AXError(pub i32);

impl AXError {
    /// Successful execution (no error)
    pub const SUCCESS: Self = Self(0);

    /// Generic failure
    pub const FAILURE: Self = Self(-25200);
    /// Illegal argument
    pub const ILLEGAL_ARGUMENT: Self = Self(-25201);
    /// Invalid UI element
    pub const INVALID_UI_ELEMENT: Self = Self(-25202);
    /// Invalid UI element observer
    pub const INVALID_UI_ELEMENT_OBSERVER: Self = Self(-25203);
    /// Cannot complete
    pub const CANNOT_COMPLETE: Self = Self(-25204);
    /// Attribute unsupported
    pub const ATTRIBUTE_UNSUPPORTED: Self = Self(-25205);
    /// Action unsupported
    pub const ACTION_UNSUPPORTED: Self = Self(-25206);
    /// Notification unsupported
    pub const NOTIFICATION_UNSUPPORTED: Self = Self(-25207);
    /// Not implemented
    pub const NOT_IMPLEMENTED: Self = Self(-25208);
    /// Notification already registered
    pub const NOTIFICATION_ALREADY_REGISTERED: Self = Self(-25209);
    /// Notification not registered
    pub const NOTIFICATION_NOT_REGISTERED: Self = Self(-25210);
    /// API disabled
    pub const API_DISABLED: Self = Self(-25211);
    /// No value
    pub const NO_VALUE: Self = Self(-25212);
    /// Parameterized attribute unsupported
    pub const PARAMETERIZED_ATTRIBUTE_UNSUPPORTED: Self = Self(-25213);
    /// Not enough precision
    pub const NOT_ENOUGH_PRECISION: Self = Self(-25214);
}

impl AXError {
    /// Was this operation successful?
    pub fn is_success(&self) -> bool {
        *self == Self::SUCCESS
    }

    /// Was this operation unsuccessful?
    pub fn is_err(&self) -> bool {
        *self != Self::SUCCESS
    }
}

impl fmt::Display for AXError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            Self::SUCCESS => "success",
            Self::FAILURE => "failure",
            Self::ILLEGAL_ARGUMENT => "illegal argument",
            Self::INVALID_UI_ELEMENT => "invalid ui element",
            Self::INVALID_UI_ELEMENT_OBSERVER => "invalid ui element observer",
            Self::CANNOT_COMPLETE => "invalid ui element observer",
            Self::ATTRIBUTE_UNSUPPORTED => "attribute unsupported",
            Self::NOT_IMPLEMENTED => "not implemented",
            Self::NOTIFICATION_ALREADY_REGISTERED => "notification already registered",
            Self::NOTIFICATION_NOT_REGISTERED => "notification not registered",
            Self::API_DISABLED => "api disabled",
            Self::PARAMETERIZED_ATTRIBUTE_UNSUPPORTED => "parameterized attribute unsupported",
            Self::NOT_ENOUGH_PRECISION => "not enough precision",

            _ => return write!(f, "unknown error ({})", self.0),
        };

        write!(f, "{s}")
    }
}

impl error::Error for AXError {}
