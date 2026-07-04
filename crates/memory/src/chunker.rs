//! Chunker — splits long text into chunks for embedding + indexing.
//!
//! Strategies:
//! - **BySize**: split by token budget (default 512 tokens).
//! - **ByStructure**: split on markdown headings, paragraphs.
//! - **ByTurn**: split conversation into speaker turns.
//! - **BySentence**: split at sentence boundaries (for dense prose).

// ─── Chunk ─────────────────────────────────────────────────────

/// A single chunk produced by the chunker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Chunk {
    /// The chunk content.
    pub content: String,
    /// Chunk index within the source.
    pub index: usize,
    /// Token count estimate.
    pub token_count: u32,
    /// Type of boundary used for this chunk.
    pub boundary: ChunkBoundary,
}

/// What boundary was used to split.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChunkBoundary {
    SizeHard,
    Heading,
    Paragraph,
    Sentence,
    Turn,
}

// ─── Chunking Strategy ─────────────────────────────────────────

/// Chunking strategy selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkStrategy {
    /// Split strictly by token budget (default 512 tokens).
    BySize,
    /// Split on markdown headings, fall back to paragraphs.
    ByStructure,
    /// Split on speaker turns (conversation).
    ByTurn,
    /// Split on sentence boundaries.
    BySentence,
}

impl ChunkStrategy {
    /// Detect the best strategy from content heuristics.
    pub fn detect(content: &str) -> Self {
        let lines: Vec<&str> = content.lines().collect();

        // Check for structured content: headings, lists, code blocks
        let has_headings = lines.iter().any(|l| l.trim().starts_with('#'));
        let has_turns = lines.iter().any(|l| l.starts_with("**") || l.contains(":**"));
        let has_code_blocks = content.contains("```");
        let has_sentences = content.contains('.') && content.len() > 200;

        if has_headings || has_code_blocks {
            ChunkStrategy::ByStructure
        } else if has_turns {
            ChunkStrategy::ByTurn
        } else if has_sentences {
            ChunkStrategy::BySentence
        } else {
            ChunkStrategy::BySize
        }
    }
}

// ─── Chunker ───────────────────────────────────────────────────

/// Splits text into chunks using the chosen strategy.
pub struct Chunker {
    /// Target token count per chunk.
    pub target_tokens: u32,
    /// Overlap between chunks (tokens, for sliding window).
    pub overlap_tokens: u32,
    /// Strategy override (None = auto-detect).
    pub strategy: Option<ChunkStrategy>,
    /// Token counter (uses the simple estimator from context.rs).
    token_counter: crate::context::TokenCounter,
}

impl Chunker {
    /// Create a new chunker with the given target size.
    pub fn new(target_tokens: u32) -> Self {
        Self {
            target_tokens,
            overlap_tokens: (target_tokens as f32 * 0.1) as u32, // 10% overlap
            strategy: None,
            token_counter: crate::context::TokenCounter::default_token_counter(),
        }
    }

    /// Create a chunker with default settings (512 tokens, auto-detect).
    pub fn default_chunker() -> Self {
        Self::new(512)
    }

    /// Set the target tokens per chunk.
    pub fn with_target(mut self, tokens: u32) -> Self {
        self.target_tokens = tokens;
        self.overlap_tokens = (tokens as f32 * 0.1) as u32;
        self
    }

    /// Set the overlap between chunks.
    pub fn with_overlap(mut self, tokens: u32) -> Self {
        self.overlap_tokens = tokens;
        self
    }

    /// Override the strategy (don't auto-detect).
    pub fn with_strategy(mut self, strategy: ChunkStrategy) -> Self {
        self.strategy = Some(strategy);
        self
    }

    /// Split content into chunks.
    pub fn chunk(&self, content: &str) -> Vec<Chunk> {
        let strategy = self.strategy.unwrap_or_else(|| ChunkStrategy::detect(content));

        match strategy {
            ChunkStrategy::BySize => self.chunk_by_size(content),
            ChunkStrategy::ByStructure => self.chunk_by_structure(content),
            ChunkStrategy::ByTurn => self.chunk_by_turn(content),
            ChunkStrategy::BySentence => self.chunk_by_sentence(content),
        }
    }

