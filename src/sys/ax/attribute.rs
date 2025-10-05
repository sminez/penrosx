use crate::sys::ax::{ui_element::AXUIElement, value::AXValue};
use core_foundation::{array::CFArray, base::CFType, boolean::CFBoolean, string::CFString};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use penrose::Result;
use std::marker::PhantomData;

// See https://github.com/eiz/accessibility/blob/master/accessibility-sys/src/attribute_constants.rs
// for a full list
pub const AX_ALLOWED_VALUES: &str = "AXAllowedValues";
pub const AX_CHILDREN: &str = "AXChildren";
pub const AX_CLOSE_BUTTON: &str = "AXCloseButton";
pub const AX_CONTENTS: &str = "AXContents";
pub const AX_DESCRIPTION: &str = "AXDescription";
pub const AX_ELEMENT_BUSY: &str = "AXElementBusy";
pub const AX_ENABLED: &str = "AXEnabled";
pub const AX_ENHANCED_USER_INTERFACE: &str = "AXEnhancedUserInterface";
pub const AX_FOCUSED_WINDOW: &str = "AXFocusedWindow";
pub const AX_FOCUSED: &str = "AXFocused";
pub const AX_FRAME: &str = "AXFrame";
pub const AX_FRONTMOST: &str = "AXFrontmost";
pub const AX_FULL_SCREEN: &str = "AXFullScreen";
pub const AX_HELP: &str = "AXHelp";
pub const AX_IDENTIFIER: &str = "AXIdentifier";
pub const AX_LABEL_VALUE: &str = "AXLabelValue";
pub const AX_MAIN_WINDOW: &str = "AXMainWindow";
pub const AX_MAIN: &str = "AXMain";
pub const AX_MAX_VALUE: &str = "AXMaxValue";
pub const AX_MIN_VALUE: &str = "AXMinValue";
pub const AX_MINIMIZED: &str = "AXMinimized";
pub const AX_PARENT: &str = "AXParent";
pub const AX_PLACEHOLDER_VALUE: &str = "AXPlaceholderValue";
pub const AX_POSITION: &str = "AXPosition";
pub const AX_ROLE_DESCRIPTION: &str = "AXRoleDescription";
pub const AX_ROLE: &str = "AXRole";
pub const AX_SELECTED_CHILDREN: &str = "AXSelectedChildren";
pub const AX_SIZE: &str = "AXSize";
pub const AX_SUBROLE: &str = "AXSubrole";
pub const AX_TITLE_UI_ELEMENT: &str = "AXTitleUIElement";
pub const AX_TITLE: &str = "AXTitle";
pub const AX_TOP_LEVEL_UI_ELEMENT: &str = "AXTopLevelUIElement";
pub const AX_VALUE_DESCRIPTION: &str = "AXValueDescription";
pub const AX_VALUE_INCREMENT: &str = "AXValueIncrement";
pub const AX_VALUE: &str = "AXValue";
pub const AX_VISIBLE_CHILDREN: &str = "AXVisibleChildren";
pub const AX_WINDOW: &str = "AXWindow";
pub const AX_WINDOWS: &str = "AXWindows";

#[derive(Clone, Debug)]
pub struct AXAttribute<T>(CFString, PhantomData<*const T>);

impl<T> AXAttribute<T> {
    pub fn as_cf_string(&self) -> &CFString {
        &self.0
    }
}

impl AXAttribute<CFType> {
    pub fn new(name: &CFString) -> Self {
        AXAttribute(name.to_owned(), PhantomData)
    }
}

macro_rules! constructor {
    ($name:ident, $ty:ty, $const:ident $(,$setter:ident)?) => {
        pub fn $name() -> AXAttribute<$ty> {
            AXAttribute(CFString::from_static_string($const), PhantomData)
        }
    };
}

macro_rules! define_elem_attr_method {
    ($name:ident, AXValue<$ty:ty>, $const:ident $(, $setter:ident)?) => {
        fn $name(&self) -> Result<$ty>;
        $(fn $setter(&self, value: impl Into<$ty>) -> Result<()>;)?
    };
    ($name:ident, $ty:ty, $const:ident $(, $setter:ident)?) => {
        fn $name(&self) -> Result<$ty>;
        $(fn $setter(&self, value: impl Into<$ty>) -> Result<()>;)?
    };
}

