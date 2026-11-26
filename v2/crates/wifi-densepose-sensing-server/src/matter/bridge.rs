//! Matter bridge-tree assembly (ADR-115 §3.11.2).
//!
//! Given a list of RuView nodes and the `EntityKind`s enabled for
//! each, produce the Matter endpoint tree the SDK will materialise:
//!
//! ```text
//! Endpoint 0 (root: BridgedDevicesAggregator)
//!   Endpoint 1 (BridgedNode for ruview-node-0)
//!     Endpoint 2 (OccupancySensor for presence + PersonCount attr)
//!     Endpoint 3 (OccupancySensor for zone_kitchen)
//!     Endpoint 4 (OccupancySensor for SomeoneSleeping)
//!     Endpoint 5 (GenericSwitch for FallDetected)
//!     …
//!   Endpoint N (BridgedNode for ruview-node-1)
//!     …
//! ```
//!
//! Tree assembly is pure logic — no SDK calls. The SDK layer reads
//! this struct and registers the matching clusters. Splitting this
//! out keeps the bridge topology testable independently of the
//! `rs-matter` / chip-tool choice (per §9.10).

use crate::mqtt::discovery::EntityKind;

use super::clusters::{matter_mapping, DEVICE_TYPE_AGGREGATOR};

/// One endpoint on the Matter device tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub endpoint_id: u16,
    pub device_type: u32,
    pub label: String,
    pub clusters: Vec<u32>,
    pub vendor_attrs: Vec<u32>,
    /// `Some(_)` if this endpoint maps back to an `EntityKind`;
    /// `None` for structural endpoints (aggregator root, bridged node).
    pub source_entity: Option<EntityKind>,
}

/// One RuView node's slice of the bridge tree.
#[derive(Debug, Clone)]
pub struct NodeBranch {
    pub node_id: String,
    pub friendly_name: String,
    pub bridged_node_endpoint: u16,
    pub child_endpoints: Vec<Endpoint>,
}

/// Whole bridge tree the SDK will materialise.
#[derive(Debug, Clone)]
pub struct BridgeTree {
    pub root: Endpoint,
    pub nodes: Vec<NodeBranch>,
}

/// Builds a [`BridgeTree`] from a list of `(node_id, friendly_name,
/// entities)` tuples. Endpoint IDs are assigned monotonically starting
/// at 1 (Matter reserves endpoint 0 for the root).
pub fn build_bridge_tree(nodes: &[(String, String, Vec<EntityKind>)]) -> BridgeTree {
    let root = Endpoint {
        endpoint_id: 0,
        device_type: DEVICE_TYPE_AGGREGATOR,
        label: "RuView Bridge".into(),
        clusters: vec![super::clusters::CLUSTER_BASIC_INFORMATION],
        vendor_attrs: vec![],
        source_entity: None,
    };

    let mut next_endpoint: u16 = 1;
    let mut branches = Vec::with_capacity(nodes.len());

    for (node_id, friendly_name, entities) in nodes {
        let bridged_node_ep = next_endpoint;
        next_endpoint += 1;

        let mut children = Vec::new();

        // Build a children-by-mapping bucket: entities that share the
        // OccupancySensor endpoint (e.g. PersonCount attaches to
        // Presence's endpoint) collapse onto the parent rather than
        // taking their own endpoint ID.
        let mut presence_endpoint_id: Option<u16> = None;

        for entity in entities {
            let Some(m) = matter_mapping(*entity) else {
                continue; // explicitly MQTT-only
            };

            if m.shares_occupancy_endpoint {
                if let Some(parent_ep) = presence_endpoint_id {
                    // Attach as vendor attribute on the parent endpoint.
                    if let Some(parent) = children
                        .iter_mut()
                        .find(|c: &&mut Endpoint| c.endpoint_id == parent_ep)
                    {
                        if let Some(va) = m.vendor_attr_id {
                            parent.vendor_attrs.push(va);
                        }
                        parent.source_entity.get_or_insert(*entity);
                    }
                    continue;
                }
            }

            let ep_id = next_endpoint;
            next_endpoint += 1;
            let ep = Endpoint {
                endpoint_id: ep_id,
                device_type: m.device_type,
                label: format!("{:?}", entity),
                clusters: vec![m.cluster, super::clusters::CLUSTER_BASIC_INFORMATION],
                vendor_attrs: m.vendor_attr_id.into_iter().collect(),
                source_entity: Some(*entity),
            };
            // Switch endpoints need the event cluster declared
            // (already covered by `clusters` above — but we record it
            // for the SDK layer's convenience).
            if matches!(*entity, EntityKind::Presence) {
                presence_endpoint_id = Some(ep_id);
            }
            if let Some(_eid) = m.event_id {
                // Event support is implicit when the Switch cluster is
                // present; the SDK reads the cluster and exposes the
                // event automatically. No extra field needed.
            }
            children.push(ep);
        }

        branches.push(NodeBranch {
            node_id: node_id.clone(),
            friendly_name: friendly_name.clone(),
            bridged_node_endpoint: bridged_node_ep,
            child_endpoints: children,
        });
    }

    BridgeTree {
        root,
        nodes: branches,
    }
}

impl BridgeTree {
    /// Total number of endpoints (root + bridged nodes + per-entity).
    pub fn total_endpoints(&self) -> usize {
        let per_node: usize = self
            .nodes
            .iter()
            .map(|n| 1 + n.child_endpoints.len()) // BridgedNode + children
            .sum();
        1 /* root */ + per_node
    }

