# AI-Powered Transaction Investigation Platform

A fintech operations platform that uses AI agents to investigate transaction discrepancies between internal records and external payment networks. Built with a hybrid Go/Python/Rust architecture featuring LangGraph agent orchestration, RAG-powered investigation, and deterministic transaction replay verification.

## Overview

Operations teams at banks manually investigate thousands of transaction discrepancies—transactions missing from settlement reports, status mismatches, duplicate processing, and timing issues. This platform automates investigation using an AI agent that:

- Queries transaction and settlement data from PostgreSQL
- Searches historical incidents via RAG (pgvector similarity search)
- Retrieves payment network documentation
- Analyzes DTRE (Deterministic Transaction Replay Engine) audit trails
- Synthesizes findings with GPT-4
- Generates actionable recommendations

**Example Query:** "Why is transaction TXN-20261109-001 successful internally but missing from settlement report?"

**AI Response:** Settlement timing mismatch (92% confidence) - transaction submitted at 23:58 UTC after the 23:00 cutoff, will appear in next settlement report.

## Architecture

```
Operator Query → Go API (8080) → PostgreSQL + pgvector
                      ↓
              Python AI Service (8000)
              • LangGraph workflow (7 nodes)
              • RAG retrieval
              • OpenAI GPT-4
```

## Technology Stack

- **Backend API**: Go 1.21+ with Gin, PostgreSQL driver, OpenTelemetry
- **AI Service**: Python 3.11+ with FastAPI, LangGraph, LangChain, pgvector
- **Database**: PostgreSQL 16 with pgvector extension
- **Embeddings & LLM**: OpenAI (text-embedding-ada-002, GPT-4)
- **Observability**: OpenTelemetry + Jaeger
- **Transaction Replay**: Rust DTRE library (FFI integration)


## Prerequisites

- Docker and Docker Compose
- OpenAI API key
- (Optional) LangSmith API key for LLM tracing

## Quick Start

### 1. Environment Setup

```bash
cp .env.example .env
# Edit .env and add your OpenAI API key
```

### 2. Start Services

```bash
docker-compose up --build
```

Services:
- PostgreSQL with pgvector (port 5432)
- Go API (port 8080)
- Python AI service (port 8000)
- Jaeger UI (port 16686)

### 3. Run Investigation

```bash
curl -X POST http://localhost:8080/api/v1/investigations \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Why is TXN-20261109-001 successful internally but missing from settlement report?",
    "transaction_id": "TXN-20261109-001"
  }'
```

## Project Structure

```
transaction-api/          # Go API service
  cmd/api-server/        # Main entry point
  internal/
    api/handlers/        # HTTP handlers
    repository/postgres/ # Database access
    ai/client/          # Python AI client

investigation-agent/      # Python AI service
  app/
    agent/              # LangGraph workflow
      graph.py          # Agent orchestration
      nodes.py          # 7 investigation nodes
    rag/                # RAG pipeline
      vectorstore.py    # pgvector integration
      embeddings.py     # OpenAI embeddings

src/ffi/                  # Rust DTRE FFI layer
migrations/               # Database migrations
```

## LangGraph Agent Workflow

7-node investigation pipeline:

1. **analyze_transaction** - Parse data, identify discrepancy type
2. **check_settlement_timing** - Compare timestamps vs cutoffs
3. **search_similar_incidents** - Vector search historical incidents
4. **retrieve_documentation** - Vector search network docs
5. **analyze_dtre_replay** - Interpret replay verification
6. **synthesize_findings** - LLM correlates evidence
7. **generate_recommendations** - Produce action items

## Database Schema

**Core tables:**
- `transactions` - Internal records
- `settlements` - Daily reports from payment networks
- `settlement_items` - Individual settlement line items
- `audit_logs` - DTRE audit trail
- `incidents` - Historical incidents (RAG)
- `documentation` - Provider documentation (RAG)
- `embeddings` - Vector embeddings (1536d, pgvector)
- `investigations` - Investigation tracking

## Observability

Jaeger UI: `http://localhost:16686`

Distributed traces span:
- Go API → PostgreSQL
- Go API → Python AI service  
- Python AI → OpenAI API
- LangGraph agent execution

## Development

### Local Development (without Docker)

**Go API:**
```bash
cd transaction-api
export DATABASE_URL="postgresql://dbuser:dbpass@localhost:5432/transaction_investigation"
export PYTHON_AI_SERVICE_URL="http://localhost:8000"
go run cmd/api-server/main.go
```

**Python AI:**
```bash
cd investigation-agent
pip install -r requirements.txt
export DATABASE_URL="postgresql://dbuser:dbpass@localhost:5432/transaction_investigation"
export OPENAI_API_KEY="sk-..."
uvicorn app.main:app --port 8000
```

### Testing

```bash
# Go tests
cd transaction-api && go test ./...

# Python tests
cd investigation-agent && pytest
```

## API Reference

### POST /api/v1/investigations

**Request:**
```json
{
  "query": "Why is TXN-X missing?",
  "transaction_id": "TXN-X"
}
```

**Response:**
```json
{
  "investigation_id": "uuid",
  "status": "success",
  "findings": {
    "primary_issue": "Settlement timing mismatch",
    "confidence": 0.92,
    "summary": "..."
  },
  "evidence": [...],
  "recommendations": [...],
  "agent_trace": {...}
}
```

## Rust DTRE Library

The `src/` directory contains the Deterministic Transaction Replay Engine - a Rust library for cryptographically verifiable transaction replay. It provides:

- Deterministic execution with Blake3 hashing
- Rule versioning and migration analysis
- Parallel replay with consistency guarantees
- FFI layer ([src/ffi/mod.rs](src/ffi/mod.rs)) for Go integration

See `examples/bank_transfer.rs` for usage.

## Production Considerations

- **Security**: JWT auth, rate limiting, input validation
- **Error Handling**: Circuit breakers, exponential backoff
- **Performance**: Connection pooling, database indexes, LLM streaming
- **Observability**: Structured logging, metrics, distributed tracing

## License

MIT

## Contributing

Contributions welcome! Open an issue or PR.
