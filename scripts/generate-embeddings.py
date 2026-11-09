#!/usr/bin/env python3
"""
Generate embeddings for incidents and documentation in the database.

This script reads incidents and documentation from PostgreSQL, generates
OpenAI embeddings, and stores them in the embeddings table for RAG retrieval.
"""

import os
import sys
import asyncio
from typing import List, Dict, Any

import psycopg
from openai import AsyncOpenAI


async def main():
    database_url = os.getenv("DATABASE_URL")
    if not database_url:
        print("ERROR: DATABASE_URL environment variable required")
        sys.exit(1)

    openai_api_key = os.getenv("OPENAI_API_KEY")
    if not openai_api_key:
        print("ERROR: OPENAI_API_KEY environment variable required")
        sys.exit(1)

    client = AsyncOpenAI(api_key=openai_api_key)

    async with await psycopg.AsyncConnection.connect(database_url) as conn:
        print("Generating embeddings for incidents...")
        await generate_incident_embeddings(conn, client)

        print("Generating embeddings for documentation...")
        await generate_documentation_embeddings(conn, client)

    print("Embedding generation complete!")


async def generate_incident_embeddings(conn, client: AsyncOpenAI):
    async with conn.cursor() as cur:
        await cur.execute("SELECT id, incident_id, title, description, incident_type, severity, root_cause, resolution, payment_network FROM incidents")
        incidents = await cur.fetchall()

    print(f"Found {len(incidents)} incidents")

    for incident in incidents:
        incident_id, ext_id, title, desc, inc_type, severity, root_cause, resolution, network = incident

        # Format incident for embedding
        text = f"""Incident: {title}
Type: {inc_type}
Severity: {severity}
Description: {desc}
Root Cause: {root_cause or 'Unknown'}
Resolution: {resolution or 'Ongoing'}
Network: {network or 'N/A'}"""

        # Generate embedding
        response = await client.embeddings.create(
            model="text-embedding-ada-002",
            input=text
        )
        embedding = response.data[0].embedding

        # Store in database
        async with conn.cursor() as cur:
            await cur.execute("""
                INSERT INTO embeddings (content_type, content_id, embedding, content_text, metadata)
                VALUES (%s, %s, %s, %s, %s)
            """, (
                "incident",
                str(incident_id),
                embedding,
                text,
                psycopg.types.json.Json({
                    "incident_id": ext_id,
                    "incident_type": inc_type,
                    "severity": severity,
                    "payment_network": network
                })
            ))

        await conn.commit()
        print(f"  Generated embedding for incident: {ext_id}")


async def generate_documentation_embeddings(conn, client: AsyncOpenAI):
    async with conn.cursor() as cur:
        await cur.execute("SELECT id, doc_id, title, content, doc_type, payment_network FROM documentation")
        docs = await cur.fetchall()

    print(f"Found {len(docs)} documentation pages")

    for doc in docs:
        doc_id, ext_id, title, content, doc_type, network = doc

        # Format documentation for embedding
        text = f"""{title} ({doc_type})

{content}

Payment Network: {network or 'General'}"""

        # Generate embedding
        response = await client.embeddings.create(
            model="text-embedding-ada-002",
            input=text
        )
        embedding = response.data[0].embedding

        # Store in database
        async with conn.cursor() as cur:
            await cur.execute("""
                INSERT INTO embeddings (content_type, content_id, embedding, content_text, metadata)
                VALUES (%s, %s, %s, %s, %s)
            """, (
                "documentation",
                str(doc_id),
                embedding,
                text,
                psycopg.types.json.Json({
                    "doc_id": ext_id,
                    "doc_type": doc_type,
                    "payment_network": network
                })
            ))

        await conn.commit()
        print(f"  Generated embedding for doc: {ext_id}")


if __name__ == "__main__":
    asyncio.run(main())
