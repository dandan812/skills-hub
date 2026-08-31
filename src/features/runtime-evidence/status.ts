import type { RuntimeEvidenceStatus } from './types'

export const hasObservedRuntimeEvidence = (
  status: RuntimeEvidenceStatus | null,
): boolean =>
  status?.collector_state === 'ready' && status.last_event_at_ms !== null
