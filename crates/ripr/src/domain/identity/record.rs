//! Registry row types. The catalog records existing authorities; it does not
//! wrap them again.

use super::kinds::{IdentityKind, PortabilityClass};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldVisibility {
    Public,
    Private,
}

impl FieldVisibility {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldRole {
    Canonical,
    CompatibilityAlias,
    Component,
}

impl FieldRole {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::CompatibilityAlias => "compatibility_alias",
            Self::Component => "component",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SerializationField {
    pub name: &'static str,
    pub role: FieldRole,
    pub surface: &'static str,
    pub visibility: FieldVisibility,
    pub removal_generation: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdentityRecord {
    pub kind: IdentityKind,
    pub canonical_type: &'static str,
    pub owner_path: &'static str,
    pub owner_issue: &'static str,
    pub portability: PortabilityClass,
    pub semantic_inputs: &'static [&'static str],
    pub volatile_excluded: &'static [&'static str],
    pub parents: &'static [IdentityKind],
    pub children: &'static [IdentityKind],
    pub invalidation: &'static str,
    pub persistence: &'static str,
    pub serialization: &'static [SerializationField],
    pub competing_wrappers: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdjacentField {
    pub name: &'static str,
    pub surfaces: &'static [&'static str],
    pub reason: &'static str,
}

pub(crate) const fn field(
    name: &'static str,
    role: FieldRole,
    surface: &'static str,
    visibility: FieldVisibility,
    removal_generation: Option<&'static str>,
) -> SerializationField {
    SerializationField {
        name,
        role,
        surface,
        visibility,
        removal_generation,
    }
}