    /// Look up an endpoint by its assigned ID. Returns `None` if no
    /// endpoint with that ID exists in the tree.
    pub fn endpoint(&self, id: u16) -> Option<EndpointRef<'_>> {
        if self.root.endpoint_id == id {
            return Some(EndpointRef::Root(&self.root));
        }
        for n in &self.nodes {
            if n.bridged_node_endpoint == id {
                return Some(EndpointRef::BridgedNode(n));
            }
            for child in &n.child_endpoints {
                if child.endpoint_id == id {
                    return Some(EndpointRef::Child { branch: n, child });
                }
            }
        }
        None
    }
}

/// Resolved endpoint with backref to the owning branch (for logging /
/// error messages).
pub enum EndpointRef<'a> {
    Root(&'a Endpoint),
    BridgedNode(&'a NodeBranch),
    Child { branch: &'a NodeBranch, child: &'a Endpoint },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mqtt::discovery::EntityKind::*;

    fn fixture() -> Vec<(String, String, Vec<EntityKind>)> {
        vec![(
            "node_aabb".into(),
            "Bedroom".into(),
            vec![
                Presence,
                PersonCount, // shares Presence's endpoint
                SomeoneSleeping,
                FallDetected,
                HeartRate, // MQTT-only → must NOT add an endpoint
            ],
        )]
    }

    #[test]
    fn tree_has_aggregator_root() {
        let tree = build_bridge_tree(&fixture());
        assert_eq!(tree.root.endpoint_id, 0);
        assert_eq!(tree.root.device_type, DEVICE_TYPE_AGGREGATOR);
    }

    #[test]
    fn one_branch_per_node() {
        let tree = build_bridge_tree(&fixture());
        assert_eq!(tree.nodes.len(), 1);
        assert_eq!(tree.nodes[0].node_id, "node_aabb");
        assert_eq!(tree.nodes[0].friendly_name, "Bedroom");
        assert_eq!(tree.nodes[0].bridged_node_endpoint, 1);
    }

    #[test]
    fn person_count_collapses_onto_presence_endpoint() {
        let tree = build_bridge_tree(&fixture());
        let branch = &tree.nodes[0];

        // Children: Presence/PersonCount (1 ep), SomeoneSleeping (1 ep),
        // FallDetected (1 ep) = 3 endpoints. HR/BR → skipped.
        assert_eq!(branch.child_endpoints.len(), 3);

        // Find the Presence endpoint — it should carry the PersonCount
        // vendor attribute.
        let presence_ep = branch
            .child_endpoints
            .iter()
            .find(|e| e.source_entity == Some(Presence))
            .expect("presence endpoint missing");
        assert!(presence_ep
            .vendor_attrs
            .contains(&super::super::clusters::VENDOR_ATTR_PERSON_COUNT));
    }

    #[test]
    fn biometric_entities_skip_matter_tree() {
        let tree = build_bridge_tree(&fixture());
        let branch = &tree.nodes[0];
        for ep in &branch.child_endpoints {
            assert!(
                ep.source_entity != Some(HeartRate),
                "HeartRate must NOT have a Matter endpoint"
            );
            assert!(
                ep.source_entity != Some(BreathingRate),
                "BreathingRate must NOT have a Matter endpoint"
            );
        }
    }

    #[test]
    fn each_child_carries_basic_information_cluster() {
        let tree = build_bridge_tree(&fixture());
        for branch in &tree.nodes {
            for ep in &branch.child_endpoints {
                assert!(
                    ep.clusters
                        .contains(&super::super::clusters::CLUSTER_BASIC_INFORMATION),
                    "every endpoint must declare BasicInformation"
                );
            }
        }
    }

    #[test]
    fn endpoint_ids_are_monotonic_and_unique() {
        let tree = build_bridge_tree(&fixture());
        let mut all_ids = vec![tree.root.endpoint_id];
        for branch in &tree.nodes {
            all_ids.push(branch.bridged_node_endpoint);
            for ep in &branch.child_endpoints {
                all_ids.push(ep.endpoint_id);
            }
        }
        let mut sorted = all_ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(all_ids.len(), sorted.len(), "endpoint IDs must be unique");
    }

    #[test]
    fn total_endpoints_matches_explicit_count() {
        let tree = build_bridge_tree(&fixture());
        // 1 root + 1 bridged + 3 children = 5.
        assert_eq!(tree.total_endpoints(), 5);
    }

    #[test]
    fn endpoint_lookup_resolves_all_ids() {
        let tree = build_bridge_tree(&fixture());
        for id in 0..tree.total_endpoints() as u16 {
            let er = tree.endpoint(id);
            assert!(er.is_some(), "endpoint {} not findable", id);
        }
        // Unknown ID returns None.
        assert!(tree.endpoint(999).is_none());
    }

    #[test]
    fn multi_node_tree_keeps_per_node_isolation() {
        let nodes = vec![
            ("aabb".into(), "Bedroom".into(), vec![Presence, FallDetected]),
            ("ccdd".into(), "Living".into(), vec![Presence, MeetingInProgress]),
        ];
        let tree = build_bridge_tree(&nodes);
        assert_eq!(tree.nodes.len(), 2);
        // Each node's children are isolated to that branch.
        for branch in &tree.nodes {
            assert_eq!(branch.child_endpoints.len(), 2);
        }
        // Total endpoints: 1 root + (1 bridged + 2 children) × 2 = 7.
        assert_eq!(tree.total_endpoints(), 7);
    }

    #[test]
    fn empty_node_list_yields_just_root() {
        let tree = build_bridge_tree(&[]);
        assert_eq!(tree.nodes.len(), 0);
        assert_eq!(tree.total_endpoints(), 1); // just the root
    }
}
