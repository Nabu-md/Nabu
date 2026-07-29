use crate::event_bus::{EventBus, ItemStoredEvent, PipelineEvent};
use crate::event_bus::kinds::{GRAPH_UPDATED, ITEM_STORED};
use crate::graph::incremental::engine::IncrementalUpdateEngine;
use crate::graph::serializer::{GraphSnapshot, SerializedNode};
use crate::graph::{GraphOperation, GraphUpdatedEvent, VaultGraph};
use crate::models::KnowledgeObject;

use std::sync::{Arc, Mutex};


/// The GraphEventBridge connects the EventBus to the IncrementalUpdateEngine.
///
/// It subscribes to ItemStored events and translates them into incremental
/// graph updates. The flow is:
///
/// EVENT_ITEM_STORED
///   ↓
/// Determine change type (add/modify/delete)
///   ↓
/// Find affected graph region
///   ↓
/// Apply incremental update to VaultGraph
///   ↓
/// Persist updated graph
///   ↓
/// Publish GRAPH_UPDATED event
pub struct GraphEventBridge {
    engine: Arc<Mutex<IncrementalUpdateEngine>>,
    graph: Arc<Mutex<VaultGraph>>,
    snapshot: Arc<Mutex<GraphSnapshot>>,
}

impl GraphEventBridge {
    /// Create a new event bridge.
    pub fn new(
        engine: Arc<Mutex<IncrementalUpdateEngine>>,
        graph: Arc<Mutex<VaultGraph>>,
        snapshot: GraphSnapshot,
    ) -> Self {
        Self {
            engine,
            graph,
            snapshot: Arc::new(Mutex::new(snapshot)),
        }
    }

    /// Wire up the EventBus subscriptions for incremental graph updates.
    ///
    /// Call this during application startup to enable event-driven updates.
    pub fn wire(&self, event_bus: &EventBus<PipelineEvent>) {
        let engine = self.engine.clone();
        let graph = self.graph.clone();
        let snapshot = self.snapshot.clone();

        event_bus.subscribe(ITEM_STORED, move |event: &PipelineEvent| {
            if let PipelineEvent::ItemStored(stored) = event {
                // Translate ItemStored to incremental graph update
                let object_id = stored.object_id;

                let mut engine = engine.lock().unwrap();
                let graph = graph.lock().unwrap();
                let mut snapshot = snapshot.lock().unwrap();

                // Determine change type
                let exists_in_graph = graph.all_nodes().iter().any(|n| n.id == object_id);
                let exists_in_snapshot = snapshot.nodes.iter().any(|n| n.id == object_id);

                match (exists_in_graph, exists_in_snapshot) {
                    // New object
                    (false, false) => {
                        // Create a KnowledgeObject for the graph
                        let object = KnowledgeObject::new(
                            stored.object_type.clone(),
                            crate::models::ObjectContent::PlainText(String::new()),
                        );

                        // Use the object from the event
                        let mut obj = KnowledgeObject::new(
                            stored.object_type.clone(),
                            crate::models::ObjectContent::PlainText(String::new()),
                        );
                        obj.id = object_id;

                        graph.add_node(&obj).unwrap();

                        // Add to snapshot
                        let node = SerializedNode::new(
                            object_id,
                            stored.object_type.variant_name(),
                            None,
                            "text",
                        );
                        snapshot.add_node(node);

                        // Track incrementally
                        engine.node_added(&obj);
                    }
                    // Existing object modified
                    (true, true) => {
                        let old_object = graph.all_nodes().into_iter().find(|n| n.id == object_id);
                        let object = KnowledgeObject::new(
                            stored.object_type.clone(),
                            crate::models::ObjectContent::PlainText(String::new()),
                        );

                        engine.node_modified(&object, old_object.as_ref());

                        // Update snapshot node
                        if let Some(node) = snapshot.nodes.iter_mut().find(|n| n.id == object_id) {
                            node.object_type = stored.object_type.variant_name().to_string();
                        }

                        // Publish graph update event
                        if let Some(bus) = &graph.event_bus {
                            bus.publish(
                                GRAPH_UPDATED,
                                &PipelineEvent::GraphUpdated(GraphUpdatedEvent {
                                    object_id,
                                    operation: GraphOperation::NodeUpdated,
                                    timestamp: chrono::Utc::now(),
                                }),
                            );
                        }
                    }
                    // Object existed but snapshot didn't have it (inconsistency)
                    (true, false) => {
                        // Re-sync graph state
                        let node = SerializedNode::new(
                            object_id,
                            stored.object_type.variant_name(),
                            None,
                            "text",
                        );
                        snapshot.add_node(node);
                        engine.node_added(&KnowledgeObject::new(
                            stored.object_type.clone(),
                            crate::models::ObjectContent::PlainText(String::new()),
                        ));
                    }
                    // Object in snapshot but not graph (stale snapshot)
                    (false, true) => {
                        // Remove from snapshot
                        snapshot.nodes.retain(|n| n.id != object_id);

                        // Remove dependent edges
                        snapshot.edges.retain(|e| {
                            e.source != object_id && e.target != object_id
                        });

                        engine.node_removed(object_id);
                    }
                }
            }
        });
    }

