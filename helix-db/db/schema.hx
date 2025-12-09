// Vidkosha Cortex foundational Helix schema
// Vector-first chunks + thin graph spine (Artifact/Agent/Topic/Project) for governance and hydration.

// Semantic chunks (vector rows)
V::MemoryChunk {
    agent_name: String,
    topic: String,
    project: String,
    summary: String,
    timestamp: Date,
    open_questions: [String],
    metadata: String,
    payload_hash: String,
    chunk_id: String,
    artifact_id: String,
}

// Canonical memory node used for graph neighbors and richer associations
N::MemoryEntry {
    agent_name: String,
    topic: String,
    project: String,
    summary: String,
    full_content: String,
    timestamp: Date,
    confidence: F32,
    open_questions: [String],
    metadata: String,
    conversation_id: String,
}

// Logical artifact representing a memory composed of one or more chunks
N::Artifact {
    agent_name: String,
    topic: String,
    project: String,
    summary: String,
    status: String,
    payload_hash: String,
    metadata: String,
    created_at: Date,
}

// Registry of agents created by AgentCreator
N::Agent {
    name: String,
    role: String,
    agent_version: String,
    routing_intent: String,
    metadata: String,
}

N::Conversation {
    conversation_id: String,
    title: String,
    metadata: String,
}

N::Message {
    message_id: String,
    conversation_id: String,
    role: String,
    content: String,
    created_at: Date,
    reply_to: String,
    metadata: String,
}

N::ToolCall {
    tool_call_id: String,
    tool_name: String,
    args_json: String,
    result_summary: String,
    created_at: Date,
    metadata: String,
}

N::PerspectiveView {
    role: String,
    summary: String,
    body: String,
    risks: String,
    decisions: String,
    actions: String,
    metadata: String,
}

// Topical taxonomy
N::Topic {
    name: String,
    metadata: String,
}

// Project grouping
N::Project {
    name: String,
    metadata: String,
}

// Relationships (typed endpoints)
E::Produced_by {
    From: Artifact,
    To: Agent,
}

E::Belongs_to_topic {
    From: Artifact,
    To: Topic,
}

E::Belongs_to_project {
    From: Artifact,
    To: Project,
}

E::Supersedes {
    From: Artifact,
    To: Artifact,
    Properties: {
        reason: String,
    }
}

E::Relates_to {
    From: Artifact,
    To: Artifact,
    Properties: {
        relation: String,
        weight: F32,
    }
}

// Memory graph edges (vector chunk + canonical node + context)
E::Chunk_of_memory {
    From: MemoryChunk,
    To: MemoryEntry,
}

E::Recorded_by {
    From: MemoryEntry,
    To: Agent,
}

E::Relates_to_topic_v2 {
    From: MemoryEntry,
    To: Topic,
}

E::Part_of_project_v2 {
    From: MemoryEntry,
    To: Project,
}

E::Has_perspective {
    From: MemoryEntry,
    To: PerspectiveView,
}

E::References_artifact_v2 {
    From: MemoryEntry,
    To: Artifact,
}

E::Produced_memory {
    From: ToolCall,
    To: MemoryEntry,
}

E::In_thread {
    From: Message,
    To: Conversation,
}

E::Replies_to {
    From: Message,
    To: Message,
}

E::Has_message {
    From: Conversation,
    To: Message,
}