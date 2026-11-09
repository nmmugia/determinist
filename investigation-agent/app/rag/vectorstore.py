"""
Vector store implementation using pgvector for RAG similarity search.

This module handles embedding storage and retrieval for historical incidents
and payment provider documentation, enabling semantic search for the investigation agent.
"""

import json
from typing import Dict, List, Optional, Any
from datetime import datetime

import psycopg
from psycopg.rows import dict_row
from pgvector.psycopg import register_vector


class PgVectorStore:
    """PostgreSQL vector store using pgvector extension."""

    def __init__(self, database_url: str):
        """
        Initialize vector store with database connection.

        Args:
            database_url: PostgreSQL connection string
        """
        self.database_url = database_url
        self._pool = None

    async def connect(self):
        """Establish connection pool and register pgvector types."""
        self._pool = psycopg.AsyncConnectionPool(
            self.database_url,
            min_size=2,
            max_size=10,
            open=True,
        )

        # Register pgvector types
        async with self._pool.connection() as conn:
            await register_vector(conn)

    async def close(self):
        """Close connection pool."""
        if self._pool:
            await self._pool.close()

    async def add_embedding(
        self,
        content_type: str,
        content_id: str,
        embedding: List[float],
        content_text: str,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> str:
        """
        Store an embedding in the vector database.

        Args:
            content_type: Type of content ('incident' or 'documentation')
            content_id: UUID of the content record
            embedding: Vector embedding (1536 dimensions for OpenAI ada-002)
            content_text: Text content for retrieval
            metadata: Optional metadata for filtering

        Returns:
            UUID of the created embedding record
        """
        query = """
            INSERT INTO embeddings (content_type, content_id, embedding, content_text, metadata)
            VALUES (%s, %s, %s, %s, %s)
            RETURNING id
        """

        async with self._pool.connection() as conn:
            async with conn.cursor() as cur:
                await cur.execute(
                    query,
                    (
                        content_type,
                        content_id,
                        embedding,
                        content_text,
                        json.dumps(metadata) if metadata else None,
                    ),
                )
                row = await cur.fetchone()
                await conn.commit()
                return str(row[0])

    async def similarity_search(
        self,
        query_embedding: List[float],
        content_type: str,
        top_k: int = 10,
        filters: Optional[Dict[str, str]] = None,
        min_similarity: float = 0.0,
    ) -> List[Dict[str, Any]]:
        """
        Search for similar content using cosine similarity.

        Args:
            query_embedding: Query vector
            content_type: Type of content to search ('incident' or 'documentation')
            top_k: Number of results to return
            filters: Optional metadata filters (e.g., {"payment_network": "VISA"})
            min_similarity: Minimum similarity threshold (0-1)

        Returns:
            List of matching documents with similarity scores
        """
        # Build filter clause
        filter_conditions = ["content_type = %s"]
        params: List[Any] = [content_type]

        if filters:
            for key, value in filters.items():
                filter_conditions.append(f"metadata->>'{{key}}' = %s")
                params.append(value)

        filter_clause = " AND ".join(filter_conditions)

        # Query with cosine distance (1 - cosine_distance = cosine_similarity)
        query = f"""
            SELECT
                id,
                content_id,
                content_text,
                metadata,
                1 - (embedding <=> %s::vector) AS similarity
            FROM embeddings
            WHERE {filter_clause}
                AND 1 - (embedding <=> %s::vector) >= %s
            ORDER BY embedding <=> %s::vector
            LIMIT %s
        """

        # Add query embedding to params (used 3 times in the query)
        params.extend([query_embedding, query_embedding, min_similarity, query_embedding, top_k])

        async with self._pool.connection() as conn:
            async with conn.cursor(row_factory=dict_row) as cur:
                await cur.execute(query, params)
                rows = await cur.fetchall()

                return [
                    {
                        "id": str(row["id"]),
                        "content_id": str(row["content_id"]),
                        "content_text": row["content_text"],
                        "metadata": json.loads(row["metadata"]) if row["metadata"] else {},
                        "similarity": float(row["similarity"]),
                    }
                    for row in rows
                ]

    async def get_incident_by_id(self, incident_id: str) -> Optional[Dict[str, Any]]:
        """Retrieve full incident details by ID."""
        query = "SELECT * FROM incidents WHERE id = %s"

        async with self._pool.connection() as conn:
            async with conn.cursor(row_factory=dict_row) as cur:
                await cur.execute(query, (incident_id,))
                row = await cur.fetchone()

                if row:
                    return {
                        "id": str(row["id"]),
                        "incident_id": row["incident_id"],
                        "title": row["title"],
                        "description": row["description"],
                        "incident_type": row["incident_type"],
                        "severity": row["severity"],
                        "root_cause": row["root_cause"],
                        "resolution": row["resolution"],
                        "payment_network": row["payment_network"],
                        "affected_transaction_count": row["affected_transaction_count"],
                        "occurred_at": row["occurred_at"].isoformat() if row["occurred_at"] else None,
                        "resolved_at": row["resolved_at"].isoformat() if row["resolved_at"] else None,
                    }
                return None

    async def get_documentation_by_id(self, doc_id: str) -> Optional[Dict[str, Any]]:
        """Retrieve full documentation details by ID."""
        query = "SELECT * FROM documentation WHERE id = %s"

        async with self._pool.connection() as conn:
            async with conn.cursor(row_factory=dict_row) as cur:
                await cur.execute(query, (doc_id,))
                row = await cur.fetchone()

                if row:
                    return {
                        "id": str(row["id"]),
                        "doc_id": row["doc_id"],
                        "title": row["title"],
                        "content": row["content"],
                        "doc_type": row["doc_type"],
                        "payment_network": row["payment_network"],
                        "version": row["version"],
                        "url": row["url"],
                    }
                return None

    async def batch_add_embeddings(
        self,
        embeddings: List[Dict[str, Any]],
    ) -> int:
        """
        Batch insert multiple embeddings.

        Args:
            embeddings: List of embedding dicts with keys:
                content_type, content_id, embedding, content_text, metadata

        Returns:
            Number of embeddings inserted
        """
        query = """
            INSERT INTO embeddings (content_type, content_id, embedding, content_text, metadata)
            VALUES (%s, %s, %s, %s, %s)
        """

        records = [
            (
                e["content_type"],
                e["content_id"],
                e["embedding"],
                e["content_text"],
                json.dumps(e.get("metadata")) if e.get("metadata") else None,
            )
            for e in embeddings
        ]

        async with self._pool.connection() as conn:
            async with conn.cursor() as cur:
                await cur.executemany(query, records)
                await conn.commit()
                return len(records)
