/// like format but for cdn urls
macro_rules! cdn {
    ($fmt:literal, $($args:tt)+) => {
        format!(concat!("https://cdn.discordapp.com/", $fmt), $($args)+)
    };
}

pub const API_VERSION: u8 = 8;
macro_rules! api {
    ($fmt:literal) => {
        concat!("https://discordapp.com/api/v8", $fmt).to_string()
    };
    ($fmt:literal, $($args:tt)+) => {
        format!(concat!("https://discordapp.com/api/v8", $fmt), $($args)+)
    };
}

/// derive `Serialize`, `Deserialize` for bitflags
macro_rules! serde_bitflag {
    ($bitflag:ty, $repr:ty) => {
        impl serde::ser::Serialize for $bitflag {
            fn serialize<S: serde::ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.bits.serialize(s)
            }
        }

        impl<'de> serde::de::Deserialize<'de> for $bitflag {
            fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let bits = <$repr>::deserialize(d)?;
                <$bitflag>::from_bits(bits)
                    .ok_or(serde::de::Error::custom(format!("Unexpected flags value: {}", bits)))
            }
        }
    };
}