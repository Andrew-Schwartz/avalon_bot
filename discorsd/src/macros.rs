/// Like format but for cdn urls.
macro_rules! cdn {
    ($fmt:literal, $($args:tt)+) => {
        format!(concat!("https://cdn.discordapp.com/", $fmt), $($args)+)
    };
}

macro_rules! api {
    (VERSION) => { 8 };
    (@priv str $fmt:literal) => {
        concat!("https://discord.com/api/v", api!(VERSION), $fmt)
    };
    ($fmt:literal) => {
        api!(@priv str $fmt).to_string()
    };
    ($fmt:literal, $($args:tt)+) => {
        format!(api!(@priv str $fmt), $($args)+)
    };
}
pub const API_VERSION: u8 = api!(VERSION);

/// Derive `Serialize`, `Deserialize` for bitflags, (de)serializing as if this were an integer.
/// ```rust
/// bitflags! {
///     struct Flags: u8 {
///         const A = 1;
///         const B = 2;
///         const C = 4;
///     }
/// }
/// serde_bitflag!(Flags: u8);
/// ```
macro_rules! serde_bitflag {
    ($bitflag:ty: $repr:ty) => {
        impl serde::ser::Serialize for $bitflag {
            fn serialize<S: serde::ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.bits.serialize(s)
            }
        }

        impl<'de> serde::de::Deserialize<'de> for $bitflag {
            fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let bits = <$repr>::deserialize(d)?;
                Self::from_bits(bits)
                    .ok_or_else(|| serde::de::Error::custom(format!("Unexpected flags value: {}", bits)))
            }
        }
    };
}

/// Same as the `serde_repr` crate but doesn't upset clippy about not using `Self`, and is a normal
/// macro instead of a derive macro.
///
/// Derives `Debug, Clone, Copy, PartialEq, Eq, Hash`.
///
/// Only handles pub enums because that's all I need.
macro_rules! serde_repr {
    (
        $(#[$outer:meta])*
        pub enum $enum_name:ident: $repr:tt {
            $(
                $(#[$inner:meta])*
                $variant:ident = $num:literal
            ),* $(,)?
        }
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr($repr)]
        pub enum $enum_name {
            $(
                $(#[$inner])*
                $variant = $num
            ),*
        }

        impl ::serde::Serialize for $enum_name {
            fn serialize<S: ::serde::ser::Serializer>(&self, s: S) -> ::std::result::Result<S::Ok, S::Error> {
                (*self as $repr).serialize(s)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $enum_name {
            fn deserialize<D: ::serde::de::Deserializer<'de>>(d: D) -> ::std::result::Result<Self, D::Error> {
                use serde::de::{Error, Unexpected};

                match <$repr>::deserialize(d)? {
                    $(
                        $num => ::std::result::Result::Ok(Self::$variant),
                    )*
                    other => ::std::result::Result::Err(D::Error::invalid_value(
                        Unexpected::Unsigned(other as _),
                        &concat!(
                            "one of: ",
                            $(
                                stringify!($variant = $num),
                                ", ",
                            )*
                        )
                    ))
                }
            }
        }
    };
}