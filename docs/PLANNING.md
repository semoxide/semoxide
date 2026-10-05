# Planning

GitHub conventions (labels, milestones, epics, dependencies, project board) are defined in Wave C.

## Research-phase waves
Each wave ends with a user discussion and approval before the next one starts.

```mermaid
flowchart LR
    S0["Step 0: bootstrap repo + doc skeleton"] --> A["Wave A: specs + research"]
    A --> DA{{"Discussion → ADRs"}}
    DA --> B["Wave B: git + plugin PoCs, porting gaps, testing, observability"]
    B --> DB{{"Discussion → ADRs"}}
    DB --> C["Wave C: architecture docs, then GitHub milestones/epics/issues/project"]
    C --> DC{{"Discussion"}}
    DC --> P["Product milestones"]
```
