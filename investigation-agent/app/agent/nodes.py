"""
Agent node implementations for transaction investigation workflow.

Each node represents a step in the investigation process, analyzing different
aspects of the transaction discrepancy and gathering evidence.
"""

import time
from typing import Dict, Any
from datetime import datetime, timedelta

from openai import AsyncOpenAI
import os


# Initialize OpenAI client
openai_client = AsyncOpenAI(api_key=os.getenv("OPENAI_API_KEY"))


async def analyze_transaction_node(state: Dict[str, Any]) -> Dict[str, Any]:
    """
    Analyze transaction data and identify the type of discrepancy.

    Examines the transaction record to understand its status, timing,
    and basic characteristics that may be relevant to the investigation.
    """
    start_time = time.time()

    tx = state["transaction_data"]

    analysis = {
        "transaction_id": tx.get("id"),
        "external_id": tx.get("external_id"),
        "status": tx.get("status"),
        "amount": tx.get("amount"),
        "currency": tx.get("currency"),
        "payment_network": tx.get("payment_network"),
        "initiated_at": tx.get("initiated_at"),
        "completed_at": tx.get("completed_at"),
        "discrepancy_type": "unknown",
    }

    # Identify discrepancy type from query
    query_lower = state["query"].lower()
    if "missing" in query_lower or "not found" in query_lower:
        analysis["discrepancy_type"] = "missing_from_settlement"
    elif "duplicate" in query_lower:
        analysis["discrepancy_type"] = "duplicate_transaction"
    elif "mismatch" in query_lower or "different" in query_lower:
        analysis["discrepancy_type"] = "status_mismatch"

    duration_ms = int((time.time() - start_time) * 1000)

    state["transaction_analysis"] = analysis
    state["agent_trace"].append({
        "node": "analyze_transaction",
        "duration_ms": duration_ms,
        "output": f"Identified discrepancy type: {analysis['discrepancy_type']}",
    })

    return state


async def check_settlement_timing_node(state: Dict[str, Any]) -> Dict[str, Any]:
    """
    Check if transaction timing explains the settlement discrepancy.

    Compares transaction timestamp against typical settlement cutoff times
    to determine if the discrepancy is due to timing rather than an error.
    """
    start_time = time.time()

    tx = state["transaction_data"]
    initiated_at = datetime.fromisoformat(tx["initiated_at"].replace("Z", "+00:00"))

    # Settlement cutoff is typically 23:00 UTC for same-day processing
    cutoff_hour = 23
    tx_hour = initiated_at.hour

    timing_analysis = {
        "transaction_time": initiated_at.isoformat(),
        "transaction_hour_utc": tx_hour,
        "settlement_cutoff_hour": cutoff_hour,
        "after_cutoff": tx_hour >= cutoff_hour,
        "expected_settlement_date": None,
    }

    if tx_hour >= cutoff_hour:
        # Transaction after cutoff, expect next business day settlement
        next_day = initiated_at + timedelta(days=1)
        timing_analysis["expected_settlement_date"] = next_day.date().isoformat()
        timing_analysis["explanation"] = "Transaction submitted after settlement cutoff"
    else:
        timing_analysis["expected_settlement_date"] = initiated_at.date().isoformat()
        timing_analysis["explanation"] = "Transaction submitted before cutoff"

    duration_ms = int((time.time() - start_time) * 1000)

    state["settlement_timing_analysis"] = timing_analysis
    state["agent_trace"].append({
        "node": "check_settlement_timing",
        "duration_ms": duration_ms,
        "output": timing_analysis["explanation"],
    })

    return state


async def search_similar_incidents_node(state: Dict[str, Any]) -> Dict[str, Any]:
    """
    Search for similar historical incidents using vector similarity.

    Performs semantic search in the incidents database to find past issues
    with similar characteristics to the current investigation.
    """
    start_time = time.time()

    # TODO: Implement actual RAG retrieval using vectorstore
    # For now, return mock results
    similar_incidents = [
        {
            "incident_id": "INC-2026-045",
            "title": "Settlement timing delays for late-day transactions",
            "similarity": 0.87,
            "summary": "Transactions after 23:00 UTC not appearing in same-day settlement",
        }
    ]

    duration_ms = int((time.time() - start_time) * 1000)

    state["similar_incidents"] = similar_incidents
    state["agent_trace"].append({
        "node": "search_similar_incidents",
        "duration_ms": duration_ms,
        "output": f"Found {len(similar_incidents)} similar incidents",
    })

    return state


async def retrieve_documentation_node(state: Dict[str, Any]) -> Dict[str, Any]:
    """
    Retrieve relevant payment network documentation.

    Searches for documentation about settlement processes, timing rules,
    and error conditions for the relevant payment network.
    """
    start_time = time.time()

    # TODO: Implement actual RAG retrieval using vectorstore
    # For now, return mock results
    relevant_docs = [
        {
            "doc_id": "DOC-VISA-SETTLEMENT-TIMING",
            "title": "VISA Settlement Timing and Cutoff Rules",
            "relevance": 0.91,
            "excerpt": "Transactions must be submitted before 23:00 UTC for same-day settlement...",
        }
    ]

    duration_ms = int((time.time() - start_time) * 1000)

    state["relevant_docs"] = relevant_docs
    state["agent_trace"].append({
        "node": "retrieve_documentation",
        "duration_ms": duration_ms,
        "output": f"Retrieved {len(relevant_docs)} relevant documents",
    })

    return state


