use ndn_packet::{Name, NameComponent};
use std::collections::HashMap;
use std::sync::Arc;

/// Error returned when a pattern or rule string cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PatternParseError {
    #[error("empty pattern string")]
    Empty,
    #[error("unclosed capture variable (missing '>')")]
    UnclosedCapture,
    #[error("MultiCapture ('**') must be the last component")]
    MultiCaptureNotLast,
    #[error("rule must have exactly one '=>' separator")]
    BadRuleSeparator,
}

/// A single component in a name pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternComponent {
    /// Must match this exact component.
    Literal(NameComponent),
    /// Binds one component to a named variable.
    Capture(Arc<str>),
    /// Binds one or more trailing components to a named variable.
    MultiCapture(Arc<str>),
}

/// A name pattern with named capture groups.
///
/// Used by the trust schema to express rules like:
/// "Data under `/sensor/<node>/<type>` must be signed by `/sensor/<node>/KEY/<id>`"
/// where `<node>` must match in both patterns.
///
/// # Text format
///
/// Patterns can be parsed from and serialized to a human-readable string:
///
/// - `/literal` → [`PatternComponent::Literal`]
/// - `/<var>` → [`PatternComponent::Capture`] — matches one name component
/// - `/<**var>` → [`PatternComponent::MultiCapture`] — matches all remaining components (must be last)
///
/// Example: `/sensor/<node>/KEY/<id>` parses to
/// `[Literal("sensor"), Capture("node"), Literal("KEY"), Capture("id")]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamePattern(pub Vec<PatternComponent>);

impl NamePattern {
    /// Parse a pattern from a text string.
    ///
    /// Components are `/`-separated. An empty leading `/` is ignored.
    /// `<var>` is a single-component capture; `<**var>` is a multi-component
    /// capture (must be the last component in the pattern).
    ///
    /// # Examples
    ///
    /// ```
    /// use ndn_security::trust_schema::NamePattern;
    ///
    /// let p = NamePattern::parse("/sensor/<node>/KEY/<id>").unwrap();
    /// ```
    pub fn parse(s: &str) -> Result<Self, PatternParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(PatternParseError::Empty);
        }
        // Strip optional leading slash.
        let s = s.strip_prefix('/').unwrap_or(s);
        if s.is_empty() {
            // Just a lone "/" — empty pattern (matches root).
            return Ok(Self(vec![]));
        }

        let mut components = Vec::new();
        let parts: Vec<&str> = s.split('/').collect();
        let last_idx = parts.len().saturating_sub(1);

        for (i, part) in parts.iter().enumerate() {
            if let Some(inner) = part.strip_prefix('<') {
                let var = inner
                    .strip_suffix('>')
                    .ok_or(PatternParseError::UnclosedCapture)?;
                if let Some(multi_var) = var.strip_prefix("**") {
                    if i != last_idx {
                        return Err(PatternParseError::MultiCaptureNotLast);
                    }
                    components.push(PatternComponent::MultiCapture(Arc::from(multi_var)));
                } else {
                    components.push(PatternComponent::Capture(Arc::from(var)));
                }
            } else {
                let comp = NameComponent::generic(bytes::Bytes::copy_from_slice(part.as_bytes()));
                components.push(PatternComponent::Literal(comp));
            }
        }

        Ok(Self(components))
    }

    /// Attempt to match `name` against this pattern, extending `bindings`.
    /// Returns `true` if the match succeeds.
    pub fn matches(&self, name: &Name, bindings: &mut HashMap<Arc<str>, NameComponent>) -> bool {
        let components = name.components();
        let mut name_idx = 0;

        for pat in &self.0 {
            match pat {
                PatternComponent::Literal(c) => {
                    if name_idx >= components.len() || &components[name_idx] != c {
                        return false;
                    }
                    name_idx += 1;
                }
                PatternComponent::Capture(var) => {
                    if name_idx >= components.len() {
                        return false;
                    }
                    let comp = components[name_idx].clone();
                    if let Some(existing) = bindings.get(var) {
                        if existing != &comp {
                            return false; // variable must be consistent
                        }
                    } else {
                        bindings.insert(Arc::clone(var), comp);
                    }
                    name_idx += 1;
                }
                PatternComponent::MultiCapture(_var) => {
                    // Greedily consume all remaining components.
                    name_idx = components.len();
                }
            }
        }
        name_idx == components.len()
    }
}

