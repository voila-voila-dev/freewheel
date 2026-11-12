//! Who a source address belongs to.

/// The result of looking an address up, as upstream does: groups newest first, and in each
/// group its links before the address that last spoke for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AddressOwner {
    /// A registered link: indexes of its group and of the link in it.
    Link { group: usize, link: usize },
    /// Not a link, but the group's last address (e.g. the one that sent REG1).
    Group(usize),
    /// Nobody: its packets are dropped silently.
    Nobody,
}
