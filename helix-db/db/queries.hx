// Minimal vector insert matching the schema (caller supplies vector and properties)
QUERY InsertMemoryChunk(
    vector: [F64],
    agent_name: String,
    topic: String,
    project: String,
    summary: String,
    timestamp: Date,
    open_questions: [String],
    metadata: String,
    payload_hash: String,
    chunk_id: String,
    artifact_id: String
) =>
    memory_chunk <- AddV<MemoryChunk>(vector, {
        agent_name: agent_name,
        topic: topic,
        project: project,
        summary: summary,
        timestamp: timestamp,
        open_questions: open_questions,
        metadata: metadata,
        payload_hash: payload_hash,
        chunk_id: chunk_id,
        artifact_id: artifact_id,
    })

    RETURN memory_chunk

// Basic vector search over MemoryChunk
QUERY SearchMemoryChunk(
    vector: [F64],
    limit: I64
) =>
    matches <- SearchV<MemoryChunk>(vector, limit)

    RETURN matches

// Delete a memory chunk by chunk_id (client-supplied identifier)
QUERY DeleteMemoryChunk(
    chunk_id: String
) =>
    // Drop matching vector rows (Helix currently deletes via traversals, not filters)
    DROP V<MemoryChunk>::WHERE(_::{chunk_id}::EQ(chunk_id))

    RETURN "Deleted memory chunks"

// Insert-or-replace a topical/category node (enforces uniqueness by name).
QUERY InsertTopic(
    name: String,
    metadata: String
) =>
    DROP N<Topic>::WHERE(_::{name}::EQ(name))

    topic <- AddN<Topic>({
        name: name,
        metadata: metadata,
    })

    RETURN topic

// Delete topics by exact name (used for de-duping seed loads).
QUERY DeleteTopic(
    name: String
) =>
    DROP N<Topic>::WHERE(_::{name}::EQ(name))

    RETURN "Deleted topics"

// List all Topic nodes (no filtering); client can dedupe/format as needed.
QUERY ListTopics() =>
    topics <- N<Topic>

    RETURN topics

// Exact-name topic lookup.
QUERY SearchTopics(
    name: String
) =>
    topics <- N<Topic>::WHERE(_::{name}::EQ(name))

    RETURN topics

// Rich write that creates a canonical memory node, a vector chunk, and basic edges.
QUERY write_memory_v2(
    vector: [F64],
    agent_name: String,
    topic: String,
    project: String,
    summary: String,
    full_content: String,
    timestamp: Date,
    confidence: F32,
    open_questions: [String],
    metadata: String,
    payload_hash: String,
    chunk_id: String,
    artifact_id: String,
    conversation_id: String
) =>
    memory_entry <- AddN<MemoryEntry>({
        agent_name: agent_name,
        topic: topic,
        project: project,
        summary: summary,
        full_content: full_content,
        timestamp: timestamp,
        confidence: confidence,
        open_questions: open_questions,
        metadata: metadata,
        conversation_id: conversation_id,
    })

    memory_chunk <- AddV<MemoryChunk>(vector, {
        agent_name: agent_name,
        topic: topic,
        project: project,
        summary: summary,
        timestamp: timestamp,
        open_questions: open_questions,
        metadata: metadata,
        payload_hash: payload_hash,
        chunk_id: chunk_id,
        artifact_id: artifact_id,
    })

    chunk_edge <- AddE<Chunk_of_memory>::From(memory_chunk)::To(memory_entry)

    topic_node <- AddN<Topic>({
        name: topic,
        metadata: metadata,
    })
    topic_edge <- AddE<Relates_to_topic_v2>::From(memory_entry)::To(topic_node)

    project_node <- AddN<Project>({
        name: project,
        metadata: metadata,
    })
    project_edge <- AddE<Part_of_project_v2>::From(memory_entry)::To(project_node)

    agent_node <- AddN<Agent>({
        name: agent_name,
        role: agent_name,
        agent_version: "v-auto",
        routing_intent: "memory_ingest",
        metadata: metadata,
    })
    agent_edge <- AddE<Recorded_by>::From(memory_entry)::To(agent_node)

    artifact_node <- AddN<Artifact>({
        agent_name: agent_name,
        topic: topic,
        project: project,
        summary: summary,
        status: "materialized",
        payload_hash: payload_hash,
        metadata: metadata,
        created_at: timestamp,
    })
    artifact_edge <- AddE<References_artifact_v2>::From(memory_entry)::To(artifact_node)

    RETURN { memory_entry: memory_entry, memory_chunk: memory_chunk }

// Vector search over MemoryChunk (v2 alias for clarity)
QUERY search_memory_v2(
    vector: [F64],
    limit: I64
) =>
    matches <- SearchV<MemoryChunk>(vector, limit)

    RETURN matches

// Delete both the canonical memory node and its vector chunk
QUERY delete_memory_v2(
    memory_id: ID,
    chunk_id: String
) =>
    DROP V<MemoryChunk>::WHERE(_::{chunk_id}::EQ(chunk_id))::OutE<Chunk_of_memory>
    DROP V<MemoryChunk>::WHERE(_::{chunk_id}::EQ(chunk_id))
    DROP N<MemoryEntry>::WHERE(_::{id}::EQ(memory_id))

    RETURN "Deleted memory entry and chunk"