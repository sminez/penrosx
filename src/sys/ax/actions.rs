use crate::sys::ax::ui_element::AXUIElement;
use penrose::Result;

pub const AX_CANCEL: &str = "AXCancel";
pub const AX_CONFIRM: &str = "AXConfirm";
pub const AX_DECREMENT: &str = "AXDecrement";
pub const AX_INCREMENT: &str = "AXIncrement";
pub const AX_PICK: &str = "AXPick";
pub const AX_PRESS: &str = "AXPress";
pub const AX_RAISE: &str = "AXRaise";
pub const AX_SHOW_ALTERNATE_UI: &str = "AXShowAlternateUI";
pub const AX_SHOW_DEFAULT_UI: &str = "AXShowDefaultUI";
pub const AX_SHOW_MENU: &str = "AXShowMenu";

macro_rules! define_actions {
    ($(($name:ident, $const:ident)),*,) => {
        #[allow(missing_docs)]
        pub trait AXUIElementActions {
            $(fn $name(&self) -> Result<()>;)*
        }

        impl AXUIElementActions for AXUIElement {
            $(fn $name(&self) -> Result<()> { self.perform_action($const) })*
        }
    }
}

define_actions! {
    (cancel, AX_CANCEL),
    (confirm, AX_CONFIRM),
    (decrement, AX_DECREMENT),
    (increment, AX_INCREMENT),
    (pick, AX_PICK),
    (press, AX_PRESS),
    (raise, AX_RAISE),
    (show_alternate_ui, AX_SHOW_ALTERNATE_UI),
    (show_default_ui, AX_SHOW_DEFAULT_UI),
    (show_menu, AX_SHOW_MENU),
}
