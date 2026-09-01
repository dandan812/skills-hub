export type RuntimeAgentId = 'codex' | 'claude' | 'opencode' | 'pi'
export type TruthState = 'yes' | 'no' | 'unknown'
export type RuntimeSessionState = 'active' | 'ended' | 'unknown'
export type RuntimeOverviewStatus = 'ready' | 'partial' | 'unknown'
export type RuntimeCatalogState = 'available' | 'unavailable'
export type RuntimeEvidenceSource =
  | 'managementCatalog'
  | 'localHookReported'
  | 'catalogAndLocalHookReported'
  | 'unknown'

export type RuntimeLastCall =
  | { state: 'unknown' }
  | { state: 'observed'; observedAt: string }

export type RuntimeSkillRow = {
  skillId: string
  skillName: string
  installed: TruthState
  assigned: TruthState
  loaded: TruthState
  lastCall: RuntimeLastCall
  source: RuntimeEvidenceSource
  reason: string
}

export type RuntimeSession = {
  sessionId: string | null
  state: RuntimeSessionState
  startedAt: string | null
  lastObservedAt: string | null
  skills: RuntimeSkillRow[]
}

export type RuntimeAgent = {
  agentId: RuntimeAgentId
  catalogState: RuntimeCatalogState
  sessions: RuntimeSession[]
}

export type RuntimeImportSummary = {
  accepted: number
  duplicate: number
  rejected: number
  reasons: string[]
}

export type RuntimeEvidenceOverview = {
  schemaVersion: number
  status: RuntimeOverviewStatus
  import: RuntimeImportSummary
  inboxPath: string | null
  eventCount: number
  agents: RuntimeAgent[]
}

export type RuntimeEvidenceInvoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>
