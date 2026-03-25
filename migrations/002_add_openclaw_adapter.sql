-- Add 'openclaw' to the allowed adapter_type values
ALTER TABLE agents DROP CONSTRAINT IF EXISTS agents_adapter_type_check;
ALTER TABLE agents ADD CONSTRAINT agents_adapter_type_check
    CHECK (adapter_type IN ('http', 'claude_code', 'process', 'openclaw'));