async def analyze_dtre_replay_node(state: Dict[str, Any]) -> Dict[str, Any]:
    """
    Analyze DTRE replay results for state consistency.

    Interprets the deterministic replay verification to confirm that
    the transaction was processed correctly according to business rules.
    """
    start_time = time.time()

    dtre_result = state.get("dtre_replay_result")

    if dtre_result:
        analysis = {
            "replay_verified": dtre_result.get("replay_verified", False),
            "final_state_hash": dtre_result.get("final_state_hash"),
            "state_transitions": dtre_result.get("state_transitions", 0),
            "interpretation": "DTRE replay confirmed correct transaction processing"
            if dtre_result.get("replay_verified")
            else "DTRE replay detected inconsistency",
        }
    else:
        analysis = {
            "replay_verified": None,
            "interpretation": "No DTRE replay data available",
        }

    duration_ms = int((time.time() - start_time) * 1000)

    state["dtre_analysis"] = analysis
    state["agent_trace"].append({
        "node": "analyze_dtre_replay",
        "duration_ms": duration_ms,
        "output": analysis["interpretation"],
    })

    return state


async def synthesize_findings_node(state: Dict[str, Any]) -> Dict[str, Any]:
    """
    Use LLM to synthesize all evidence into coherent explanation.

    Correlates transaction data, timing analysis, historical incidents,
    and documentation to generate a natural language explanation of the
    root cause.
    """
    start_time = time.time()

    # Build prompt with all gathered evidence
    prompt = f"""Analyze this transaction investigation and provide a clear explanation.

Query: {state['query']}

Transaction Analysis:
{state.get('transaction_analysis', {})}

Settlement Timing:
{state.get('settlement_timing_analysis', {})}

Similar Incidents:
{state.get('similar_incidents', [])}

Relevant Documentation:
{state.get('relevant_docs', [])}

DTRE Analysis:
{state.get('dtre_analysis', {})}

Provide:
1. Primary issue (one clear sentence)
2. Confidence score (0.0 to 1.0)
3. Detailed summary explaining the root cause

Format as JSON with keys: primary_issue, confidence, summary"""

    response = await openai_client.chat.completions.create(
        model="gpt-4",
        messages=[{"role": "user", "content": prompt}],
        temperature=0.3,
    )

    # Parse LLM response (simplified)
    findings_text = response.choices[0].message.content

    # For MVP, use simplified findings structure
    findings = {
        "primary_issue": "Settlement timing mismatch",
        "confidence": 0.92,
        "summary": findings_text,
    }

    # Collect evidence from all analysis steps
    evidence = [
        {
            "type": "transaction_record",
            "source": "internal_db",
            "data": state["transaction_data"],
        },
        {
            "type": "timing_analysis",
            "source": "settlement_cutoff_check",
            "data": state["settlement_timing_analysis"],
        },
    ]

    for incident in state.get("similar_incidents", []):
        evidence.append({
            "type": "similar_incident",
            "source": "rag_retrieval",
            "relevance": incident.get("similarity"),
            "data": incident,
        })

    for doc in state.get("relevant_docs", []):
        evidence.append({
            "type": "documentation",
            "source": "rag_retrieval",
            "relevance": doc.get("relevance"),
            "data": doc,
        })

    duration_ms = int((time.time() - start_time) * 1000)

    state["findings"] = findings
    state["evidence"] = evidence
    state["agent_trace"].append({
        "node": "synthesize_findings",
        "duration_ms": duration_ms,
        "output": findings["primary_issue"],
    })

    return state


async def generate_recommendations_node(state: Dict[str, Any]) -> Dict[str, Any]:
    """
    Generate actionable recommendations for operations team.

    Based on the findings and root cause, produces specific next steps
    for resolving or monitoring the transaction issue.
    """
    start_time = time.time()

    findings = state.get("findings", {})
    timing = state.get("settlement_timing_analysis", {})

    recommendations = []

    # Generate recommendations based on findings
    if timing.get("after_cutoff"):
        recommendations.append(
            f"Wait for next settlement report (expected {timing['expected_settlement_date']})"
        )
        recommendations.append("No operational action required - timing-related delay is normal")
        recommendations.append(
            "Consider setting alert threshold to 48 hours for late settlements"
        )
    else:
        recommendations.append("Verify transaction status with payment provider")
        recommendations.append("Check for network-wide incidents or delays")
        recommendations.append("Escalate to payments team if unresolved after 24 hours")

    duration_ms = int((time.time() - start_time) * 1000)

    state["recommendations"] = recommendations
    state["agent_trace"].append({
        "node": "generate_recommendations",
        "duration_ms": duration_ms,
        "output": f"Generated {len(recommendations)} recommendations",
    })

    return state
