use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use praxis_core::r#loop::LoopPathologyDetector;
use praxis_memory::embedding::EmbeddingService;
use praxis_memory::episodic::{ChunkMetadata, ChunkType, EpisodicMemory, MemoryChunk};
use praxis_providers::MockProvider;
use std::sync::Arc;

fn bench_agent_execute_mock(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("agent_execute_mock", |b| {
        b.to_async(&rt).iter_custom(|_| async {
            let provider = MockProvider::simple("bench");
            let es = Arc::new(EmbeddingService::new_default(
                Arc::new(provider) as Arc<dyn praxis_agent_traits::provider::LLMProvider>
            ));
            let mut memory = EpisodicMemory::default_store().with_embedding_service(es);

            let chunk = MemoryChunk {
                id: uuid::Uuid::new_v4().to_string(),
                content: "Benchmark agent execution with mock provider".to_string(),
                embedding: vec![0.1; 768],
                metadata: ChunkMetadata {
                    session_id: "bench".to_string(),
                    project_id: "bench".to_string(),
                    agent_id: "coder".to_string(),
                    chunk_type: ChunkType::Conversation,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    token_count: 42,
                },
                score: None,
            };

            let start = std::time::Instant::now();
            memory.store(chunk).await;
            start.elapsed()
        });
    });
}

fn bench_phase_transition(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("phase_transition", |b| {
        b.to_async(&rt).iter_custom(|_| async {
            let mut detector = LoopPathologyDetector::new();
            let start = std::time::Instant::now();

            for i in 0..100 {
                detector.record_iteration(
                    i,
                    &format!("Iteration {} output", i),
                    "Planning",
                    Some(1000),
                );
            }
            start.elapsed()
        });
    });
}

fn bench_rag_injection(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("rag_injection", |b| {
        b.to_async(&rt).iter_custom(|_| async {
            let provider = MockProvider::simple("bench");
            let es = Arc::new(EmbeddingService::new_default(
                Arc::new(provider) as Arc<dyn praxis_agent_traits::provider::LLMProvider>
            ));
            let mut memory = EpisodicMemory::default_store().with_embedding_service(es);

            // Pre-populate with chunks
            for i in 0..50 {
                memory
                    .store(MemoryChunk {
                        id: uuid::Uuid::new_v4().to_string(),
                        content: format!("Benchmark chunk {} with relevant context", i),
                        embedding: vec![0.1 + i as f32 * 0.01; 768],
                        metadata: ChunkMetadata {
                            session_id: "bench".to_string(),
                            project_id: "bench".to_string(),
                            agent_id: "coder".to_string(),
                            chunk_type: ChunkType::Conversation,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            token_count: 100,
                        },
                        score: None,
                    })
                    .await;
            }

            let query = "benchmark context";
            let query_embedding = es.embed(query).await;

            let start = std::time::Instant::now();
            let results = memory.search_with_filter(&query_embedding, 5, None);
            let _ = black_box(results);
            start.elapsed()
        });
    });
}

criterion_group!(
    benches,
    bench_agent_execute_mock,
    bench_phase_transition,
    bench_rag_injection
);
criterion_main!(benches);