    /// Split strictly by token budget.
    fn chunk_by_size(&self, content: &str) -> Vec<Chunk> {
        if content.is_empty() {
            return Vec::new();
        }

        let total_tokens = self.token_counter.count_tokens(content);
        if total_tokens <= self.target_tokens {
            return vec![Chunk {
                content: content.to_string(),
                index: 0,
                token_count: total_tokens,
                boundary: ChunkBoundary::SizeHard,
            }];
        }

        let mut chunks = Vec::new();
        let words: Vec<&str> = content.split_whitespace().collect();
        let mut current_chunk = String::new();
        let mut current_tokens: u32 = 0;
        let mut chunk_index: usize = 0;

        for word in &words {
            let word_tokens = self.token_counter.count_tokens(word);
            let word_with_space = if current_chunk.is_empty() {
                word.to_string()
            } else {
                format!(" {}", word)
            };
            let word_tokens_with_space = if current_chunk.is_empty() {
                word_tokens
            } else {
                word_tokens + 1 // space token
            };

            if current_tokens + word_tokens_with_space > self.target_tokens && !current_chunk.is_empty() {
                // Emit current chunk
                chunks.push(Chunk {
                    content: current_chunk.trim().to_string(),
                    index: chunk_index,
                    token_count: current_tokens,
                    boundary: ChunkBoundary::SizeHard,
                });
                chunk_index += 1;

                // Start new chunk with overlap
                current_chunk = self.extract_overlap(&current_chunk);
                current_tokens = self.token_counter.count_tokens(&current_chunk);

                // Now add the current word
                current_chunk.push(' ');
                current_chunk.push_str(word);
                current_tokens += word_tokens + 1;
            } else {
                current_chunk.push_str(&word_with_space);
                current_tokens += word_tokens_with_space;
            }
        }

        // Last chunk
        if !current_chunk.trim().is_empty() {
            chunks.push(Chunk {
                content: current_chunk.trim().to_string(),
                index: chunk_index,
                token_count: current_tokens,
                boundary: ChunkBoundary::SizeHard,
            });
        }

        chunks
    }