/// A single trust schema rule: Data matching `data_pattern` must be signed
/// by a key matching `key_pattern`, with captured variables consistent between
/// both patterns.
///
/// # Text format
///
/// A rule is serialized as `"<data_pattern> => <key_pattern>"`, e.g.:
///
/// ```text
/// /sensor/<node>/<type> => /sensor/<node>/KEY/<id>
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaRule {
    pub data_pattern: NamePattern,
    pub key_pattern: NamePattern,
}

impl SchemaRule {
    /// Parse a rule from its text representation (`"data_pattern => key_pattern"`).
    pub fn parse(s: &str) -> Result<Self, PatternParseError> {
        let parts: Vec<&str> = s.splitn(2, "=>").collect();
        if parts.len() != 2 {
            return Err(PatternParseError::BadRuleSeparator);
        }
        let data_pattern = NamePattern::parse(parts[0].trim())?;
        let key_pattern = NamePattern::parse(parts[1].trim())?;
        Ok(Self { data_pattern, key_pattern })
    }

    /// Check whether `data_name` and `key_name` satisfy this rule.
    pub fn check(&self, data_name: &Name, key_name: &Name) -> bool {
        let mut bindings = HashMap::new();
        self.data_pattern.matches(data_name, &mut bindings)
            && self.key_pattern.matches(key_name, &mut bindings)
    }
}

