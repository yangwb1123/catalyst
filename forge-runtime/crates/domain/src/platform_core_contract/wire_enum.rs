macro_rules! open_wire_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $($variant,)+
            Unknown(String),
        }

        impl $name {
            pub(crate) fn wire_value(&self) -> &str {
                match self {
                    $(Self::$variant => $wire,)+
                    Self::Unknown(value) => value,
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.wire_value())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                $(if value == $wire {
                    return Ok(Self::$variant);
                })+
                Ok(Self::Unknown(value))
            }
        }
    };
}

pub(crate) use open_wire_enum;
