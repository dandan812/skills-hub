export type RuntimeEvidenceCollectorState = 'not_configured' | 'ready'

export type RuntimeEvidenceStatus = {
  schema_version: number
  event_name: string
  collector_state: RuntimeEvidenceCollectorState
  last_event_at_ms: number | null
  supported_event_types: string[]
}

export type RuntimeEvidenceInvoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>
