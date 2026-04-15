use heck::{ToKebabCase, ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};

/// Output case convention for serialized member names.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaseConvention {
    /// camelCase (e.g. `publishedAt`)
    CamelCase,
    /// snake_case (e.g. `published_at`)
    SnakeCase,
    /// kebab-case (e.g. `published-at`)
    KebabCase,
    /// PascalCase (e.g. `PublishedAt`)
    PascalCase,
    /// Pass-through — use the Rust field name as-is.
    #[default]
    None,
}

impl CaseConvention {
    /// Convert a name to this convention.
    pub fn convert(&self, name: &str) -> String {
        match self {
            CaseConvention::CamelCase => name.to_lower_camel_case(),
            CaseConvention::SnakeCase => name.to_snake_case(),
            CaseConvention::KebabCase => name.to_kebab_case(),
            CaseConvention::PascalCase => name.to_upper_camel_case(),
            CaseConvention::None => name.to_string(),
        }
    }
}

/// Controls output casing for attribute and relationship member names.
///
/// Used by the derive macro when `#[jsonapi(case = "...")]` is specified on a struct.
/// The default convention is [`CaseConvention::None`] (pass-through).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CaseConfig {
    /// Case convention applied to attribute and relationship member names.
    pub member_case: CaseConvention,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_conversion() {
        assert_eq!(
            CaseConvention::CamelCase.convert("published_at"),
            "publishedAt"
        );
        assert_eq!(CaseConvention::CamelCase.convert("first_name"), "firstName");
        assert_eq!(CaseConvention::CamelCase.convert("title"), "title");
    }

    #[test]
    fn snake_case_conversion() {
        assert_eq!(
            CaseConvention::SnakeCase.convert("publishedAt"),
            "published_at"
        );
        assert_eq!(
            CaseConvention::SnakeCase.convert("first_name"),
            "first_name"
        );
    }

    #[test]
    fn kebab_case_conversion() {
        assert_eq!(
            CaseConvention::KebabCase.convert("published_at"),
            "published-at"
        );
        assert_eq!(CaseConvention::KebabCase.convert("firstName"), "first-name");
    }

    #[test]
    fn pascal_case_conversion() {
        assert_eq!(
            CaseConvention::PascalCase.convert("published_at"),
            "PublishedAt"
        );
        assert_eq!(CaseConvention::PascalCase.convert("firstName"), "FirstName");
    }

    #[test]
    fn none_passthrough() {
        assert_eq!(CaseConvention::None.convert("published_at"), "published_at");
        assert_eq!(CaseConvention::None.convert("firstName"), "firstName");
    }

    #[test]
    fn default_is_none() {
        assert_eq!(CaseConvention::default(), CaseConvention::None);
    }

    #[test]
    fn case_config_default() {
        let config = CaseConfig::default();
        assert_eq!(config.member_case, CaseConvention::None);
    }
}
