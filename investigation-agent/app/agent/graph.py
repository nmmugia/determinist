"""
LangGraph agent workflow for transaction investigation.

This module defines the investigation agent's state schema and workflow graph.
The agent orchestrates a multi-step investigation process to analyze transaction
discrepancies and generate actionable reports.
"""

from typing import Dict, List, Optional, Any, TypedDict
from langgraph.graph import StateGraph, END


class InvestigationState(TypedDict):
    """State maintained throughout the investigation workflow."""

    # Input data
    investigation_id: str
    query: str
    transaction_data: Dict[str, Any]
    settlement_data: Dict[str, Any]
    audit_logs: List[Dict[str, Any]]
    dtre_replay_result: Optional[Dict[str, Any]]

    # Intermediate analysis results
    transaction_analysis: Optional[Dict[str, Any]]
    settlement_timing_analysis: Optional[Dict[str, Any]]
    similar_incidents: List[Dict[str, Any]]
    relevant_docs: List[Dict[str, Any]]
    dtre_analysis: Optional[Dict[str, Any]]

    # Final output
    findings: Optional[Dict[str, Any]]
    evidence: List[Dict[str, Any]]
    recommendations: List[str]

    # Execution metadata
    agent_trace: List[Dict[str, str]]
    current_step: str
    error: Optional[str]


def create_investigation_graph() -> StateGraph:
    """
    Create the LangGraph workflow for transaction investigation.

    The workflow consists of 7 nodes:
    1. analyze_transaction - Parse transaction data and identify discrepancy type
    2. check_settlement_timing - Compare timestamps against settlement cutoffs
    3. search_similar_incidents - Vector search for historical incidents
    4. retrieve_documentation - Vector search for relevant documentation
    5. analyze_dtre_replay - Interpret DTRE replay results
    6. synthesize_findings - LLM correlates evidence into coherent explanation
    7. generate_recommendations - Produce actionable next steps

    Returns:
        Compiled StateGraph ready for execution
    """
    from .nodes import (
        analyze_transaction_node,
        check_settlement_timing_node,
        search_similar_incidents_node,
        retrieve_documentation_node,
        analyze_dtre_replay_node,
        synthesize_findings_node,
        generate_recommendations_node,
    )

    workflow = StateGraph(InvestigationState)

    # Add nodes
    workflow.add_node("analyze_transaction", analyze_transaction_node)
    workflow.add_node("check_settlement_timing", check_settlement_timing_node)
    workflow.add_node("search_similar_incidents", search_similar_incidents_node)
    workflow.add_node("retrieve_documentation", retrieve_documentation_node)
    workflow.add_node("analyze_dtre_replay", analyze_dtre_replay_node)
    workflow.add_node("synthesize_findings", synthesize_findings_node)
    workflow.add_node("generate_recommendations", generate_recommendations_node)

    # Define workflow edges (linear pipeline)
    workflow.set_entry_point("analyze_transaction")
    workflow.add_edge("analyze_transaction", "check_settlement_timing")
    workflow.add_edge("check_settlement_timing", "search_similar_incidents")
    workflow.add_edge("search_similar_incidents", "retrieve_documentation")
    workflow.add_edge("retrieve_documentation", "analyze_dtre_replay")
    workflow.add_edge("analyze_dtre_replay", "synthesize_findings")
    workflow.add_edge("synthesize_findings", "generate_recommendations")
    workflow.add_edge("generate_recommendations", END)

    return workflow.compile()


async def run_investigation(
    investigation_id: str,
    query: str,
    transaction_data: Dict[str, Any],
    settlement_data: Dict[str, Any],
    audit_logs: List[Dict[str, Any]],
    dtre_replay_result: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """
    Execute the investigation workflow.

    Args:
        investigation_id: Unique investigation ID
        query: User's investigation query
        transaction_data: Transaction record
        settlement_data: Settlement information
        audit_logs: DTRE audit logs
        dtre_replay_result: Optional DTRE replay verification result

    Returns:
        Investigation result with findings, evidence, and recommendations
    """
    graph = create_investigation_graph()

    initial_state: InvestigationState = {
        "investigation_id": investigation_id,
        "query": query,
        "transaction_data": transaction_data,
        "settlement_data": settlement_data,
        "audit_logs": audit_logs,
        "dtre_replay_result": dtre_replay_result,
        "transaction_analysis": None,
        "settlement_timing_analysis": None,
        "similar_incidents": [],
        "relevant_docs": [],
        "dtre_analysis": None,
        "findings": None,
        "evidence": [],
        "recommendations": [],
        "agent_trace": [],
        "current_step": "analyze_transaction",
        "error": None,
    }

    result = await graph.ainvoke(initial_state)

    return {
        "status": "error" if result.get("error") else "success",
        "findings": result.get("findings"),
        "evidence": result.get("evidence"),
        "recommendations": result.get("recommendations"),
        "agent_trace": {
            "steps": result.get("agent_trace", []),
            "total_duration_ms": sum(
                step.get("duration_ms", 0) for step in result.get("agent_trace", [])
            ),
        },
    }
