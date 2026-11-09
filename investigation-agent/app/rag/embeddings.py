"""
Embedding generation using OpenAI API.

Handles text chunking and batch embedding generation for incidents and documentation.
"""

import os
from typing import List, Dict, Any, Optional
from openai import AsyncOpenAI


class EmbeddingGenerator:
    """Generate embeddings using OpenAI API."""

    def __init__(self, api_key: Optional[str] = None):
        """
        Initialize embedding generator.

        Args:
            api_key: OpenAI API key (defaults to OPENAI_API_KEY env var)
        """
        self.client = AsyncOpenAI(api_key=api_key or os.getenv("OPENAI_API_KEY"))
        self.model = "text-embedding-ada-002"
        self.dimension = 1536

    async def generate_embedding(self, text: str) -> List[float]:
        """
        Generate embedding for a single text.

        Args:
            text: Input text

        Returns:
            Embedding vector (1536 dimensions)
        """
        response = await self.client.embeddings.create(
            model=self.model,
            input=text,
        )
        return response.data[0].embedding

    async def generate_embeddings_batch(self, texts: List[str]) -> List[List[float]]:
        """
        Generate embeddings for multiple texts in a single API call.

        Args:
            texts: List of input texts (max 2048 per batch)

        Returns:
            List of embedding vectors
        """
        if len(texts) > 2048:
            raise ValueError("Maximum 2048 texts per batch")

        response = await self.client.embeddings.create(
            model=self.model,
            input=texts,
        )

        # Sort by index to ensure correct order
        sorted_embeddings = sorted(response.data, key=lambda x: x.index)
        return [item.embedding for item in sorted_embeddings]

    def chunk_text(
        self,
        text: str,
        chunk_size: int = 500,
        overlap: int = 50,
    ) -> List[str]:
        """
        Split text into overlapping chunks for embedding.

        Args:
            text: Input text
            chunk_size: Target chunk size in tokens (approximate)
            overlap: Overlap between chunks in tokens (approximate)

        Returns:
            List of text chunks
        """
        # Simple word-based chunking (rough approximation of token count)
        words = text.split()
        chunks = []

        i = 0
        while i < len(words):
            chunk = " ".join(words[i : i + chunk_size])
            chunks.append(chunk)
            i += chunk_size - overlap

        return chunks

    def prepare_incident_for_embedding(self, incident: Dict[str, Any]) -> str:
        """
        Format incident data into searchable text.

        Args:
            incident: Incident record

        Returns:
            Formatted text for embedding
        """
        parts = [
            f"Incident: {incident['title']}",
            f"Type: {incident['incident_type']}",
            f"Severity: {incident['severity']}",
            f"Description: {incident['description']}",
        ]

        if incident.get("root_cause"):
            parts.append(f"Root Cause: {incident['root_cause']}")

        if incident.get("resolution"):
            parts.append(f"Resolution: {incident['resolution']}")

        if incident.get("payment_network"):
            parts.append(f"Network: {incident['payment_network']}")

        return "\n".join(parts)

    def prepare_documentation_for_embedding(self, doc: Dict[str, Any]) -> List[str]:
        """
        Chunk documentation into searchable segments.

        Args:
            doc: Documentation record

        Returns:
            List of text chunks for embedding
        """
        header = f"{doc['title']} ({doc['doc_type']})"
        content = doc["content"]

        # Chunk the content
        chunks = self.chunk_text(content, chunk_size=500, overlap=50)

        # Prepend header to each chunk for context
        return [f"{header}\n\n{chunk}" for chunk in chunks]
