# RA Generality And SRS v2

Requirement Analysis now produces one adaptive `ae-sdd-ra-srs/v2` SRS for every
engineering Work Item. Existing unversioned and v1 RA documents require an RA
correction before routing; they are not silently promoted to verified evidence.

The control-plane order is `Initialized -> RequirementAnalyzed -> RouteSelected`.
`G-RA-1..4` validate the bound SRS and receipt at `RequirementAnalyzed`.
`G-RA-FLOW-VIOLATION` is the real route-boundary Gate and recomputes the typed
SRS, receipt, scale evidence, candidate, approval, and conflict binding before
`EngineeringRoute` is frozen. `G-RA-5/6` remain compatibility entry points that
return real applicability and closure diagnostics, but they are no longer in an
automatic required Gate set.

Gate fingerprints use the exact `documentPaths/RA` file and verified RA receipt.
Foreign RA documents and unrelated project assets no longer invalidate RA Gates.
The policy digest revision invalidates receipts produced under the previous
route-before-RA semantics.
