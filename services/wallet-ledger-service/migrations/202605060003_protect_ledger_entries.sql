CREATE OR REPLACE FUNCTION fn_prevent_ledger_entry_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'ledger_entries are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_prevent_ledger_entry_update
BEFORE UPDATE ON ledger_entries
FOR EACH ROW EXECUTE FUNCTION fn_prevent_ledger_entry_mutation();

CREATE TRIGGER trg_prevent_ledger_entry_delete
BEFORE DELETE ON ledger_entries
FOR EACH ROW EXECUTE FUNCTION fn_prevent_ledger_entry_mutation();