    /// Split on markdown structure: headings, then paragraphs.
    fn chunk_by_structure(&self, content: &str) -> Vec<Chunk> {
        if content.is_empty() {
            return Vec::new();
        }

        // Split into sections by heading markers
        let mut sections: Vec<String> = Vec::new();
        let mut current_section = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                if !current_section.is_empty() {
                    sections.push(current_section.trim().to_string());
                }
                current_section = line.to_string();
            } else {
                current_section.push('\n');
                current_section.push_str(line);
            }
        }
        if !current_section.trim().is_empty() {
            sections.push(current_section.trim().to_string());
        }
        if sections.is_empty() {
            sections.push(content.to_string());
        }

        // Chunk each section by size if too large
        let mut chunks = Vec::new();
        let mut index = 0;

        for section in &sections {
            let section_tokens = self.token_counter.count_tokens(section);
            if section_tokens <= self.target_tokens {
                chunks.push(Chunk {
                    content: section.clone(),
                    index,
                    token_count: section_tokens,
                    boundary: ChunkBoundary::Heading,
                });
                index += 1;
            } else {
                // Split large sections into paragraphs
                let paragraphs: Vec<&str> = section
                    .split("\n\n")
                    .filter(|p| !p.trim().is_empty())
                    .collect();

                let mut current_para = String::new();
                let mut current_tokens = 0u32;

                for para in &paragraphs {
                    let para_tokens = self.token_counter.count_tokens(para);
                    if current_tokens + para_tokens > self.target_tokens && !current_para.is_empty() {
                        chunks.push(Chunk {
                            content: current_para.trim().to_string(),
                            index,
                            token_count: current_tokens,
                            boundary: ChunkBoundary::Paragraph,
                        });
                        index += 1;
                        current_para = para.to_string();
                        current_tokens = para_tokens;
                    } else {
                        if !current_para.is_empty() {
                            current_para.push_str("\n\n");
                        }
                        current_para.push_str(para);
                        current_tokens += para_tokens;
                    }
                }

                if !current_para.trim().is_empty() {
                    chunks.push(Chunk {
                        content: current_para.trim().to_string(),
                        index,
                        token_count: current_tokens,
                        boundary: ChunkBoundary::Paragraph,
                    });
                    index += 1;
                }
            }
        }

        chunks
    }

    /// Split on speaker turns (conversation format).
    fn chunk_by_turn(&self, content: &str) -> Vec<Chunk> {
        if content.is_empty() {
            return Vec::new();
        }

        // Split on patterns like "**User:**", "**Assistant:**", "User:", "Assistant:"
        let mut turns: Vec<String> = Vec::new();
        let mut current_turn = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("**") && trimmed.contains(":**")
                || (!trimmed.starts_with("**") && trimmed.contains(':') && trimmed.len() < 30)
            {
                if !current_turn.is_empty() {
                    turns.push(current_turn.trim().to_string());
                }
                current_turn = line.to_string();
            } else {
                current_turn.push('\n');
                current_turn.push_str(line);
            }
        }
        if !current_turn.trim().is_empty() {
            turns.push(current_turn.trim().to_string());
        }
        if turns.is_empty() {
            turns.push(content.to_string());
        }

        // Group turns into chunks by token budget
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_tokens = 0u32;
        let mut index = 0;

        for turn in &turns {
            let turn_tokens = self.token_counter.count_tokens(turn);
            if current_tokens + turn_tokens > self.target_tokens && !current.is_empty() {
                chunks.push(Chunk {
                    content: current.trim().to_string(),
                    index,
                    token_count: current_tokens,
                    boundary: ChunkBoundary::Turn,
                });
                index += 1;
                current = turn.to_string();
                current_tokens = turn_tokens;
            } else {
                if !current.is_empty() {
                    current.push('\n');
                }
                current.push_str(turn);
                current_tokens += turn_tokens;
            }
        }

        if !current.trim().is_empty() {
            chunks.push(Chunk {
                content: current.trim().to_string(),
                index,
                token_count: current_tokens,
                boundary: ChunkBoundary::Turn,
            });
        }

        chunks
    }

    /// Split on sentence boundaries.
    fn chunk_by_sentence(&self, content: &str) -> Vec<Chunk> {
        if content.is_empty() {
            return Vec::new();
        }

        // Split on sentence endings (. ! ? followed by space or uppercase)
        let sentences: Vec<&str> = self.split_sentences(content);

        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_tokens = 0u32;
        let mut index = 0;

        for sentence in &sentences {
            let trimmed = sentence.trim();
            if trimmed.is_empty() {
                continue;
            }
            let sentence_tokens = self.token_counter.count_tokens(trimmed);

            if current_tokens + sentence_tokens > self.target_tokens && !current.is_empty() {
                chunks.push(Chunk {
                    content: current.trim().to_string(),
                    index,
                    token_count: current_tokens,
                    boundary: ChunkBoundary::Sentence,
                });
                index += 1;
                current = trimmed.to_string();
                current_tokens = sentence_tokens;
            } else {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(trimmed);
                current_tokens += sentence_tokens;
            }
        }

        if !current.trim().is_empty() {
            chunks.push(Chunk {
                content: current.trim().to_string(),
                index,
                token_count: current_tokens,
                boundary: ChunkBoundary::Sentence,
            });
        }

        chunks
    }

    /// Split text into sentences (simple approach).
    fn split_sentences<'a>(&self, text: &'a str) -> Vec<&'a str> {
        let mut sentences = Vec::new();
        let mut start = 0;
        let bytes = text.as_bytes();

        for i in 0..text.len().saturating_sub(1) {
            let ch = bytes[i] as char;
            if (ch == '.' || ch == '!' || ch == '?')
                && (i + 1 >= text.len() || bytes[i + 1] == b' ' || bytes[i + 1] == b'\n')
            {
                let end = i + 1;
                sentences.push(&text[start..=end]);
                start = end + 1;
                // Skip whitespace after the sentence end
                while start < text.len() && text.as_bytes()[start].is_ascii_whitespace() {
                    start += 1;
                }
            }
        }

        if start < text.len() {
            sentences.push(&text[start..]);
        }

        if sentences.is_empty() {
            sentences.push(text);
        }

        sentences
    }

    /// Extract the last ~overlap_tokens from a chunk for sliding window overlap.
    fn extract_overlap(&self, content: &str) -> String {
        let words: Vec<&str> = content.split_whitespace().collect();
        let mut overlap = String::new();
        let mut tokens = 0u32;

        for word in words.iter().rev() {
            let word_tokens = self.token_counter.count_tokens(word);
            if tokens + word_tokens > self.overlap_tokens {
                break;
            }
            if overlap.is_empty() {
                overlap = word.to_string();
            } else {
                overlap = format!("{} {}", word, overlap);
            }
            tokens += word_tokens;
        }

        overlap
    }
}

