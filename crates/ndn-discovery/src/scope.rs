//! Namespace isolation — well-known prefix constants and scope enforcement.
//!
//! All discovery and local management traffic lives under `/ndn/local/`, which
//! **must never be forwarded beyond the local link**.  This mirrors IPv6
//! link-local address semantics (`fe80::/10`).
//!
//! ## Reserved sub-namespaces
//!
//! | Prefix | Purpose |
//! |--------|---------|
//! | `/ndn/local/nd/hello`        | Neighbor discovery hello Interest/Data |
//! | `/ndn/local/nd/probe/direct` | SWIM direct liveness probe |
//! | `/ndn/local/nd/probe/via`    | SWIM indirect liveness probe |
//! | `/ndn/local/nd/peers`        | Demand-driven neighbor queries |
//! | `/ndn/local/sd/services`     | Service discovery records |
//! | `/ndn/local/sd/updates`      | Service discovery SVS sync group |
//! | `/ndn/local/routing/lsa`     | Link-state advertisements (NLSR adapter) |
//! | `/ndn/local/routing/prefix`  | Prefix announcements |
//! | `/ndn/local/mgmt`            | Management protocol |
//!
//! Third-party or experimental protocols must use:
//! `/ndn/local/x/<owner-name>/v=<version>/...`
//!
//! ## Scope roots for service discovery
//!
//! | [`DiscoveryScope`](crate::config::DiscoveryScope) | Root prefix |
//! |----------------------------------------------------|-------------|
//! | `LinkLocal` | `/ndn/local` |
//! | `Site`      | `/ndn/site`  |
//! | `Global`    | `/ndn/global`|

use std::str::FromStr;
use std::sync::OnceLock;

use ndn_packet::Name;

use crate::config::DiscoveryScope;

// ─── Macro helper ─────────────────────────────────────────────────────────────

/// Build and cache a `Name` from a string literal.
///
/// Well-known names are parsed once at first use and stored as `&'static Name`.
macro_rules! cached_name {
    ($vis:vis fn $fn:ident() -> $s:literal) => {
        $vis fn $fn() -> &'static Name {
            static CELL: OnceLock<Name> = OnceLock::new();
            CELL.get_or_init(|| {
                Name::from_str($s).expect(concat!("invalid well-known name: ", $s))
            })
        }
    };
}

// ─── Link-local root ──────────────────────────────────────────────────────────

cached_name!(pub fn ndn_local() -> "/ndn/local");

// ─── Neighbor discovery sub-prefixes ──────────────────────────────────────────

cached_name!(pub fn nd_root()        -> "/ndn/local/nd");
cached_name!(pub fn hello_prefix()   -> "/ndn/local/nd/hello");
cached_name!(pub fn probe_direct()   -> "/ndn/local/nd/probe/direct");
cached_name!(pub fn probe_via()      -> "/ndn/local/nd/probe/via");
cached_name!(pub fn peers_prefix()   -> "/ndn/local/nd/peers");
cached_name!(pub fn gossip_prefix()  -> "/ndn/local/nd/gossip");

// ─── Service discovery sub-prefixes ───────────────────────────────────────────

cached_name!(pub fn sd_root()     -> "/ndn/local/sd");
cached_name!(pub fn sd_services() -> "/ndn/local/sd/services");
cached_name!(pub fn sd_updates()  -> "/ndn/local/sd/updates");

// ─── Routing sub-prefixes ─────────────────────────────────────────────────────

cached_name!(pub fn routing_lsa()    -> "/ndn/local/routing/lsa");
cached_name!(pub fn routing_prefix() -> "/ndn/local/routing/prefix");

// ─── Management sub-prefix ───────────────────────────────────────────────────

cached_name!(pub fn mgmt_prefix() -> "/ndn/local/mgmt");

// ─── Scope roots ─────────────────────────────────────────────────────────────

cached_name!(pub fn site_root()   -> "/ndn/site");
cached_name!(pub fn global_root() -> "/ndn/global");

/// Return the root prefix for the given [`DiscoveryScope`].
///
/// - `LinkLocal` → `/ndn/local`
/// - `Site`      → `/ndn/site`
/// - `Global`    → `/ndn/global`
pub fn scope_root(scope: &DiscoveryScope) -> &'static Name {
    match scope {
        DiscoveryScope::LinkLocal => ndn_local(),
        DiscoveryScope::Site => site_root(),
        DiscoveryScope::Global => global_root(),
    }
}

// ─── Predicates ──────────────────────────────────────────────────────────────

/// Return `true` if `name` is under `/ndn/local/` (link-local scope).
///
/// Any packet whose name matches this predicate **must not** be forwarded
/// beyond the local link.  The engine enforces this by dropping outbound
/// Interest and Data packets whose name is link-local when the outbound face
/// is not a local-scope face.
#[inline]
pub fn is_link_local(name: &Name) -> bool {
    name.has_prefix(ndn_local())
}

/// Return `true` if `name` falls under the neighbor-discovery sub-tree
/// (`/ndn/local/nd/`).
#[inline]
pub fn is_nd_packet(name: &Name) -> bool {
    name.has_prefix(nd_root())
}

/// Return `true` if `name` falls under the service-discovery sub-tree
/// (`/ndn/local/sd/`).
#[inline]
pub fn is_sd_packet(name: &Name) -> bool {
    name.has_prefix(sd_root())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ndn_packet::Name;

    use super::*;

    fn n(s: &str) -> Name {
        Name::from_str(s).unwrap()
    }

    #[test]
    fn hello_prefix_is_link_local() {
        assert!(is_link_local(hello_prefix()));
    }

    #[test]
    fn nd_root_is_nd_packet() {
        assert!(is_nd_packet(&n("/ndn/local/nd/hello/abc")));
        assert!(!is_nd_packet(&n("/ndn/local/sd/services")));
    }

    #[test]
    fn sd_root_is_sd_packet() {
        assert!(is_sd_packet(&n("/ndn/local/sd/services/foo")));
        assert!(!is_sd_packet(&n("/ndn/local/nd/hello/abc")));
    }

    #[test]
    fn non_local_is_not_link_local() {
        assert!(!is_link_local(&n("/ndn/edu/ucla/cs")));
    }

    #[test]
    fn scope_root_returns_correct_prefix() {
        assert_eq!(scope_root(&DiscoveryScope::LinkLocal), ndn_local());
        assert_eq!(scope_root(&DiscoveryScope::Site), site_root());
        assert_eq!(scope_root(&DiscoveryScope::Global), global_root());
    }

    #[test]
    fn nd_and_sd_are_disjoint() {
        // Neither is a prefix of the other.
        assert!(!nd_root().has_prefix(sd_root()));
        assert!(!sd_root().has_prefix(nd_root()));
    }
}
