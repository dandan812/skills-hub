import { describe, expect, it } from 'vitest'
import { hasObservedRuntimeEvidence } from './status'
import type { RuntimeEvidenceStatus } from './types'

const makeStatus = (
  overrides: Partial<RuntimeEvidenceStatus> = {},
): RuntimeEvidenceStatus => ({
  schema_version: 1,
  event_name: 'runtime-evidence://event-v1',
  collector_state: 'not_configured',
  last_event_at_ms: null,
  supported_event_types: ['session_started', 'skill_loaded', 'skill_called'],
  ...overrides,
})
describe('hasObservedRuntimeEvidence', () => {
  it('keeps runtime claims unknown while the collector is not configured', () => {
    expect(hasObservedRuntimeEvidence(makeStatus())).toBe(false)
  })

  it('requires an observed event even when the collector is ready', () => {
    expect(hasObservedRuntimeEvidence(makeStatus({ collector_state: 'ready' }))).toBe(false)
  })

  it('recognizes evidence only after a ready collector reports an event', () => {
    expect(hasObservedRuntimeEvidence(makeStatus({
      collector_state: 'ready',
      last_event_at_ms: 1_725_000_000_000,
    }))).toBe(true)
  })
})
