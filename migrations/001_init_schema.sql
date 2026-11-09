-- AI-Powered Transaction Investigation Platform
-- Database Schema Migration 001: Initial Schema
--
-- This migration creates the foundational tables for:
-- - Transaction and settlement data storage
-- - Audit logging and DTRE integration
-- - Historical incidents and documentation for RAG
-- - Vector embeddings for similarity search
-- - Investigation tracking

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgvector";
CREATE EXTENSION IF NOT EXISTS "btree_gist";

-- ============================================================================
-- CORE TRANSACTION DATA
-- ============================================================================

-- Transactions table: stores internal transaction records
CREATE TABLE transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    external_id VARCHAR(255) UNIQUE NOT NULL,
    transaction_type VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL,
    amount BIGINT NOT NULL,
    currency CHAR(3) NOT NULL,
    source_account VARCHAR(255) NOT NULL,
    destination_account VARCHAR(255) NOT NULL,
    payment_network VARCHAR(100) NOT NULL,
    initiated_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_transactions_external_id ON transactions(external_id);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_initiated_at ON transactions(initiated_at);
CREATE INDEX idx_transactions_payment_network ON transactions(payment_network);
CREATE INDEX idx_transactions_created_at ON transactions(created_at);

-- Settlements table: daily settlement reports from payment networks
CREATE TABLE settlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    settlement_id VARCHAR(255) UNIQUE NOT NULL,
    payment_network VARCHAR(100) NOT NULL,
    settlement_date DATE NOT NULL,
    reporting_date DATE NOT NULL,
    transaction_count INTEGER NOT NULL,
    total_amount BIGINT NOT NULL,
    currency CHAR(3) NOT NULL,
    raw_data JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_settlements_settlement_date ON settlements(settlement_date);
CREATE INDEX idx_settlements_payment_network ON settlements(payment_network);
CREATE INDEX idx_settlements_reporting_date ON settlements(reporting_date);

-- Settlement items table: individual transactions within settlement reports
CREATE TABLE settlement_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    settlement_id UUID NOT NULL REFERENCES settlements(id) ON DELETE CASCADE,
    external_transaction_id VARCHAR(255) NOT NULL,
    amount BIGINT NOT NULL,
    currency CHAR(3) NOT NULL,
    settled_at TIMESTAMPTZ NOT NULL,
    status VARCHAR(50) NOT NULL,
    metadata JSONB
);

CREATE INDEX idx_settlement_items_settlement_id ON settlement_items(settlement_id);
CREATE INDEX idx_settlement_items_external_transaction_id ON settlement_items(external_transaction_id);
CREATE INDEX idx_settlement_items_settled_at ON settlement_items(settled_at);

-- Audit logs table: DTRE-generated audit trail for each transaction
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id UUID REFERENCES transactions(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    state_before JSONB,
    state_after JSONB,
    state_hash_before VARCHAR(64),
    state_hash_after VARCHAR(64),
    rule_version VARCHAR(50),
    occurred_at TIMESTAMPTZ NOT NULL,
    details JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_transaction_id ON audit_logs(transaction_id);
CREATE INDEX idx_audit_logs_event_type ON audit_logs(event_type);
CREATE INDEX idx_audit_logs_occurred_at ON audit_logs(occurred_at);
CREATE INDEX idx_audit_logs_state_hash_after ON audit_logs(state_hash_after);

-- ============================================================================
-- RAG DATA: INCIDENTS AND DOCUMENTATION
-- ============================================================================

-- Incidents table: historical incidents for RAG retrieval
CREATE TABLE incidents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id VARCHAR(255) UNIQUE NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    incident_type VARCHAR(100) NOT NULL,
    severity VARCHAR(50) NOT NULL,
    root_cause TEXT,
    resolution TEXT,
    payment_network VARCHAR(100),
    affected_transaction_count INTEGER,
    occurred_at TIMESTAMPTZ NOT NULL,
    resolved_at TIMESTAMPTZ,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_incidents_incident_type ON incidents(incident_type);
CREATE INDEX idx_incidents_payment_network ON incidents(payment_network);
CREATE INDEX idx_incidents_occurred_at ON incidents(occurred_at);
CREATE INDEX idx_incidents_severity ON incidents(severity);

-- Documentation table: payment provider documentation for RAG
CREATE TABLE documentation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doc_id VARCHAR(255) UNIQUE NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    doc_type VARCHAR(100) NOT NULL,
    payment_network VARCHAR(100),
    version VARCHAR(50),
    tags TEXT[],
    url TEXT,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_documentation_doc_type ON documentation(doc_type);
CREATE INDEX idx_documentation_payment_network ON documentation(payment_network);
CREATE INDEX idx_documentation_tags ON documentation USING GIN(tags);

-- Embeddings table: vector embeddings for RAG similarity search
CREATE TABLE embeddings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content_type VARCHAR(50) NOT NULL,
    content_id UUID NOT NULL,
    embedding vector(1536),
    content_text TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_embeddings_content_type_id ON embeddings(content_type, content_id);

-- pgvector similarity search index using IVFFlat
CREATE INDEX idx_embeddings_vector ON embeddings
USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 100);

-- Investigations table: tracks investigation sessions for auditing
CREATE TABLE investigations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    query TEXT NOT NULL,
    transaction_id UUID REFERENCES transactions(id) ON DELETE SET NULL,
    status VARCHAR(50) NOT NULL,
    findings JSONB,
    recommendations TEXT[],
    evidence JSONB,
    agent_trace JSONB,
    initiated_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_investigations_transaction_id ON investigations(transaction_id);
CREATE INDEX idx_investigations_status ON investigations(status);
CREATE INDEX idx_investigations_created_at ON investigations(created_at);
CREATE INDEX idx_investigations_initiated_by ON investigations(initiated_by);

-- ============================================================================
-- FUNCTIONS AND TRIGGERS
-- ============================================================================

-- Function to automatically update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply updated_at trigger to relevant tables
CREATE TRIGGER update_transactions_updated_at
    BEFORE UPDATE ON transactions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_documentation_updated_at
    BEFORE UPDATE ON documentation
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- COMMENTS FOR DOCUMENTATION
-- ============================================================================

COMMENT ON TABLE transactions IS 'Internal transaction records with status and metadata';
COMMENT ON TABLE settlements IS 'Daily settlement reports from external payment networks';
COMMENT ON TABLE settlement_items IS 'Individual transaction line items within settlement reports';
COMMENT ON TABLE audit_logs IS 'DTRE-generated audit trail showing state transitions';
COMMENT ON TABLE incidents IS 'Historical incidents for RAG-based investigation assistance';
COMMENT ON TABLE documentation IS 'Payment provider documentation for RAG retrieval';
COMMENT ON TABLE embeddings IS 'Vector embeddings for semantic similarity search';
COMMENT ON TABLE investigations IS 'Investigation session tracking and audit trail';

COMMENT ON COLUMN embeddings.embedding IS 'OpenAI ada-002 embedding vector (1536 dimensions)';
COMMENT ON COLUMN audit_logs.state_hash_before IS 'Blake3 hash of state before transaction';
COMMENT ON COLUMN audit_logs.state_hash_after IS 'Blake3 hash of state after transaction';
