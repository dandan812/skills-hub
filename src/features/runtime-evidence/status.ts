import type {
  RuntimeAgent,
  RuntimeAgentId,
  RuntimeEvidenceOverview,
  RuntimeSession,
  RuntimeSkillRow,
} from './types'

export const findAgent = (
  overview: RuntimeEvidenceOverview | null,
  agentId: RuntimeAgentId,
): RuntimeAgent | null =>
  overview?.agents.find((agent) => agent.agentId === agentId) ?? null

export const sessionKey = (session: RuntimeSession): string =>
  session.sessionId ?? '__unknown__'

export const findSession = (
  agent: RuntimeAgent | null,
  selectedKey: string,
): RuntimeSession | null =>
  agent?.sessions.find((session) => sessionKey(session) === selectedKey)
    ?? agent?.sessions[0]
    ?? null

export const filterRuntimeRows = (
  rows: RuntimeSkillRow[],
  query: string,
): RuntimeSkillRow[] => {
  const needle = query.trim().toLocaleLowerCase()
  if (!needle) return rows
  return rows.filter((row) =>
    `${row.skillName}\n${row.skillId}`.toLocaleLowerCase().includes(needle),
  )
}