// ─── Convenience ──────────────────────────────────────────────

/// Convenience: chunk content and produce a Vec of chunk contents.
pub fn chunk_text(content: &str, target_tokens: u32) -> Vec<String> {
    let chunker = Chunker::new(target_tokens);
    chunker.chunk(content)
        .into_iter()
        .map(|c| c.content)
        .collect()
}

/// Convenience: chunk with auto-detected strategy.
pub fn chunk_text_auto(content: &str) -> Vec<String> {
    let chunker = Chunker::default_chunker();
    chunker.chunk(content)
        .into_iter()
        .map(|c| c.content)
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_empty() {
        let chunker = Chunker::new(512);
        assert!(chunker.chunk("").is_empty());
    }

    #[test]
    fn test_chunk_short_content() {
        let chunker = Chunker::new(512);
        let chunks = chunker.chunk("Hello, this is a short text.");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello, this is a short text.");
    }

    #[test]
    fn test_chunk_by_size() {
        // Create content that's ~100 tokens (400 chars / 4 = 100)
        let content = "word ".repeat(200); // ~1000 chars = ~250 tokens
        let chunker = Chunker::new(50); // small target
        let chunks = chunker.chunk(&content);

        assert!(chunks.len() >= 3, "should produce several chunks");
        assert_eq!(chunks[0].boundary, ChunkBoundary::SizeHard);
    }

    #[test]
    fn test_chunk_by_structure() {
        let content = "\
# Introduction
This is the intro paragraph.

# Methods
This describes methods.

# Results
The results section is long.
";
        let chunker = Chunker::new(512);
        let chunks = chunker.chunk(content);

        assert!(chunks.len() >= 3, "3 headings should produce at least 3 chunks");
        assert_eq!(chunks[0].boundary, ChunkBoundary::Heading);
    }

    #[test]
    fn test_strategy_detection() {
        assert_eq!(
            ChunkStrategy::detect("# Heading\nSome text"),
            ChunkStrategy::ByStructure
        );
        // Use conversation format that's detected as turns
        let conv = "**User:** Hello\n**Assistant:** Hi there!\n**User:** How are you?";
        let turn_or_structure = ChunkStrategy::detect(conv);
        assert!(turn_or_structure == ChunkStrategy::ByTurn || turn_or_structure == ChunkStrategy::ByStructure);
        // Long text with sentences should be detected as sentence-based
        let long_text = "This is a long text with multiple sentences. It has several periods. Here is another one. And a fourth sentence for good measure. The quick brown fox jumps over the lazy dog near the riverbank. Additional padding to push past the 200 character threshold. Making sure we cross the detection limit easily.";
        assert_eq!(
            ChunkStrategy::detect(long_text),
            ChunkStrategy::BySentence
        );
        assert_eq!(
            ChunkStrategy::detect("just some words without structure or punctuation"),
            ChunkStrategy::BySize
        );
    }

    #[test]
    fn test_overlap() {
        let chunker = Chunker::new(30); // small chunks
        let content = "a b c d e f g h i j k l m n o p q r s t u v w x y z";
        let chunks = chunker.chunk(content);

        if chunks.len() > 1 {
            // Check that we have sequence numbers
            assert_eq!(chunks[0].index, 0);
            assert_eq!(chunks[1].index, 1);
        }
    }

    #[test]
    fn test_split_sentences() {
        let chunker = Chunker::new(512);
        let text = "Hello world. This is a test. Goodbye!";
        let sentences = chunker.split_sentences(text);
        assert_eq!(sentences.len(), 3);
        assert!(sentences[0].contains("Hello world"));
        assert!(sentences[1].contains("This is a test"));
        assert!(sentences[2].contains("Goodbye"));
    }

    #[test]
    fn test_chunk_text_convenience() {
        let result = chunk_text("Hello world this is a test", 512);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "Hello world this is a test");
    }

    #[test]
    fn test_chunk_text_auto() {
        // Auto-detects structure, heading + content merge into one chunk
        let result = chunk_text_auto("# Heading\nSome content");
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("# Heading"));
    }
}