macro_rules! impl_elem_attr_method {
    ($name:ident, AXValue<$ty:ty>, $const:ident $(, $setter:ident)?) => {
        fn $name(&self) -> Result<$ty> {
            self.attribute(&AXAttribute::$name()).and_then(|v| v.value())
        }
        $(fn $setter(&self, value: impl Into<$ty>) -> Result<()> {
            self.set_attribute(&AXAttribute::$name(), AXValue::new(&value.into())?)
        })?
    };
    ($name:ident, $ty:ty, $const:ident $(, $setter:ident)?) => {
        fn $name(&self) -> Result<$ty> {
            self.attribute(&AXAttribute::$name())
        }
        $(fn $setter(&self, value: impl Into<$ty>) -> Result<()> {
            self.set_attribute(&AXAttribute::$name(), value)
        })?
    };
}

macro_rules! define_attributes {
    ($(($($args:tt)*)),*,) => {
        impl AXAttribute<()> {
            $(constructor!($($args)*);)*
        }

        #[allow(missing_docs)]
        pub trait AXUIElementAttributes {
            $(define_elem_attr_method!($($args)*);)*
        }

        impl AXUIElementAttributes for AXUIElement {
            $(impl_elem_attr_method!($($args)*);)*
        }
    }
}

define_attributes! {
    (allowed_values, CFArray<CFType>, AX_ALLOWED_VALUES),
    (children, CFArray<AXUIElement>, AX_CHILDREN),
    (close_button, AXUIElement, AX_CLOSE_BUTTON),
    (contents, AXUIElement, AX_CONTENTS),
    (description, CFString, AX_DESCRIPTION),
    (element_busy, CFBoolean, AX_ELEMENT_BUSY),
    (enabled, CFBoolean, AX_ENABLED),
    (enhanced_user_interface, CFBoolean, AX_ENHANCED_USER_INTERFACE, set_enhanced_user_interface),
    (focused_window, AXUIElement, AX_FOCUSED_WINDOW),
    (focused, CFBoolean, AX_FOCUSED),
    (frame, AXValue<CGRect>, AX_FRAME),
    (frontmost, CFBoolean, AX_FRONTMOST, set_frontmost),
    (fullscreen, CFBoolean, AX_FULL_SCREEN, set_fullscreen),
    (help, CFString, AX_HELP),
    (identifier, CFString, AX_IDENTIFIER),
    (label_value, CFString, AX_LABEL_VALUE),
    (main_window, AXUIElement, AX_MAIN_WINDOW),
    (main, CFBoolean, AX_MAIN, set_main),
    (max_value, CFType, AX_MAX_VALUE),
    (min_value, CFType, AX_MIN_VALUE),
    (minimized, CFBoolean, AX_MINIMIZED, set_minimized),
    (parent, AXUIElement, AX_PARENT),
    (placeholder_value, CFString, AX_PLACEHOLDER_VALUE),
    (position, AXValue<CGPoint>, AX_POSITION, set_position),
    (role_description, CFString, AX_ROLE_DESCRIPTION),
    (role, CFString, AX_ROLE),
    (selected_children, CFArray<AXUIElement>, AX_SELECTED_CHILDREN),
    (size, AXValue<CGSize>, AX_SIZE, set_size),
    (subrole, CFString, AX_SUBROLE),
    (title_ui_element, AXUIElement, AX_TITLE_UI_ELEMENT),
    (title, CFString, AX_TITLE),
    (top_level_ui_element, AXUIElement, AX_TOP_LEVEL_UI_ELEMENT),
    (value_description, CFString, AX_VALUE_DESCRIPTION),
    (value_increment, CFType, AX_VALUE_INCREMENT),
    (value, CFType, AX_VALUE, set_value),
    (visible_children, CFArray<AXUIElement>, AX_VISIBLE_CHILDREN),
    (window, AXUIElement, AX_WINDOW),
    (windows, CFArray<AXUIElement>, AX_WINDOWS),
}
