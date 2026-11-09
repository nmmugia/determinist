# Setup Guide

Complete setup instructions to get the AI-powered transaction investigation platform running.

## Prerequisites

1. **Docker & Docker Compose** installed
2. **OpenAI API key** (get one at https://platform.openai.com/api-keys)
3. (Optional) **LangSmith API key** for LLM tracing

## Step-by-Step Setup

### 1. Configure Environment

```bash
# Create environment file
cp .env.example .env

# Edit .env and add your OpenAI API key
nano .env  # or use your preferred editor
```

Required in `.env`:
```
OPENAI_API_KEY=sk-your-actual-openai-key-here
```

### 2. Start Services

```bash
# Build and start all services
docker-compose up --build -d

# Check services are running
docker-compose ps
```

You should see 4 services:
- `transaction-investigation-db` (PostgreSQL)
- `transaction-investigation-api` (Go)
- `transaction-investigation-ai` (Python)
- `transaction-investigation-jaeger` (Tracing)

### 3. Generate Sample Data

```bash
# Install Go dependencies for data generator
cd scripts
go mod init datagen
go get github.com/google/uuid github.com/lib/pq

# Run data generator
export DATABASE_URL="postgresql://dbuser:dbpass@localhost:5432/transaction_investigation"
go run generate-data.go
```

This creates:
- 100 sample transactions (across VISA, Mastercard, ACH, Wire)
- 30 days of settlement reports
- 3 historical incidents
- 3 documentation pages

### 4. Generate Embeddings for RAG

```bash
# Install Python dependencies
pip install openai psycopg[binary]

# Set environment variables
export DATABASE_URL="postgresql://dbuser:dbpass@localhost:5432/transaction_investigation"
export OPENAI_API_KEY="sk-your-key-here"

# Generate embeddings
python generate-embeddings.py
```

This generates OpenAI embeddings for all incidents and documentation, enabling RAG-powered similarity search.

### 5. Verify Setup

**Health checks:**
```bash
# Go API
curl http://localhost:8080/health
# Expected: {"status":"healthy","service":"transaction-api"}

# Python AI service
curl http://localhost:8000/health
# Expected: {"status":"healthy","service":"investigation-agent"}
```

**Jaeger tracing:**
```bash
# Open in browser
open http://localhost:16686
```

### 6. Run Your First Investigation

```bash
curl -X POST http://localhost:8080/api/v1/investigations \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Why is transaction TXN-20261109-001 successful internally but missing from settlement report?",
    "transaction_id": "TXN-20261109-001"
  }'
```

Expected response structure:
```json
{
  "investigation_id": "uuid",
  "status": "success",
  "findings": {
    "primary_issue": "...",
    "confidence": 0.92,
    "summary": "..."
  },
  "evidence": [...],
  "recommendations": [...]
}
```

## Troubleshooting

**Service won't start:**
```bash
# Check logs
docker-compose logs go-api
docker-compose logs python-ai

# Rebuild from scratch
docker-compose down -v
docker-compose up --build
```

**Database connection errors:**
```bash
# Verify PostgreSQL is running
docker-compose ps postgres

# Check database accessibility
docker exec -it transaction-investigation-db psql -U dbuser -d transaction_investigation -c "SELECT COUNT(*) FROM transactions;"
```

**OpenAI API errors:**
- Verify your API key is correct in `.env`
- Check you have credits available: https://platform.openai.com/usage
- Check rate limits: default tier allows 3 RPM for embeddings

**Empty investigation results:**
- Ensure you ran both data generation scripts
- Verify embeddings exist: `SELECT COUNT(*) FROM embeddings;`
- Check that transaction exists in database

## Development Workflow

**View logs:**
```bash
docker-compose logs -f go-api       # Go service
docker-compose logs -f python-ai    # Python service
```

**Database access:**
```bash
docker exec -it transaction-investigation-db psql -U dbuser -d transaction_investigation
```

**Restart single service:**
```bash
docker-compose restart go-api
docker-compose restart python-ai
```

**Local development (no Docker):**

See [README.md](README.md) section "Local Development Without Docker"

## Next Steps

- Add more sample data for richer investigations
- Customize the LangGraph agent nodes ([nodes.py](investigation-agent/app/agent/nodes.py))
- Add authentication to the Go API
- Deploy to production (Kubernetes, Cloud Run, etc.)
