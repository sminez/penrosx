//! Queries against client windows
use crate::{AXUIElementAttributes, conn::OsxConn};
use penrose::{Result, WinId, core::conn::Query};

macro_rules! define_string_queries {
    ( $(($($args:tt)*)),*, ) => {
        $(define_string_queries!($($args)*);)*
    };

    ($(#[$docs:meta])* @win $name:ident, $method:ident) => {
        $(#[$docs])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub struct $name(pub &'static str);
        impl Query<OsxConn> for $name {
            fn run(&self, id: WinId, conn: &mut OsxConn) -> Result<bool> {
                Ok(conn.window(id)?.axwin.$method()? == self.0)
            }
        }
    };

    ($(#[$docs:meta])* @app $name:ident, $method:ident) => {
        $(#[$docs])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub struct $name(pub &'static str);
        impl Query<OsxConn> for $name {
            fn run(&self, id: WinId, conn: &mut OsxConn) -> Result<bool> {
                Ok(conn.app_for_window(id)?.axapp.$method()? == self.0)
            }
        }
    };
}

define_string_queries!(
    (/// A [Query] for matching a window's title
    @win Title, title),
    (/// A [Query] for matching a window's role
    @win Role, role),
    (/// A [Query] for matching a window's subrole
    @win SubRole, subrole),

    (/// A [Query] for matching a window's application name
    @app AppName, title),
);