impl std::fmt::Display for NamePattern {
    /// Serialize a pattern to its text form, e.g. `/sensor/<node>/KEY/<id>`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return f.write_str("/");
        }
        for comp in &self.0 {
            f.write_str("/")?;
            match comp {
                PatternComponent::Literal(nc) => {
                    f.write_str(&String::from_utf8_lossy(&nc.value))?;
                }
                PatternComponent::Capture(var) => {
                    write!(f, "<{var}>")?;
                }
                PatternComponent::MultiCapture(var) => {
                    write!(f, "<**{var}>")?;
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for SchemaRule {
    /// Serialize a rule to its text form, e.g. `/sensor/<node> => /sensor/<node>/KEY/<id>`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} => {}", self.data_pattern, self.key_pattern)
    }
}

/// A collection of trust schema rules.
#[derive(Clone, Debug, Default)]
pub struct TrustSchema {
    rules: Vec<SchemaRule>,
}

impl TrustSchema {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: SchemaRule) {
        self.rules.push(rule);
    }

    /// Returns `true` if at least one rule permits this (data_name, key_name) pair.
    pub fn allows(&self, data_name: &Name, key_name: &Name) -> bool {
        self.rules.iter().any(|r| r.check(data_name, key_name))
    }

    /// Return an immutable slice of all rules in this schema.
    pub fn rules(&self) -> &[SchemaRule] {
        &self.rules
    }

    /// Remove the rule at `index`, returning it.
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove_rule(&mut self, index: usize) -> SchemaRule {
        self.rules.remove(index)
    }

    /// Remove all rules, returning the schema to its empty (reject-all) state.
    pub fn clear(&mut self) {
        self.rules.clear();
    }

    /// Accept any signed packet regardless of name relationship.
    ///
    /// Useful for the `AcceptSigned` security profile and for tests.
    pub fn accept_all() -> Self {
        let mut schema = Self::new();
        schema.add_rule(SchemaRule {
            data_pattern: NamePattern(vec![PatternComponent::MultiCapture("_".into())]),
            key_pattern: NamePattern(vec![PatternComponent::MultiCapture("_".into())]),
        });
        schema
    }

    /// Hierarchical trust: data and key must share a common first component.
    ///
    /// Rule: `/<org>/**` must be signed by `/<org>/**`. The actual hierarchy
    /// is enforced by the certificate chain walk — a key can only be trusted
    /// if its cert was issued by a parent key, all the way up to a trust anchor.
    /// The schema just ensures the top-level namespace matches.
    pub fn hierarchical() -> Self {
        let mut schema = Self::new();
        schema.add_rule(SchemaRule {
            data_pattern: NamePattern(vec![
                PatternComponent::Capture("org".into()),
                PatternComponent::MultiCapture("_data".into()),
            ]),
            key_pattern: NamePattern(vec![
                PatternComponent::Capture("org".into()),
                PatternComponent::MultiCapture("_key".into()),
            ]),
        });
        schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use ndn_packet::NameComponent;

    fn comp(s: &'static str) -> NameComponent {
        NameComponent::generic(Bytes::from_static(s.as_bytes()))
    }
    fn name(components: &[&'static str]) -> Name {
        Name::from_components(components.iter().map(|s| comp(s)))
    }

    #[test]
    fn literal_matches_exact() {
        let pat = NamePattern(vec![PatternComponent::Literal(comp("sensor"))]);
        assert!(pat.matches(&name(&["sensor"]), &mut HashMap::new()));
    }

    #[test]
    fn literal_rejects_wrong_component() {
        let pat = NamePattern(vec![PatternComponent::Literal(comp("sensor"))]);
        assert!(!pat.matches(&name(&["actuator"]), &mut HashMap::new()));
    }

    #[test]
    fn literal_rejects_extra_components() {
        let pat = NamePattern(vec![PatternComponent::Literal(comp("a"))]);
        assert!(!pat.matches(&name(&["a", "b"]), &mut HashMap::new()));
    }

    #[test]
    fn capture_binds_variable() {
        let pat = NamePattern(vec![
            PatternComponent::Literal(comp("sensor")),
            PatternComponent::Capture(Arc::from("node")),
        ]);
        let mut bindings = HashMap::new();
        assert!(pat.matches(&name(&["sensor", "node1"]), &mut bindings));
        assert_eq!(bindings[&Arc::from("node")], comp("node1"));
    }

    #[test]
    fn capture_enforces_consistency() {
        let var: Arc<str> = Arc::from("node");
        let data_pat = NamePattern(vec![PatternComponent::Capture(Arc::clone(&var))]);
        let key_pat = NamePattern(vec![PatternComponent::Capture(Arc::clone(&var))]);
        let mut bindings = HashMap::new();
        // Bind node = "n1" via data pattern
        assert!(data_pat.matches(&name(&["n1"]), &mut bindings));
        // Key pattern with same value succeeds
        assert!(key_pat.matches(&name(&["n1"]), &mut bindings.clone()));
        // Key pattern with different value fails
        assert!(!key_pat.matches(&name(&["n2"]), &mut bindings));
    }

    #[test]
    fn multi_capture_consumes_remaining() {
        let pat = NamePattern(vec![
            PatternComponent::Literal(comp("prefix")),
            PatternComponent::MultiCapture(Arc::from("rest")),
        ]);
        assert!(pat.matches(&name(&["prefix", "a", "b", "c"]), &mut HashMap::new()));
    }

    #[test]
    fn schema_rule_allows_matching_pair() {
        let rule = SchemaRule {
            data_pattern: NamePattern(vec![PatternComponent::Literal(comp("data"))]),
            key_pattern: NamePattern(vec![PatternComponent::Literal(comp("key"))]),
        };
        assert!(rule.check(&name(&["data"]), &name(&["key"])));
        assert!(!rule.check(&name(&["data"]), &name(&["wrong"])));
    }

    #[test]
    fn trust_schema_allows_via_any_rule() {
        let mut schema = TrustSchema::new();
        schema.add_rule(SchemaRule {
            data_pattern: NamePattern(vec![PatternComponent::Literal(comp("data"))]),
            key_pattern: NamePattern(vec![PatternComponent::Literal(comp("key"))]),
        });
        assert!(schema.allows(&name(&["data"]), &name(&["key"])));
        assert!(!schema.allows(&name(&["data"]), &name(&["wrong"])));
    }

    #[test]
    fn empty_schema_rejects_everything() {
        let schema = TrustSchema::new();
        assert!(!schema.allows(&name(&["a"]), &name(&["b"])));
    }

    #[test]
    fn accept_all_allows_any_pair() {
        let schema = TrustSchema::accept_all();
        assert!(schema.allows(&name(&["a", "b"]), &name(&["x", "y", "z"])));
        assert!(schema.allows(&name(&["data"]), &name(&["key"])));
    }

    #[test]
    fn pattern_parse_literal() {
        let p = NamePattern::parse("/sensor/temp").unwrap();
        assert_eq!(p.0.len(), 2);
        assert!(matches!(&p.0[0], PatternComponent::Literal(nc) if nc.value.as_ref() == b"sensor"));
        assert!(matches!(&p.0[1], PatternComponent::Literal(nc) if nc.value.as_ref() == b"temp"));
    }

    #[test]
    fn pattern_parse_captures() {
        let p = NamePattern::parse("/sensor/<node>/KEY/<id>").unwrap();
        assert_eq!(p.0.len(), 4);
        assert!(matches!(&p.0[0], PatternComponent::Literal(_)));
        assert!(matches!(&p.0[1], PatternComponent::Capture(v) if v.as_ref() == "node"));
        assert!(matches!(&p.0[2], PatternComponent::Literal(_)));
        assert!(matches!(&p.0[3], PatternComponent::Capture(v) if v.as_ref() == "id"));
    }

    #[test]
    fn pattern_parse_multi_capture_at_end() {
        let p = NamePattern::parse("/org/<**rest>").unwrap();
        assert_eq!(p.0.len(), 2);
        assert!(matches!(&p.0[1], PatternComponent::MultiCapture(v) if v.as_ref() == "rest"));
    }

    #[test]
    fn pattern_parse_multi_capture_not_last_errors() {
        assert!(matches!(
            NamePattern::parse("/org/<**rest>/extra"),
            Err(PatternParseError::MultiCaptureNotLast)
        ));
    }

    #[test]
    fn pattern_parse_unclosed_capture_errors() {
        assert!(matches!(
            NamePattern::parse("/sensor/<node"),
            Err(PatternParseError::UnclosedCapture)
        ));
    }

    #[test]
    fn pattern_roundtrip_text() {
        let s = "/sensor/<node>/KEY/<id>";
        let p = NamePattern::parse(s).unwrap();
        assert_eq!(p.to_string(), s);
    }

    #[test]
    fn pattern_roundtrip_multi() {
        let s = "/org/<**rest>";
        let p = NamePattern::parse(s).unwrap();
        assert_eq!(p.to_string(), s);
    }

    #[test]
    fn rule_parse_roundtrip() {
        let s = "/sensor/<node>/<type> => /sensor/<node>/KEY/<id>";
        let r = SchemaRule::parse(s).unwrap();
        assert_eq!(r.to_string(), s);
    }

    #[test]
    fn rule_parse_bad_separator_errors() {
        assert!(matches!(
            SchemaRule::parse("/a /b"),
            Err(PatternParseError::BadRuleSeparator)
        ));
    }

    #[test]
    fn schema_remove_rule() {
        let mut schema = TrustSchema::new();
        schema.add_rule(SchemaRule {
            data_pattern: NamePattern(vec![PatternComponent::Literal(comp("data"))]),
            key_pattern: NamePattern(vec![PatternComponent::Literal(comp("key"))]),
        });
        assert!(schema.allows(&name(&["data"]), &name(&["key"])));
        schema.remove_rule(0);
        assert!(!schema.allows(&name(&["data"]), &name(&["key"])));
    }

    #[test]
    fn schema_rules_returns_slice() {
        let mut schema = TrustSchema::new();
        schema.add_rule(SchemaRule {
            data_pattern: NamePattern(vec![PatternComponent::Literal(comp("d"))]),
            key_pattern: NamePattern(vec![PatternComponent::Literal(comp("k"))]),
        });
        assert_eq!(schema.rules().len(), 1);
    }

    #[test]
    fn hierarchical_requires_matching_first_component() {
        let schema = TrustSchema::hierarchical();
        // Same org: allowed
        assert!(schema.allows(&name(&["org", "data"]), &name(&["org", "KEY", "k1"])));
        // Different org: rejected
        assert!(!schema.allows(&name(&["orgA", "data"]), &name(&["orgB", "KEY", "k1"])));
        // Same org, deeper hierarchy: allowed
        assert!(schema.allows(
            &name(&["org", "dept", "sensor", "temp"]),
            &name(&["org", "dept", "KEY", "k1"])
        ));
    }
}