    /// Process a batch of ItemStored events at once (for mass imports).
    pub fn process_batch(
        &self,
        events: &[ItemStoredEvent],
    ) -> Result<(), String> {
        let mut engine = self.engine.lock().map_err(|e| e.to_string())?;
        engine.begin_transaction();

        for event in events {
            let obj = KnowledgeObject::new(
                event.object_type.clone(),
                crate::models::ObjectContent::PlainText(String::new()),
            );
            engine.node_added(&obj);
        }

        engine.commit_transaction()?;
        Ok(())
    }

    /// Get the current snapshot for persistence.
    pub fn snapshot(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, GraphSnapshot>> {
        self.snapshot.lock()
    }
}

/// Convenience function: wire up incremental graph updates to the EventBus.
///
/// This creates the full event-driven pipeline:
///
/// ItemStored → Determine change → Update graph incrementally → Persist → Notify UI
pub fn wire_incremental_graph_updates(
    event_bus: &EventBus<PipelineEvent>,
    engine: Arc<Mutex<IncrementalUpdateEngine>>,
    graph: Arc<Mutex<VaultGraph>>,
    snapshot: GraphSnapshot,
) -> GraphEventBridge {
    let bridge = GraphEventBridge::new(engine, graph, snapshot);
    bridge.wire(event_bus);
    bridge
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::incremental::engine::IncrementalUpdateEngine;
    use crate::graph::version::GraphVersion;
    use crate::models::ObjectType;
    use uuid::Uuid;

    #[test]
    fn test_event_bridge_creation() {
        let engine = Arc::new(Mutex::new(IncrementalUpdateEngine::new()));
        let graph = Arc::new(Mutex::new(VaultGraph::new()));
        let snapshot = GraphSnapshot::new(GraphVersion::new());

        let _bridge = GraphEventBridge::new(engine, graph, snapshot);
    }

    #[test]
    fn test_batch_processing() {
        let engine = Arc::new(Mutex::new(IncrementalUpdateEngine::new()));
        let graph = Arc::new(Mutex::new(VaultGraph::new()));
        let snapshot = GraphSnapshot::new(GraphVersion::new());
        let bridge = GraphEventBridge::new(engine.clone(), graph, snapshot);

        let events = vec![
            ItemStoredEvent {
                object_id: Uuid::new_v4(),
                vault_path: "/test/1.md".to_string(),
                object_type: ObjectType::Note,
                timestamp: chrono::Utc::now(),
            },
            ItemStoredEvent {
                object_id: Uuid::new_v4(),
                vault_path: "/test/2.md".to_string(),
                object_type: ObjectType::Article,
                timestamp: chrono::Utc::now(),
            },
        ];

        bridge.process_batch(&events).unwrap();

        let engine = engine.lock().unwrap();
        assert!(engine.has_pending_updates());
    }
}
