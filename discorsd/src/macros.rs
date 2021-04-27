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