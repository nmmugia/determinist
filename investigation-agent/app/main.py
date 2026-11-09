"""
AI-powered transaction investigation agent with LangGraph orchestration.

This FastAPI service receives investigation requests from the Go API service,
orchestrates multi-step agent workflows using LangGraph, performs RAG retrieval
for historical incidents and documentation, and returns investigation reports.
"""

import os
from contextlib import asynccontextmanager

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field
from typing import Dict, List, Optional, Any

from opentelemetry import trace
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor


# Pydantic models for request/response
class InvestigationRequest(BaseModel):
    investigation_id: str
    query: str
    transaction_data: Dict[str, Any]
    settlement_data: Dict[str, Any]
    audit_logs: List[Dict[str, Any]]
    dtre_replay_result: Optional[Dict[str, Any]] = None


class Findings(BaseModel):
    primary_issue: str
    confidence: float = Field(ge=0.0, le=1.0)
    summary: str


class Evidence(BaseModel):
    type: str
    source: str
    data: Optional[Dict[str, Any]] = None
    relevance: Optional[float] = None


class AgentStep(BaseModel):
    node: str
    duration_ms: int
    output: str


class AgentTrace(BaseModel):
    steps: List[AgentStep]
    total_duration_ms: int


class InvestigationResponse(BaseModel):
    status: str
    findings: Findings
    evidence: List[Evidence]
    recommendations: List[str]
    agent_trace: AgentTrace


# Initialize OpenTelemetry
def init_telemetry():
    """Initialize OpenTelemetry tracing."""
    otel_endpoint = os.getenv("OTEL_EXPORTER_OTLP_ENDPOINT", "localhost:4317")

    resource = Resource.create({
        "service.name": os.getenv("OTEL_SERVICE_NAME", "investigation-agent"),
    })

    provider = TracerProvider(resource=resource)
    processor = BatchSpanProcessor(
        OTLPSpanExporter(endpoint=otel_endpoint, insecure=True)
    )
    provider.add_span_processor(processor)
    trace.set_tracer_provider(provider)


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Lifespan context manager for startup and shutdown."""
    # Startup
    init_telemetry()
    print("Investigation agent started")

    yield

    # Shutdown
    print("Investigation agent shutting down")


# Create FastAPI app
app = FastAPI(
    title="Transaction Investigation Agent",
    description="AI-powered investigation agent using LangGraph and RAG",
    version="0.1.0",
    lifespan=lifespan,
)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Instrument FastAPI with OpenTelemetry
FastAPIInstrumentor.instrument_app(app)


@app.get("/health")
async def health_check():
    """Health check endpoint."""
    return {
        "status": "healthy",
        "service": "investigation-agent",
    }


@app.post("/ai/v1/investigate", response_model=InvestigationResponse)
async def investigate(request: InvestigationRequest):
    """
    Run AI investigation workflow using LangGraph agent.

    This endpoint receives investigation context from the Go API service,
    orchestrates a multi-step agent workflow, performs RAG retrieval,
    and returns a comprehensive investigation report.
    """
    tracer = trace.get_tracer(__name__)

    from app.agent.graph import run_investigation

    with tracer.start_as_current_span("investigate") as span:
        span.set_attribute("investigation.id", request.investigation_id)
        span.set_attribute("investigation.query", request.query)

        try:
            # Run the LangGraph investigation workflow
            result = await run_investigation(
                investigation_id=request.investigation_id,
                query=request.query,
                transaction_data=request.transaction_data,
                settlement_data=request.settlement_data,
                audit_logs=request.audit_logs,
                dtre_replay_result=request.dtre_replay_result,
            )

            return InvestigationResponse(
                status=result["status"],
                findings=Findings(**result["findings"]),
                evidence=[Evidence(**e) for e in result["evidence"]],
                recommendations=result["recommendations"],
                agent_trace=AgentTrace(**result["agent_trace"]),
            )
        except Exception as e:
            span.record_error(e)
            return InvestigationResponse(
                status="error",
                findings=Findings(
                    primary_issue="Investigation failed",
                    confidence=0.0,
                    summary=f"Error during investigation: {str(e)}",
                ),
                evidence=[],
                recommendations=["Check logs for error details", "Retry investigation"],
                agent_trace=AgentTrace(steps=[], total_duration_ms=0),
            )


@app.get("/")
async def root():
    """Root endpoint with service info."""
    return {
        "service": "investigation-agent",
        "version": "0.1.0",
        "description": "AI-powered transaction investigation using LangGraph",
    }


if __name__ == "__main__":
    import uvicorn

    port = int(os.getenv("PORT", "8000"))
    uvicorn.run(app, host="0.0.0.0", port=port)
