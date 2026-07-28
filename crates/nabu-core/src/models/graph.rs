use serde::{Deserialize, Serialize};
use uuid::Uuid;


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationType {
    BelongsTo,
    WorksOn,
    RelatedTo,
    CreatedBy,
    References,
    MemberOf,
    DependsOn,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    pub source: Uuid,
    pub target: Uuid,
    pub relation: RelationType,
}
