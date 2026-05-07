# Runbook: Unexpected Fund Movement

## Alert
Unexpected account movement exceeds USD 1,000 equivalent.

## Severity
P0

## Investigation
1. Identify affected accounts and transaction ids.
2. Verify INV-1 through INV-6 constraints for each transaction.
3. Check recent payment, settlement, and admin actions.

## Resolution
1. Freeze affected accounts through the approved compliance flow.
2. Stop related wallet mutation routes.
3. Require Compliance approval before reversal transactions.
